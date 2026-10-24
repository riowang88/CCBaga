// [INPUT]: 已认证本机身份归档、原生一次性文件选择、ring PBKDF2 与现有 AES-GCM/tar/Zstd 流。
// [OUTPUT]: 密码保护的单身份 bagapack；同 OS 验收后重定位并重封存；拒绝本机 v2 链接归档且不跟随链接导出目标内容。
// [POS]: 保险库的显式跨设备边界；不导出主密钥/SQLite，不切换身份或操作钥匙串，不迁移浏览器登录资料。
// [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
use super::{archive::{self, Manifest, Stage}, codec, compression, database::Record, private, store::{problem, Store}};
use crate::progress::{Progress, Reporter};
use base64::{engine::general_purpose::STANDARD, Engine};
use ring::pbkdf2;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, io::{self, Read, Write}, num::NonZeroU32, path::{Component, Path, PathBuf}, sync::Mutex, time::{Duration, Instant}};
use tauri::{ipc::Channel, Manager};
use tauri_plugin_dialog::DialogExt;
use zeroize::Zeroizing;
#[cfg(windows)]
#[path = "portable_windows.rs"]
mod portable_windows;

const MAGIC: &[u8; 8] = b"BAGAP001";
const ITERATIONS: u32 = 600_000;
const MAX_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 100_000;
const MAX_METADATA: u64 = 8 * 1024 * 1024;
const MAX_SOURCES: usize = 4096;
const MAX_NAME_BYTES: usize = 4096;
// GNU 长名头/4608 字节长名区/普通头/成员填充最多 6144 字节；载荷仍单独限制 8 GiB。
const MAX_TAR: u64 = MAX_BYTES + MAX_METADATA + (MAX_ENTRIES as u64 + 1) * 6144 + 1024;
const MAX_PACK: u64 = MAX_TAR + MAX_TAR / 128 + 8 * 1024 * 1024;
const TTL: Duration = Duration::from_secs(300);
fn invalid() -> String { "迁移包无法验证、密码错误或格式不受支持；未导入身份。".into() }
fn limit() -> String { "迁移包超过 8 GiB 或 100000 个成员的安全上限。".into() }

#[derive(Clone, Copy, PartialEq)]
enum Kind { Export, Import }
impl Kind {
    fn parse(value: &str) -> Result<Self, String> {
        match value { "export" => Ok(Self::Export), "import" => Ok(Self::Import), _ => Err("迁移操作无效。".into()) }
    }
}
#[derive(PartialEq)]
struct Stamp { length: u64, changed: std::time::SystemTime, identity: (u64, u64) }
fn stamp(file: &fs::File) -> Result<Stamp, String> {
    let metadata = file.metadata().map_err(problem)?;
    if !metadata.is_file() || crate::cleaner::linked(&metadata) { return Err(invalid()); }
    #[cfg(unix)]
    let identity = { use std::os::unix::fs::MetadataExt; (metadata.dev(), metadata.ino()) };
    #[cfg(windows)]
    let identity = {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION};
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { GetFileInformationByHandle(file.as_raw_handle().cast(), &mut info) } == 0 { return Err(invalid()); }
        (u64::from(info.dwVolumeSerialNumber), (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow))
    };
    Ok(Stamp { length: metadata.len(), changed: metadata.modified().map_err(problem)?, identity })
}
fn selected_file(path: &Path) -> Result<fs::File, String> {
    private::parents(path.parent().ok_or_else(invalid)?)?;
    private::safe(path)?;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)] { use std::os::windows::fs::OpenOptionsExt; options.custom_flags(0x00200000); }
    let file = options.open(path).map_err(problem)?;
    stamp(&file)?;
    #[cfg(windows)] portable_windows::owned_file(&file)?;
    #[cfg(unix)] {
        use std::os::unix::fs::MetadataExt;
        extern "C" { fn getuid() -> u32; }
        if file.metadata().map_err(problem)?.uid() != unsafe { getuid() } { return Err(invalid()); }
    }
    private::parents(path.parent().ok_or_else(invalid)?)?;
    private::safe(path)?;
    Ok(file)
}
struct Grant { token: String, path: PathBuf, kind: Kind, created: Instant, stamp: Option<Stamp> }
#[derive(Default)]
pub(crate) struct Transfers { grant: Mutex<Option<Grant>> }
#[derive(Serialize)]
pub(crate) struct Selection { token: String, file_name: String }
impl Transfers {
    fn issue(&self, path: PathBuf, kind: Kind) -> Result<Selection, String> {
        if !path.is_absolute() || !path.extension().and_then(|s| s.to_str()).is_some_and(|s| s.eq_ignore_ascii_case("bagapack")) {
            return Err("请选择 .bagapack 文件。".into());
        }
        if !path.file_name().is_some_and(|name| valid_member(Path::new(name))) { return Err(invalid()); }
        private::parents(path.parent().ok_or_else(invalid)?)?;
        let stamp = if kind == Kind::Import {
            let stamp = stamp(&selected_file(&path)?)?;
            if stamp.length > MAX_PACK { return Err(limit()); }
            Some(stamp)
        } else {
            if fs::symlink_metadata(&path).is_ok() { return Err("导出目标已存在；请选择新文件名。".into()); }
            None
        };
        let token = super::new_id()?;
        let file_name = path.file_name().and_then(|s| s.to_str()).filter(|s| !s.chars().any(char::is_control)).unwrap_or("identity.bagapack").into();
        *self.grant.lock().map_err(|_| invalid())? = Some(Grant { token: token.clone(), path, kind, created: Instant::now(), stamp });
        Ok(Selection { token, file_name })
    }
    fn take(&self, token: &str, kind: Kind) -> Result<Grant, String> {
        let mut entry = self.grant.lock().map_err(|_| invalid())?;
        if !entry.as_ref().is_some_and(|g| g.token == token && g.kind == kind && g.created.elapsed() < TTL) {
            return Err("文件选择已过期或已使用，请重新选择。".into());
        }
        Ok(entry.take().unwrap())
    }
}
#[tauri::command]
pub(crate) async fn identities_transfer_pick(app: tauri::AppHandle, window: tauri::Window, kind: String) -> Result<Option<Selection>, String> {
    let kind = Kind::parse(&kind)?;
    app.state::<Transfers>().grant.lock().map_err(|_| invalid())?.take();
    let dialog = app.dialog().file().set_parent(&window).add_filter("CCBaga identity", &["bagapack"]);
    let selection = if kind == Kind::Export { dialog.set_file_name("identity.bagapack").blocking_save_file() } else { dialog.blocking_pick_file() };
    selection.map(|file| {
        let path = file.into_path().map_err(|_| "请选择本机文件。")?;
        app.state::<Transfers>().issue(path, kind)
    }).transpose()
}
fn password_key(password: &str, salt: &[u8; 32]) -> Result<Zeroizing<[u8; 32]>, String> {
    if password.chars().count() < 12 || password.len() > 1024 {
        return Err("迁移密码至少 12 个字符，且不超过 1024 个 UTF-8 字节。".into());
    }
    let mut key = Zeroizing::new([0; 32]);
    pbkdf2::derive(pbkdf2::PBKDF2_HMAC_SHA256, NonZeroU32::new(ITERATIONS).unwrap(), salt, password.as_bytes(), key.as_mut());
    Ok(key)
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BrowserSettings { seed: u32, follow_machine: bool, language: String, timezone: String, native_graphics: bool }
impl BrowserSettings {
    fn from_config(config: crate::browser::config::Config) -> Self {
        Self { seed: config.seed, follow_machine: config.follow_machine, language: config.language, timezone: config.timezone, native_graphics: config.native_graphics }
    }
    fn config(&self) -> Result<crate::browser::config::Config, String> {
        if self.seed == 0 || self.language.len() > 32 || self.timezone.len() > 128 || self.language.chars().chain(self.timezone.chars()).any(char::is_control) { return Err(invalid()); }
        Ok(crate::browser::config::Config { seed: self.seed, follow_machine: self.follow_machine, language: self.language.clone(), timezone: self.timezone.clone(), native_graphics: self.native_graphics, proxy: String::new(), dns: Default::default() })
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Metadata { version: u32, name: String, manifest: Manifest, browser: Option<BrowserSettings> }
fn credentials(items: &[serde_json::Value], policy: &super::Policy) -> Result<Vec<serde_json::Value>, String> {
    #[cfg(not(target_os = "macos"))]
    if !items.is_empty() { return Err(invalid()); }
    if items.len() > 64 { return Err(invalid()); }
    let mut seen = HashSet::new();
    items.iter().map(|item| {
        let service = item["service"].as_str().ok_or_else(invalid)?;
        let account = item["account"].as_str().ok_or_else(invalid)?;
        if !["Claude Code-credentials", "Claude Code", "Claude Safe Storage"].contains(&service) || account.len() > 1024 || !seen.insert((service, account)) { return Err(invalid()); }
        let data = item["data"].as_str().filter(|s| s.len() <= MAX_METADATA as usize).ok_or_else(invalid)?;
        let mut bytes = Zeroizing::new(STANDARD.decode(data).map_err(|_| invalid())?);
        if service == "Claude Code-credentials" {
            let mut value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
            let keys = crate::cleaner::identity::credential_keys(policy);
            value.as_object_mut().ok_or_else(invalid)?.retain(|key, _| keys.contains(key));
            *bytes = serde_json::to_vec(&value).map_err(problem)?;
        }
        Ok(serde_json::json!({"service":service,"account":account,"data":STANDARD.encode(bytes.as_slice())}))
    }).collect()
}
fn member_key(path: &Path) -> Option<String> {
    if path.as_os_str().is_empty() || !path.to_str().is_some_and(|s| s.len() <= MAX_NAME_BYTES && !s.chars().any(char::is_control)) || !path.components().all(|c| matches!(c, Component::Normal(_))) { return None; }
    #[cfg(windows)] { portable_windows::member_key(path) }
    #[cfg(not(windows))] { path.to_str().map(str::to_owned) }
}
fn valid_member(path: &Path) -> bool { member_key(path).is_some() }
fn validate_metadata(metadata: &Metadata) -> Result<(), String> {
    let m = &metadata.manifest;
    super::name(&metadata.name)?;
    if metadata.version != 1 || m.version != 1 || !super::valid_id(&m.id) || !m.home.is_absolute() || m.home.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir)) || m.platform != std::env::consts::OS {
        return Err("迁移包版本或来源系统不兼容；仅支持相同操作系统之间迁移。".into());
    }
    if m.sources.len() > MAX_SOURCES || m.sources.len() != m.fingerprints.len() || (m.sources.is_empty() && m.credentials.is_empty()) { return Err(invalid()); }
    let mut paths = HashSet::new();
    for source in &m.sources {
        let relative = source.path.strip_prefix(&m.home).map_err(|_| invalid())?;
        if !paths.insert(member_key(relative).ok_or_else(invalid)?) || !crate::cleaner::identity::allowed(&m.home, source) { return Err(invalid()); }
    }
    if m.sources.iter().any(|source| m.sources.iter().any(|parent| parent.path != source.path && parent.keys.is_empty() && source.path.starts_with(&parent.path))) { return Err(invalid()); }
    if let Some(browser) = &metadata.browser { browser.config()?; }
    let filtered = credentials(&m.credentials, &m.policy)?;
    if filtered != m.credentials || m.account_key.is_some() { return Err(invalid()); }
    Ok(())
}
fn append<W: Write>(tar: &mut tar::Builder<W>, name: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64); header.set_mode(0o600); header.set_cksum();
    tar.append_data(&mut header, name, bytes).map_err(problem)
}
fn each_data(stage: &Stage, visit: &mut impl FnMut(&Path, &Path, bool) -> Result<(), String>) -> Result<(), String> {
    for n in 0..stage.manifest.sources.len() { archive::walk(&stage.path.join(format!("data/{n}")), &stage.path, visit)?; }
    Ok(())
}
fn checked_data(stage: &Stage) -> Result<(usize, u64), String> {
    if stage.manifest.version != 1 {
        return Err("包含本机符号链接的身份暂不支持跨设备导出。".into());
    }
    let (mut count, mut size) = (0usize, 0u64);
    each_data(stage, &mut |path, relative, directory| {
        private::safe(path)?;
        count += 1;
        if count > MAX_ENTRIES || !valid_member(relative) { return Err(limit()); }
        if !directory { size = size.checked_add(fs::metadata(path).map_err(problem)?.len()).ok_or_else(limit)?; }
        if size > MAX_BYTES { return Err(limit()); }
        Ok(())
    })?;
    Ok((count, size))
}
fn pack_data<W: Write>(tar: &mut tar::Builder<W>, stage: &Stage, reporter: &Reporter) -> Result<(), String> {
    let (total, _) = checked_data(stage)?;
    let mut current = 0;
    let mut last = Instant::now();
    reporter.log("transfer_archive", "running", "正在写入迁移文件。", 0, Some(total));
    each_data(stage, &mut |path, relative, directory| {
        if directory { tar.append_dir(relative, path).map_err(problem)?; }
        else { tar.append_file(relative, &mut fs::File::open(path).map_err(problem)?).map_err(problem)?; }
        current += 1;
        if current == total || last.elapsed() >= Duration::from_millis(50) {
            reporter.log("transfer_archive", "running", "已写入迁移文件。", current, Some(total));
            last = Instant::now();
        }
        Ok(())
    })?;
    reporter.log("transfer_archive", "ok", "迁移文件已完整写入压缩流。", total, Some(total));
    Ok(())
}
impl Store {
    pub(super) fn export_pack(&self, path: &Path, password: &str, id: &str, browser: Option<crate::browser::config::Config>, reporter: &Reporter) -> Result<(), String> {
        let record = self.record(id)?;
        reporter.log("transfer_validate", "running", "正在验收已保存身份。", 0, None);
        let stage = self.load(&record)?;
        checked_data(&stage)?;
        let mut manifest: Manifest = serde_json::from_value(serde_json::to_value(&stage.manifest).map_err(problem)?).map_err(problem)?;
        manifest.credentials = credentials(&manifest.credentials, &manifest.policy)?;
        manifest.account_key = None;
        let metadata = Metadata { version: 1, name: record.name, manifest, browser: browser.map(BrowserSettings::from_config) };
        validate_metadata(&metadata)?;
        let serialized = Zeroizing::new(serde_json::to_vec(&metadata).map_err(problem)?);
        if serialized.len() as u64 > MAX_METADATA { return Err(limit()); }
        reporter.log("transfer_validate", "ok", "已保存身份已完整验收。", 1, Some(1));
        let salt = codec::random::<32>()?;
        let key = password_key(password, &salt)?;
        let mut output = private::create(path)?;
        let created = stamp(&output)?;
        let result = (|| {
            output.write_all(MAGIC).and_then(|_| output.write_all(&ITERATIONS.to_be_bytes())).and_then(|_| output.write_all(&salt)).map_err(problem)?;
            let seal = codec::Seal::new(output.try_clone().map_err(problem)?, key.as_ref()).map_err(problem)?;
            let mut tar = tar::Builder::new(compression::encode(seal).map_err(problem)?);
            append(&mut tar, Path::new("transfer.json"), &serialized)?;
            pack_data(&mut tar, &stage, reporter)?;
            reporter.log("transfer_auth", "running", "正在完成迁移包认证与落盘。", 0, None);
            let encoder = tar.into_inner().map_err(problem)?;
            let seal = encoder.finish().map_err(problem)?;
            let file = seal.finish().map_err(problem)?;
            file.sync_all().map_err(problem)?;
            private::sync_parent(path);
            reporter.log("transfer_auth", "ok", "密码保护的迁移包已完整保存。", 1, Some(1));
            Ok(())
        })();
        drop(output);
        if result.is_err() && selected_file(path).and_then(|file| stamp(&file)).is_ok_and(|s| s.identity == created.identity) { let _ = fs::remove_file(path); }
        result
    }
}
struct Budget<R> { inner: R, remaining: u64 }
impl<R: Read> Read for Budget<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let cap = buffer.len().min(self.remaining.saturating_add(1).min(usize::MAX as u64) as usize);
        let read = self.inner.read(&mut buffer[..cap])?;
        if read as u64 > self.remaining { return Err(io::Error::new(io::ErrorKind::InvalidData, "迁移包超过解压上限。")); }
        self.remaining -= read as u64;
        Ok(read)
    }
}
pub(super) struct Imported { pub(super) stage: Stage, name: String, pub(super) browser: Option<BrowserSettings> }
fn unpack(mut file: fs::File, password: &str, parent: &Path, reporter: &Reporter) -> Result<Imported, String> {
    let mut header = [0; 44];
    file.read_exact(&mut header).map_err(|_| invalid())?;
    if &header[..8] != MAGIC || header[8..12] != ITERATIONS.to_be_bytes() { return Err(invalid()); }
    let salt: &[u8; 32] = header[12..].try_into().unwrap();
    let key = password_key(password, salt)?;
    let decrypt = codec::Open::new(file, key.as_ref()).map_err(|_| invalid())?;
    let decoded = compression::decode(decrypt).map_err(|_| invalid())?;
    let mut input = Budget { inner: decoded, remaining: MAX_TAR };
    let folder = parent.join(format!(".identity-transfer-{}", super::new_id()?));
    private::create_directory(&folder)?;
    let result = (|| {
        let mut metadata = None;
        let mut members = HashSet::new();
        let (mut count, mut bytes) = (0usize, 0u64);
        let mut last = Instant::now();
        reporter.log("transfer_validate", "running", "正在认证并检查迁移包。", 0, None);
        {
            let mut tar = tar::Archive::new(&mut input);
            let mut long_name = None;
            let mut headers = 0usize;
            // 原始模式防止 tar 库在白名单检查前为攻击者的 GNU/PAX 长头分配无限内存。
            for entry in tar.entries().map_err(|_| invalid())?.raw(true) {
                let mut entry = entry.map_err(|_| invalid())?;
                headers += 1;
                if headers > MAX_ENTRIES * 2 + 1 { return Err(limit()); }
                if entry.header().entry_type().is_gnu_longname() {
                    if long_name.is_some() || entry.size() > MAX_NAME_BYTES as u64 + 1 { return Err(invalid()); }
                    let mut text = Vec::new();
                    entry.read_to_end(&mut text).map_err(|_| invalid())?;
                    if text.last() == Some(&0) { text.pop(); }
                    if text.contains(&0) { return Err(invalid()); }
                    long_name = Some(PathBuf::from(String::from_utf8(text).map_err(|_| invalid())?));
                    continue;
                }
                let path = long_name.take().map(Ok).unwrap_or_else(|| entry.path().map(|p| p.into_owned()).map_err(|_| invalid()))?;
                count += 1;
                if count > MAX_ENTRIES + 1 || !members.insert(member_key(&path).ok_or_else(invalid)?) { return Err(invalid()); }
                if metadata.is_none() {
                    if path != Path::new("transfer.json") || !entry.header().entry_type().is_file() || entry.size() > MAX_METADATA { return Err(invalid()); }
                    let mut encoded = Zeroizing::new(Vec::new());
                    entry.read_to_end(&mut encoded).map_err(|_| invalid())?;
                    let value: serde_json::Value = serde_json::from_slice(&encoded).map_err(|_| invalid())?;
                    let known = ["version", "id", "home", "platform", "policy", "sources", "credentials", "fingerprints", "account_key"];
                    if !value["manifest"].as_object().is_some_and(|m| m.keys().all(|key| known.contains(&key.as_str()))) { return Err(invalid()); }
                    if !value["manifest"]["sources"].as_array().is_some_and(|rows| rows.iter().all(|row| row.as_object().is_some_and(|row| row.keys().all(|key| ["path", "category", "keys"].contains(&key.as_str()))))) { return Err(invalid()); }
                    if !value["manifest"]["policy"].as_object().is_some_and(|row| row.keys().all(|key| ["skills", "plugins", "mcp", "settings", "projects", "history", "cache"].contains(&key.as_str()))) { return Err(invalid()); }
                    let parsed: Metadata = serde_json::from_value(value).map_err(|_| invalid())?;
                    validate_metadata(&parsed)?;
                    metadata = Some(parsed);
                    continue;
                }
                let sources = metadata.as_ref().unwrap().manifest.sources.len();
                let mut parts = path.components();
                if parts.next() != Some(Component::Normal("data".as_ref())) { return Err(invalid()); }
                let n = parts.next().and_then(|s| s.as_os_str().to_str()).ok_or_else(invalid)?;
                let number = n.parse::<usize>().map_err(|_| invalid())?;
                if number >= sources || number.to_string() != n { return Err(invalid()); }
                let kind = entry.header().entry_type();
                if !kind.is_file() && !kind.is_dir() { return Err(invalid()); }
                bytes = bytes.checked_add(entry.size()).ok_or_else(limit)?;
                if bytes > MAX_BYTES { return Err(limit()); }
                let destination = folder.join(&path);
                if kind.is_dir() { if entry.size() != 0 { return Err(invalid()); } private::directory(&destination)?; }
                else {
                    private::directory(destination.parent().ok_or_else(invalid)?)?;
                    let mut output = private::create(&destination)?;
                    let written = io::copy(&mut entry, &mut output).map_err(|_| invalid())?;
                    if written != entry.size() { return Err(invalid()); }
                }
                if last.elapsed() >= Duration::from_millis(50) {
                    reporter.log("transfer_validate", "running", "已认证迁移包成员。", count - 1, None);
                    last = Instant::now();
                }
            }
            if long_name.is_some() { return Err(invalid()); }
        }
        // tar 终止不能代替 AES-GCM 认证结尾；完整排空后才允许打开目标保险库。
        let mut tail = Zeroizing::new([0; 64 * 1024]);
        loop {
            let length = input.read(tail.as_mut()).map_err(|_| invalid())?;
            if length == 0 { break; }
            if tail[..length].iter().any(|byte| *byte != 0) { return Err(invalid()); }
        }
        let metadata = metadata.ok_or_else(invalid)?;
        for (n, source) in metadata.manifest.sources.iter().enumerate() {
            let data = folder.join(format!("data/{n}"));
            private::safe(&data)?;
            if !source.keys.is_empty() {
                if !data.is_file() || fs::metadata(&data).map_err(problem)?.len() > MAX_METADATA { return Err(invalid()); }
                let value: serde_json::Value = serde_json::from_slice(&fs::read(&data).map_err(problem)?).map_err(|_| invalid())?;
                if !value.as_object().is_some_and(|v| v.keys().all(|key| source.keys.contains(key))) { return Err(invalid()); }
            }
        }
        reporter.log("transfer_validate", "ok", "迁移包已通过完整认证与范围检查。", count - 1, Some(count - 1));
        Ok(Imported { stage: Stage { path: folder.clone(), manifest: metadata.manifest }, name: metadata.name, browser: metadata.browser })
    })();
    if result.is_err() { let _ = fs::remove_dir_all(folder); }
    result
}
impl Store {
    pub(super) fn import_pack(&self, mut imported: Imported, browser: Option<(&str, u32)>, reporter: &Reporter) -> Result<Record, String> {
        let m = &mut imported.stage.manifest;
        let origin = m.home.clone();
        m.home = self.home.clone(); m.id = super::new_id()?;
        for source in &mut m.sources {
            source.path = self.home.join(source.path.strip_prefix(&origin).map_err(|_| invalid())?);
            if !crate::cleaner::identity::allowed(&self.home, source) { return Err(invalid()); }
        }
        let account = m.sources.iter().position(|s| s.path == self.home.join(".claude.json")).map(|n| {
            let file = imported.stage.path.join(format!("data/{n}"));
            let value = serde_json::from_slice(&fs::read(file).map_err(problem)?).map_err(problem)?;
            Ok::<_, String>(super::catalog::account_key(&value, &self.key))
        }).transpose()?.flatten();
        m.account_key = Some(account.clone().unwrap_or_default());
        let generation = super::new_id()?;
        let pending = self.vault.join(format!(".pending-{generation}"));
        let destination = self.vault.join(format!("{generation}.baga"));
        let (mut pending_created, mut destination_created) = (false, false);
        let result = (|| {
            let output = private::create(&pending)?;
            pending_created = true;
            let seal = codec::Seal::new(output, &self.key).map_err(problem)?;
            let mut tar = tar::Builder::new(compression::encode(seal).map_err(problem)?);
            pack_data(&mut tar, &imported.stage, reporter)?;
            append(&mut tar, Path::new("manifest.json"), &serde_json::to_vec(&imported.stage.manifest).map_err(problem)?)?;
            let seal = tar.into_inner().map_err(problem)?.finish().map_err(problem)?;
            let file = seal.finish().map_err(problem)?; file.sync_all().map_err(problem)?; drop(file);
            let saved_at = u64::try_from(super::profile::now_ms()).map_err(|_| invalid())?;
            let record = Record { id: imported.stage.manifest.id.clone(), name: imported.name, archive: format!("{generation}.baga"), saved_at, bytes: fs::metadata(&pending).map_err(problem)?.len(), policy: imported.stage.manifest.policy.clone(), empty: imported.stage.manifest.sources.is_empty() && imported.stage.manifest.credentials.is_empty(), account_key: Some(account.unwrap_or_default()) };
            // 随机代次仍使用不可覆盖硬链接提交，数据库失败仅移除本次新文件。
            fs::hard_link(&pending, &destination).map_err(problem)?;
            destination_created = true;
            private::sync_parent(&destination);
            let stage = self.load(&record)?;
            reporter.log("transfer_commit", "running", "正在新增本机身份归档。", 0, None);
            self.db.import_record(&record, &super::profile::summarize_stage(&stage), browser)?;
            reporter.log("transfer_commit", "ok", "身份已导入；当前身份未切换。", 1, Some(1));
            Ok(record)
        })();
        if pending_created { let _ = fs::remove_file(&pending); }
        if result.is_err() && destination_created { let _ = fs::remove_file(destination); }
        result
    }
}
#[cfg(test)]
#[path = "portable_tests.rs"]
mod tests;
#[tauri::command]
pub(crate) async fn identities_export(app: tauri::AppHandle, id: String, grant: String, password: String, include_browser: bool, on_progress: Channel<Progress>) -> Result<super::State, String> {
    let password = Zeroizing::new(password);
    let selection = app.state::<Transfers>().take(&grant, Kind::Export)?;
    let cleaner = app.state::<crate::cleaner::Cleaner>(); cleaner.enter()?;
    let worker = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let reporter = Reporter::channel(Some(on_progress));
        let result = (|| {
            let store = Store::open_for_transfer(&super::vault(&worker)?, &crate::cleaner::identity::root()?)?;
            let browser = if include_browser { crate::browser::portable_config(&worker, &id)? } else { None };
            store.export_pack(&selection.path, &password, &id, browser, &reporter)?;
            super::state(&worker)
        })();
        if let Err(error) = &result { reporter.fail(error); }
        result
    }).await.map_err(|_| "身份导出中断，请重新选择文件。".to_string()).and_then(|r| r);
    cleaner.leave(); result
}
#[tauri::command]
pub(crate) async fn identities_import(app: tauri::AppHandle, grant: String, password: String, on_progress: Channel<Progress>) -> Result<super::State, String> {
    let password = Zeroizing::new(password);
    let selection = app.state::<Transfers>().take(&grant, Kind::Import)?;
    let cleaner = app.state::<crate::cleaner::Cleaner>(); cleaner.enter()?;
    let worker = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let reporter = Reporter::channel(Some(on_progress));
        let result = (|| {
            let file = selected_file(&selection.path)?;
            if Some(stamp(&file)?) != selection.stamp { return Err("所选迁移包已变化，请重新选择。".into()); }
            let parent = worker.path().app_data_dir().map_err(|_| invalid())?;
            private::directory(&parent)?;
            let imported = unpack(file, &password, &parent, &reporter)?;
            if Some(stamp(&selected_file(&selection.path)?)?) != selection.stamp { return Err("所选迁移包已变化，请重新选择。".into()); }
            let store = Store::open_for_transfer(&super::vault(&worker)?, &crate::cleaner::identity::root()?)?;
            let slot = super::new_id()?;
            let browser = imported.browser.as_ref().map(BrowserSettings::config).transpose()?;
            let directory = browser.as_ref().map(|config| crate::browser::portable_prepare(&worker, &slot, config)).transpose()?;
            let result = store.import_pack(imported, browser.as_ref().map(|c| (slot.as_str(), c.seed)), &reporter);
            if result.is_err() {
                if let Some(path) = directory {
                    for name in ["config.json", "seed.owner"] { let _ = fs::remove_file(path.join(name)); }
                    // 新槽位意外多出其他文件时保留目录，不能递归清除未知资料。
                    let _ = fs::remove_dir(path);
                }
            }
            result?;
            super::state(&worker)
        })();
        if let Err(error) = &result { reporter.fail(error); }
        result
    }).await.map_err(|_| "身份导入中断；当前身份未切换，请刷新。".to_string()).and_then(|r| r);
    cleaner.leave(); result
}
