// [INPUT]: 保险库主密钥、已认证的归档暂存（Stage）与 CCBaga 代为续期后的 Claude Code OAuth 对象。
// [OUTPUT]: 每个已存身份至多一个绑定到具体归档代次的加密凭据侧车 `<id>-<generation>.cred`；load 时覆盖暂存中的 claudeAiOauth。
// [POS]: 不可变归档之外唯一可变的凭据位置；只含 CLI OAuth，不含 Desktop/MCP/插件秘密，不进入 SQLite、UI 或日志。
// [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
use super::{archive::Stage, codec, database::Record, private, store::problem};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;

const SERVICE: &str = "Claude Code-credentials";
const OAUTH: &str = "claudeAiOauth";
const MAX_BYTES: u64 = 256 * 1024;

#[derive(Serialize, Deserialize)]
struct Body {
    id: String,
    archive: String,
    updated_at: i64,
    oauth: Value,
}

fn generation(archive: &str) -> Option<&str> {
    archive.strip_suffix(".baga").filter(|g| super::valid_id(g))
}
fn path(vault: &Path, record: &Record) -> Option<PathBuf> {
    if !super::valid_id(&record.id) {
        return None;
    }
    Some(vault.join(format!("{}-{}.cred", record.id, generation(&record.archive)?)))
}
/// `(identity id, generation)` encoded in a sidecar file name; anything else is not ours.
pub(super) fn parse_name(name: &str) -> Option<(&str, &str)> {
    let (id, rest) = name.strip_suffix(".cred").or_else(|| name.strip_suffix(".usage"))?.split_once('-')?;
    (super::valid_id(id) && super::valid_id(rest)).then_some((id, rest))
}

fn open_sealed(path: &Path, key: &[u8]) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
    if fs::symlink_metadata(path).is_err() {
        return Ok(None);
    }
    private::safe(path)?;
    let mut plain = Zeroizing::new(Vec::new());
    codec::Open::new(fs::File::open(path).map_err(problem)?, key)
        .map_err(|_| "身份凭据侧车无法验证。")?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut plain)
        .map_err(|_| "身份凭据侧车无法验证。")?;
    if plain.len() as u64 > MAX_BYTES {
        return Err("身份凭据侧车无法验证。".into());
    }
    Ok(Some(plain))
}
fn write_sealed(path: &Path, key: &[u8], body: &[u8]) -> Result<(), String> {
    let mut seal = codec::Seal::new(Vec::new(), key).map_err(problem)?;
    seal.write_all(body).map_err(problem)?;
    let sealed = Zeroizing::new(seal.finish().map_err(problem)?);
    private::write(path, &sealed)
}

/// Latest CLI OAuth object for this exact archive generation, if CCBaga renewed it.
pub(super) fn read(vault: &Path, key: &[u8], record: &Record) -> Result<Option<Zeroizing<String>>, String> {
    let Some(path) = path(vault, record) else { return Ok(None) };
    let Some(plain) = open_sealed(&path, key)? else { return Ok(None) };
    let body: Body = serde_json::from_slice(&plain).map_err(|_| "身份凭据侧车无法验证。")?;
    if body.id != record.id || body.archive != record.archive || !body.oauth.is_object() {
        return Err("身份凭据侧车不属于该身份。".into());
    }
    Ok(Some(Zeroizing::new(body.oauth.to_string())))
}

/// Atomically replaces the sidecar. Callers that rotated a refresh token must treat an error
/// here as "the account may need a new sign-in": the server already invalidated the old one.
pub(super) fn write(vault: &Path, key: &[u8], record: &Record, oauth: &Value) -> Result<(), String> {
    let path = path(vault, record).ok_or("身份归档编号无效。")?;
    if !oauth.is_object() {
        return Err("身份凭据格式不受支持。".into());
    }
    let body = Zeroizing::new(
        serde_json::to_vec(&Body {
            id: record.id.clone(),
            archive: record.archive.clone(),
            updated_at: super::profile::now_ms(),
            oauth: oauth.clone(),
        })
        .map_err(problem)?,
    );
    write_sealed(&path, key, &body)
}

/// Read-only usage material for a saved generation: the Desktop access token (never renewed)
/// and whether the archive has a CLI OAuth at all, so later reads never decrypt the archive.
#[derive(Serialize, Deserialize)]
pub(super) struct UsageSeed {
    id: String,
    archive: String,
    pub desktop_token: Option<String>,
    pub desktop_expires_at: Option<i64>,
    pub desktop_plan: Option<String>,
    pub cli_present: bool,
}
impl Drop for UsageSeed {
    fn drop(&mut self) {
        if let Some(token) = self.desktop_token.as_mut() {
            zeroize::Zeroize::zeroize(token);
        }
    }
}
fn seed_path(vault: &Path, id: &str, archive: &str) -> Option<PathBuf> {
    if !super::valid_id(id) {
        return None;
    }
    Some(vault.join(format!("{id}-{}.usage", generation(archive)?)))
}
pub(super) fn seed_from_stage(stage: &Stage, record: &Record) -> UsageSeed {
    let account = stage_account(stage);
    let desktop = super::desktop_token::from_stage(stage, account.as_deref());
    UsageSeed {
        id: record.id.clone(),
        archive: record.archive.clone(),
        desktop_expires_at: desktop.as_ref().map(|d| d.expires_at),
        desktop_plan: desktop.as_ref().and_then(|d| d.plan.clone()),
        desktop_token: desktop.map(|d| d.access.to_string()),
        cli_present: stage_oauth(stage).ok().flatten().is_some(),
    }
}
pub(super) fn write_seed(vault: &Path, key: &[u8], seed: &UsageSeed) -> Result<(), String> {
    let path = seed_path(vault, &seed.id, &seed.archive).ok_or("身份归档编号无效。")?;
    let body = Zeroizing::new(serde_json::to_vec(seed).map_err(problem)?);
    write_sealed(&path, key, &body)
}
pub(super) fn read_seed(vault: &Path, key: &[u8], record: &Record) -> Result<Option<UsageSeed>, String> {
    let Some(path) = seed_path(vault, &record.id, &record.archive) else { return Ok(None) };
    let Some(plain) = open_sealed(&path, key)? else { return Ok(None) };
    let seed: UsageSeed = serde_json::from_slice(&plain).map_err(|_| "身份凭据侧车无法验证。")?;
    if seed.id != record.id || seed.archive != record.archive {
        return Err("身份凭据侧车不属于该身份。".into());
    }
    Ok(Some(seed))
}
/// CLI account UUID recorded in the stage's `.claude.json` member, used to match Desktop tokens.
fn stage_account(stage: &Stage) -> Option<String> {
    let target = stage.manifest.home.join(".claude.json");
    let n = stage.manifest.sources.iter().position(|s| s.path == target)?;
    let text = Zeroizing::new(fs::read(stage.path.join(format!("data/{n}"))).ok()?);
    let value: Value = serde_json::from_slice(&text).ok()?;
    value["oauthAccount"]["accountUuid"].as_str().map(str::to_string)
}

/// The CLI OAuth object stored in an authenticated stage (keychain item on macOS,
/// `.claude/.credentials.json` member elsewhere).
pub(super) fn stage_oauth(stage: &Stage) -> Result<Option<Value>, String> {
    for item in &stage.manifest.credentials {
        if item["service"] != SERVICE {
            continue;
        }
        if let Some(oauth) = decode_item(item)?.get(OAUTH).filter(|v| v.is_object()) {
            return Ok(Some(oauth.clone()));
        }
    }
    if let Some(data) = credentials_member(stage) {
        let value: Value = serde_json::from_slice(&Zeroizing::new(fs::read(data).map_err(problem)?))
            .map_err(problem)?;
        if let Some(oauth) = value.get(OAUTH).filter(|v| v.is_object()) {
            return Ok(Some(oauth.clone()));
        }
    }
    Ok(None)
}

fn decode_item(item: &Value) -> Result<Value, String> {
    let bytes = Zeroizing::new(
        STANDARD
            .decode(item["data"].as_str().ok_or("身份凭据无法处理。")?)
            .map_err(|_| "身份凭据无法处理。")?,
    );
    serde_json::from_slice(&bytes).map_err(|_| "身份凭据无法处理。".into())
}
fn credentials_member(stage: &Stage) -> Option<PathBuf> {
    let target = stage.manifest.home.join(".claude/.credentials.json");
    stage
        .manifest
        .sources
        .iter()
        .position(|s| s.path == target && !s.keys.is_empty())
        .map(|n| stage.path.join(format!("data/{n}")))
}

/// Replaces only `claudeAiOauth` in the stage with the sidecar copy; MCP/plugin secrets,
/// Desktop items and every other key stay exactly as archived.
pub(super) fn overlay(stage: &mut Stage, vault: &Path, key: &[u8], record: &Record) -> Result<(), String> {
    let Some(text) = read(vault, key, record)? else { return Ok(()) };
    let oauth: Value = serde_json::from_str(&text).map_err(problem)?;
    let mut applied = false;
    for item in &mut stage.manifest.credentials {
        if item["service"] != SERVICE {
            continue;
        }
        let mut value = decode_item(item)?;
        let Some(object) = value.as_object_mut() else { continue };
        object.insert(OAUTH.into(), oauth.clone());
        let bytes = Zeroizing::new(serde_json::to_vec(&value).map_err(problem)?);
        item["data"] = Value::String(STANDARD.encode(&*bytes));
        applied = true;
    }
    if let Some(data) = credentials_member(stage) {
        let mut value: Value =
            serde_json::from_slice(&Zeroizing::new(fs::read(&data).map_err(problem)?)).map_err(problem)?;
        if let Some(object) = value.as_object_mut() {
            object.insert(OAUTH.into(), oauth);
            let bytes = Zeroizing::new(serde_json::to_vec(&value).map_err(problem)?);
            fs::write(&data, &*bytes).map_err(problem)?;
            applied = true;
        }
    }
    if !applied {
        // A sidecar without a CLI credential slot in its archive cannot be restored safely.
        return Err("身份凭据侧车与归档不一致。".into());
    }
    Ok(())
}

/// Removes sidecars whose identity is gone or whose archive generation was superseded.
pub(super) fn collect(vault: &Path, records: &[Record]) {
    let Ok(entries) = fs::read_dir(vault) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((id, generation_id)) = parse_name(&name) else { continue };
        let live = records
            .iter()
            .any(|r| r.id == id && generation(&r.archive) == Some(generation_id));
        if !live && private::safe(&entry.path()).is_ok() {
            let _ = fs::remove_file(entry.path());
        }
    }
}
