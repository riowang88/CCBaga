// [INPUT]: 有界 OAuth 账号元数据、本机密钥、已认证旧归档与 SQLite 目录。
// [OUTPUT]: 本机 HMAC 账号索引、与当前浏览器选择一致的本地槽位、安全资料目录存在性、旧索引迁移与归档回收；不据此声称登录有效。
// [POS]: 云账号只按 UUID/组织（缺失时邮箱/组织）匹配；本地槽位连续性独立于认证判断，不按名字合并未知账号。
// [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
use super::{database::{Database, Record}, private, store::Store};
use ring::hmac;
use serde_json::Value;
use std::{fs, io::Read, path::Path};
pub(super) fn archive_name(name: &str) -> bool {
    name.strip_suffix(".baga").is_some_and(super::valid_id)
}
pub(super) fn account_key(value: &Value, key: &[u8]) -> Option<String> {
    let account = &value["oauthAccount"];
    let field = |name: &str| {
        account[name]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty() && s.len() <= 254 && !s.chars().any(char::is_control))
    };
    let org = field("organizationUuid")
        .or_else(|| field("organizationName"))
        .unwrap_or("");
    let identity = if let Some(uuid) = field("accountUuid") {
        ("uuid", uuid.to_string(), org)
    } else {
        let email = field("emailAddress")?.to_lowercase();
        if !email.contains('@') {
            return None;
        }
        ("email", email, org)
    };
    let bytes = serde_json::to_vec(&identity).ok()?;
    let tag = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, key), &bytes);
    Some(tag.as_ref().iter().map(|b| format!("{b:02x}")).collect())
}
fn file_key(path: &Path, key: &[u8]) -> Result<Option<String>, String> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("身份账号索引无法读取。".into()),
    };
    private::parents(path.parent().ok_or("身份路径无效。")?)?;
    private::safe(path)?;
    if !meta.is_file() || meta.len() > 8 * 1024 * 1024 {
        return Ok(None);
    }
    let mut bytes = zeroize::Zeroizing::new(Vec::new());
    fs::File::open(path)
        .map_err(|_| "身份账号索引无法读取。")?
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "身份账号索引无法读取。")?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Ok(None);
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| "身份配置不是有效 JSON。")?;
    Ok(account_key(&value, key))
}
pub(super) fn live_key(home: &Path, key: &[u8]) -> Result<Option<String>, String> {
    file_key(&home.join(".claude.json"), key)
}
// A selected local archive is a stable workspace slot, not a recognized cloud
// account. Never use this fallback to merge names or overwrite a known account
// whose metadata has disappeared; an observed account always takes precedence.
pub(super) fn snapshot_record(db: &Database, live_key: Option<&str>) -> Result<Option<Record>, String> {
    if let Some(key) = live_key {
        return db.by_account(key);
    }
    let active = db.get("active")?;
    let records = db.records()?;
    let local = |record: &&Record| !record.empty && record.account_key.as_deref().is_none_or(str::is_empty);
    let browser_slot = if db.browser_link()?.enabled { Some(db.browser_slot()?) } else { None };
    if let Some(record) = records.iter().filter(local).find(|record| active.as_deref() == Some(record.id.as_str())) {
        let bound = db.browser_for(&record.id)?;
        // 联动重开后新资料不属于旧 active；保留旧档案，另存当前资料。
        if browser_slot.as_ref().is_none_or(|slot| bound.as_ref().is_none_or(|bound| bound == slot)) {
            return Ok(Some(record.clone()));
        }
    }
    // 首次原地保存不改变 CLI 的 active；由当前浏览器的独占绑定找回该本地档案。
    if let Some(slot) = browser_slot {
        for record in records.iter().filter(local) {
            if db.browser_for(&record.id)?.as_deref() == Some(slot.as_str()) { return Ok(Some(record.clone())); }
        }
    }
    Ok(None)
}
pub(super) fn browser_profile_present(vault: &Path, db: &Database) -> Result<bool, String> {
    if !db.browser_link()?.enabled { return Ok(false); }
    let root = vault.parent().ok_or("应用目录不可用。")?.join("browser");
    let slot = db.browser_slot()?;
    let profile = if slot == "legacy" { root.join("profile") } else { root.join("contexts").join(slot).join("profile") };
    private::parents(&profile)?;
    match fs::symlink_metadata(profile) {
        Ok(meta) => Ok(meta.is_dir()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err("浏览器资料无法读取。".into()),
    }
}
pub(super) fn current_id(
    vault: &Path,
    home: &Path,
    db: &Database,
) -> Result<Option<String>, String> {
    Ok(current_ids(vault, home, db)?.0)
}
// Cloud-account match and the actual snapshot destination have different meanings.
pub(super) fn current_ids(
    vault: &Path,
    home: &Path,
    db: &Database,
) -> Result<(Option<String>, Option<String>), String> {
    let key_path = vault.join("key.bin");
    if !key_path.exists() {
        return Ok((None, None));
    }
    private::parents(vault)?;
    private::safe(&key_path)?;
    if fs::metadata(&key_path)
        .map_err(|_| "身份密钥无法读取。")?
        .len()
        != 32
    {
        return Err("身份密钥无效。".into());
    }
    let key = zeroize::Zeroizing::new(fs::read(key_path).map_err(|_| "身份密钥无法读取。")?);
    let live = live_key(home, &key)?;
    let account = match live.as_deref() {
        Some(key) => db.by_account(key)?.map(|r| r.id),
        None => None,
    };
    let workspace = snapshot_record(db, live.as_deref())?.map(|r| r.id);
    Ok((account, workspace))
}
impl Store {
    pub(super) fn index_legacy(&self) -> Result<(), String> {
        for record in self
            .db
            .records()?
            .into_iter()
            .filter(|r| r.account_key.is_none())
        {
            // 损坏的旧副本不自动删除；用户仍可单独删除或保留排查。
            let Ok(stage) = self.load(&record) else {
                continue;
            };
            if record.account_key.is_none() {
                let key = if let Some(key) = &stage.manifest.account_key {
                    Some(key.clone())
                } else if let Some(n) = stage
                    .manifest
                    .sources
                    .iter()
                    .position(|s| s.path == self.home.join(".claude.json"))
                {
                    file_key(&stage.path.join(format!("data/{n}")), &self.key)?
                } else {
                    None
                };
                self.db.index(&record.id, key.as_deref().unwrap_or(""))?;
            }
            if !record.empty {
                let _ = self
                    .db
                    .set_summary(&record.id, &super::profile::summarize_stage(&stage));
            }
        }
        Ok(())
    }
    pub(super) fn collect_generations(&self) -> Result<(), String> {
        let records = self.db.records()?;
        for entry in fs::read_dir(&self.vault).map_err(|_| "身份保险库无法读取。")? {
            let entry = entry.map_err(|_| "身份保险库无法读取。")?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if (archive_name(&name) && !records.iter().any(|r| r.archive == name))
                || name.strip_prefix(".pending-").is_some_and(super::valid_id)
            {
                private::safe(&entry.path())?;
                fs::remove_file(entry.path()).map_err(|_| "旧身份归档回收失败。")?;
            }
        }
        private::sync_parent(&self.vault.join("identities.sqlite"));
        Ok(())
    }
}
