// [INPUT]: 显式单端关联、已保存目标代次、当前端受支持资料和进程检查。
// [OUTPUT]: 仅替换指定端的原槽位归档，不切换工作区或 B 绑定；人工核对与本地元数据分开记录。
// [POS]: D1 单端关联事务；复用认证归档与 SQLite 提交，不建立另一套账号库。
// [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
#[cfg(test)]
mod tests {
    use super::*;
    use crate::identities::{optimization_tests::Fixture, store::Store, Policy};
    use crate::progress::Reporter;
    use std::fs;
    fn browser(f: &Fixture, s: &mut Store) {
        super::super::private::directory(&f.root.join("browser/profile/Default")).unwrap();
        s.db.configure_browser(true, true, None).unwrap();
    }
    fn data(stage: &super::super::archive::Stage, suffix: &str) -> Vec<u8> {
        let n = stage
            .manifest
            .sources
            .iter()
            .position(|s| s.path.ends_with(suffix))
            .unwrap();
        fs::read(stage.path.join(format!("data/{n}"))).unwrap()
    }
    #[test]
    fn attaching_cli_to_browser_slot_keeps_id_browser_and_other_client_archive() {
        let f = Fixture::new();
        let mut s = Store::open(&f.vault, &f.home).unwrap();
        browser(&f, &mut s);
        let p = Policy::default();
        let r = Reporter::channel(None);
        #[cfg(target_os = "macos")]
        f.write(
            "Library/Application Support/Claude/Local Storage/fixture",
            b"original-app",
        );
        s.save_current("browser", &p, &r).unwrap();
        let old = s.records()[0].clone();
        let slot = s.db.browser_slot().unwrap();
        f.login("A", "org", "new-cli");
        #[cfg(target_os = "macos")]
        f.write(
            "Library/Application Support/Claude/Local Storage/fixture",
            b"different-live-app",
        );
        s.associate_client(&old.id, Client::Cli, true, &r).unwrap();
        assert_eq!(s.records().len(), 1);
        let saved = s.record(&old.id).unwrap();
        assert_ne!(saved.archive, old.archive);
        assert_eq!(saved.name, old.name);
        assert_eq!(s.db.browser_slot().unwrap(), slot);
        let stage = s.load(&saved).unwrap();
        assert!(String::from_utf8(data(&stage, ".claude/.credentials.json"))
            .unwrap()
            .contains("new-cli"));
        #[cfg(target_os = "macos")]
        {
            let n = stage
                .manifest
                .sources
                .iter()
                .position(|s| s.path.ends_with("Local Storage"))
                .unwrap();
            assert_eq!(
                fs::read(stage.path.join(format!("data/{n}/fixture"))).unwrap(),
                b"original-app"
            );
        }
        assert_eq!(f.token(), "new-cli");
    }
    #[test]
    fn mismatched_account_unknown_cli_and_process_conflict_preserve_original() {
        let f = Fixture::new();
        let mut s = Store::open(&f.vault, &f.home).unwrap();
        browser(&f, &mut s);
        f.login("A", "org", "old");
        let r = Reporter::channel(None);
        s.save_current("A", &Policy::default(), &r).unwrap();
        let old = s.records()[0].clone();
        f.login("B", "org", "foreign");
        assert!(s.associate_client(&old.id, Client::Cli, true, &r).is_err());
        f.write(".claude.json", b"{}");
        assert!(s.associate_client(&old.id, Client::Cli, true, &r).is_err());
        f.login("A", "org", "new");
        s.process_probe = Some(Box::new(|| Err("fixture active".into())));
        assert!(s.associate_client(&old.id, Client::Cli, true, &r).is_err());
        assert_eq!(s.record(&old.id).unwrap().archive, old.archive);
        assert_eq!(f.token(), "new");
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn desktop_update_preserves_archived_cli_and_does_not_absorb_live_foreign_cli() {
        let f = Fixture::new();
        let mut s = Store::open(&f.vault, &f.home).unwrap();
        browser(&f, &mut s);
        f.login("A", "org", "a-cli");
        f.write(
            "Library/Application Support/Claude/Local Storage/fixture",
            b"old-app",
        );
        let r = Reporter::channel(None);
        s.save_current("A", &Policy::default(), &r).unwrap();
        let old = s.records()[0].clone();
        f.login("B", "other-org", "foreign-cli");
        f.write(
            "Library/Application Support/Claude/Local Storage/fixture",
            b"new-app",
        );
        assert!(s
            .associate_client(&old.id, Client::Desktop, false, &r)
            .is_err());
        s.associate_client(&old.id, Client::Desktop, true, &r)
            .unwrap();
        let saved = s.record(&old.id).unwrap();
        let stage = s.load(&saved).unwrap();
        assert!(String::from_utf8(data(&stage, ".claude/.credentials.json"))
            .unwrap()
            .contains("a-cli"));
        assert!(
            !String::from_utf8(data(&stage, ".claude/.credentials.json"))
                .unwrap()
                .contains("foreign-cli")
        );
        let n = stage
            .manifest
            .sources
            .iter()
            .position(|s| s.path.ends_with("Local Storage"))
            .unwrap();
        assert_eq!(
            fs::read(stage.path.join(format!("data/{n}/fixture"))).unwrap(),
            b"new-app"
        );
        assert_eq!(saved.account_key, old.account_key);
        assert_eq!(f.token(), "foreign-cli");
    }
    #[test]
    fn failed_summary_commit_retains_old_generation_and_browser_selection() {
        let f = Fixture::new();
        let mut s = Store::open(&f.vault, &f.home).unwrap();
        browser(&f, &mut s);
        f.login("A", "org", "old-cli");
        let r = Reporter::channel(None);
        s.save_current("A", &Policy::default(), &r).unwrap();
        let old = s.records()[0].clone();
        let bytes = fs::read(f.vault.join(&old.archive)).unwrap();
        let slot = s.db.browser_slot().unwrap();
        f.login("A", "org", "new-cli");
        s.db.connection.execute_batch("CREATE TRIGGER fail_association BEFORE INSERT ON identity_summaries BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
        assert!(s.associate_client(&old.id, Client::Cli, true, &r).is_err());
        assert_eq!(s.record(&old.id).unwrap().archive, old.archive);
        assert_eq!(fs::read(f.vault.join(&old.archive)).unwrap(), bytes);
        assert_eq!(s.db.browser_slot().unwrap(), slot);
        assert_eq!(f.token(), "new-cli");
        s.db.connection
            .execute_batch("DROP TRIGGER fail_association")
            .unwrap();
        s.associate_client(&old.id, Client::Cli, true, &r).unwrap();
        assert_eq!(s.records().len(), 1);
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn desktop_association_does_not_read_other_live_client_and_preserves_archived_links() {
        use std::os::unix::fs::symlink;
        let f = Fixture::new();
        let mut s = Store::open(&f.vault, &f.home).unwrap();
        browser(&f, &mut s);
        f.login("A", "org", "old-cli");
        f.write(".claude/skills/custom/value", b"old-skill");
        symlink(
            "/nonexistent/fixture",
            f.home.join(".claude/skills/custom/link"),
        )
        .unwrap();
        f.write(
            "Library/Application Support/Claude/Local Storage/fixture",
            b"old-app",
        );
        let r = Reporter::channel(None);
        let p = Policy {
            skills: false,
            ..Policy::default()
        };
        s.save_current("A", &p, &r).unwrap();
        let old = s.records()[0].clone();
        fs::remove_file(f.home.join(".claude/.credentials.json")).unwrap();
        symlink(
            "/nonexistent/other-client",
            f.home.join(".claude/.credentials.json"),
        )
        .unwrap();
        f.write(
            "Library/Application Support/Claude/Local Storage/fixture",
            b"new-app",
        );
        s.associate_client(&old.id, Client::Desktop, true, &r)
            .unwrap();
        let stage = s.load(&s.record(&old.id).unwrap()).unwrap();
        let n = stage
            .manifest
            .sources
            .iter()
            .position(|v| v.path.ends_with(".claude/skills"))
            .unwrap();
        assert_eq!(
            fs::read_link(stage.path.join(format!("data/{n}/custom/link"))).unwrap(),
            std::path::PathBuf::from("/nonexistent/fixture")
        );
        assert!(String::from_utf8(data(&stage, ".claude/.credentials.json"))
            .unwrap()
            .contains("old-cli"));
    }
}
use super::{
    archive::{walk, Manifest},
    codec, compression,
    database::Record,
    private,
    store::{problem, Store},
};
use crate::{
    cleaner::identity,
    progress::{Progress, Reporter},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, path::Path};
use tauri::{ipc::Channel, Manager};
#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Client {
    Cli,
    Desktop,
}
impl Client {
    fn desktop(self) -> bool {
        self == Self::Desktop
    }
    fn name(self) -> &'static str {
        if self.desktop() {
            "desktop"
        } else {
            "cli"
        }
    }
}
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ClientCheck {
    pub source: String,
    pub checked_at: i64,
}
impl Store {
    pub(super) fn snapshot_client_keys(&self, desktop: bool) -> Result<Vec<Value>, String> {
        #[cfg(test)]
        if identity::root().ok().as_ref() != Some(&self.home) {
            return Ok(Vec::new());
        }
        crate::native::identity_snapshot_client(desktop)
    }
    fn association_target(&self, id: &str) -> Result<(), String> {
        if self.db.get("recovery")?.is_some() {
            return Err("请先恢复未完成的身份切换。".into());
        }
        let link = self.db.browser_link()?;
        if link.enabled {
            if link.current_id.as_deref() != Some(id) {
                return Err("请先选择目标档案对应的 B 资料；未关联任何端。".into());
            }
        } else if self.db.get("active")?.as_deref() != Some(id)
            && super::catalog::live_key(&self.home, &self.key)?
                .and_then(|k| self.db.by_account(&k).ok().flatten())
                .is_none_or(|r| r.id != id)
        {
            return Err("请先选择目标本地档案；未关联任何端。".into());
        }
        Ok(())
    }
    pub(super) fn associate_client(
        &mut self,
        id: &str,
        client: Client,
        confirmed: bool,
        reporter: &Reporter,
    ) -> Result<(), String> {
        if !confirmed {
            return Err("请先核对该端账号与组织，并确认人工维护窗口。".into());
        }
        self.association_target(id)?;
        self.stopped()?;
        let old = self.record(id)?;
        let previous = self.load(&old)?;
        let account = if client == Client::Cli {
            let key = super::catalog::live_key(&self.home, &self.key)?
                .ok_or("CLI 账号元数据未知，未关联；原有完整备份仍可使用。")?;
            if old
                .account_key
                .as_deref()
                .filter(|k| !k.is_empty())
                .is_some_and(|k| k != key)
                || self.db.by_account(&key)?.is_some_and(|r| r.id != id)
            {
                return Err("CLI 账号或组织与目标不符，或已属于另一档案；未关联。".into());
            }
            Some(key)
        } else {
            old.account_key.clone()
        };
        let (temporary, current) =
            self.save_client_snapshot(&old.name, &old.policy, reporter, client.desktop())?;
        fs::remove_file(self.vault.join(&temporary.archive)).map_err(problem)?;
        if temporary.empty {
            return Err("该端未发现受支持资料，未覆盖原档案。".into());
        }
        let selected = |source: &identity::Source| {
            identity::source_is_cli(&self.home, source) != client.desktop()
        };
        let payloads: Vec<_> = previous
            .manifest
            .sources
            .iter()
            .enumerate()
            .filter(|(_, s)| !selected(s))
            .map(|(n, s)| {
                (
                    s.clone(),
                    previous.path.join(format!("data/{n}")),
                    previous.manifest.fingerprints[n].clone(),
                )
            })
            .chain(current.manifest.sources.iter().enumerate().map(|(n, s)| {
                (
                    s.clone(),
                    current.path.join(format!("data/{n}")),
                    current.manifest.fingerprints[n].clone(),
                )
            }))
            .collect();
        let service_selected = |v: &Value| {
            if client.desktop() {
                v["service"] == "Claude Safe Storage"
            } else {
                v["service"] == "Claude Code-credentials" || v["service"] == "Claude Code"
            }
        };
        let credentials = previous
            .manifest
            .credentials
            .iter()
            .filter(|v| !service_selected(v))
            .chain(current.manifest.credentials.iter())
            .cloned()
            .collect();
        let mut checks = previous.manifest.client_checks.clone();
        checks.insert(
            client.name().into(),
            ClientCheck {
                source: if client == Client::Cli {
                    "local_metadata"
                } else {
                    "user_confirmed"
                }
                .into(),
                checked_at: super::profile::now_ms(),
            },
        );
        let manifest = Manifest {
            version: previous.manifest.version.max(current.manifest.version),
            id: id.into(),
            home: self.home.clone(),
            platform: std::env::consts::OS.into(),
            policy: old.policy.clone(),
            sources: payloads.iter().map(|x| x.0.clone()).collect(),
            credentials,
            fingerprints: payloads.iter().map(|x| x.2.clone()).collect(),
            account_key: account.clone(),
            client_checks: checks,
        };
        let generation = super::new_id()?;
        let pending = self.vault.join(format!(".pending-{generation}"));
        let destination = self.vault.join(format!("{generation}.baga"));
        let result = (|| {
            let seal = codec::Seal::new(private::create(&pending)?, &self.key).map_err(problem)?;
            let mut tar = tar::Builder::new(compression::encode(seal).map_err(problem)?);
            for (n, (_, path, _)) in payloads.iter().enumerate() {
                self.stopped()?;
                let base = path.parent().ok_or("身份路径无效。")?;
                let archive = std::path::PathBuf::from(format!("data/{n}"));
                walk(path, base, &mut |entry, relative, directory| {
                    self.during_io()?;
                    let suffix = relative
                        .strip_prefix(path.file_name().unwrap())
                        .map_err(problem)?;
                    let target = if suffix.as_os_str().is_empty() {
                        archive.clone()
                    } else {
                        archive.join(suffix)
                    };
                    if let Some(link) = private::link_target(entry)? {
                        let mut header = tar::Header::new_gnu();
                        header.set_entry_type(tar::EntryType::Symlink);
                        header.set_size(0);
                        header.set_mode(0o777);
                        tar.append_link(&mut header, target, link).map_err(problem)
                    } else if directory {
                        tar.append_dir(target, entry).map_err(problem)
                    } else {
                        tar.append_file(target, &mut fs::File::open(entry).map_err(problem)?)
                            .map_err(problem)
                    }
                })?;
                reporter.log(
                    "archive",
                    "running",
                    "正在更新指定端，保留档案中其他端的原资料。",
                    n + 1,
                    Some(payloads.len()),
                );
            }
            let bytes = serde_json::to_vec(&manifest).map_err(problem)?;
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o600);
            header.set_cksum();
            tar.append_data(&mut header, Path::new("manifest.json"), bytes.as_slice())
                .map_err(problem)?;
            let seal = tar
                .into_inner()
                .map_err(problem)?
                .finish()
                .map_err(problem)?;
            let file = seal.finish().map_err(problem)?;
            file.sync_all().map_err(problem)?;
            drop(file);
            fs::rename(&pending, &destination).map_err(problem)?;
            private::sync_parent(&destination);
            let record = Record {
                id: old.id.clone(),
                name: old.name.clone(),
                archive: format!("{generation}.baga"),
                saved_at: super::profile::now_ms() as u64,
                bytes: fs::metadata(&destination).map_err(problem)?.len(),
                policy: old.policy.clone(),
                empty: false,
                account_key: account.clone(),
            };
            let verified = self.load(&record)?;
            // The user keeps clients stopped. Observed changes or unknown process queries abort before pointer commit.
            self.stopped()?;
            self.association_target(id)?;
            for (source, stamp) in current
                .manifest
                .sources
                .iter()
                .zip(&current.manifest.fingerprints)
            {
                if identity::fingerprint(&source.path)? != *stamp {
                    return Err("该端资料在关联期间发生变化；未提交。".into());
                }
            }
            if credential_signature(&self.snapshot_client_keys(client.desktop())?)?
                != credential_signature(&current.manifest.credentials)?
            {
                return Err("该端凭据在关联期间发生变化；未提交。".into());
            }
            self.stopped()?;
            self.db
                .backup(&record, &super::profile::summarize_stage(&verified), false)?;
            reporter.log(
                "commit",
                "ok",
                "指定端资料已更新到原档案；其他端、工作区与 B 绑定保持不变。",
                1,
                Some(1),
            );
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&pending);
            let _ = fs::remove_file(&destination);
        } else {
            let _ = self.collect_generations();
        }
        result
    }
}
fn credential_signature(values: &[Value]) -> Result<Vec<String>, String> {
    let mut rows = values
        .iter()
        .map(|v| {
            serde_json::to_string(&(v.get("service"), v.get("account"), v.get("data")))
                .map_err(problem)
        })
        .collect::<Result<Vec<_>, _>>()?;
    rows.sort();
    Ok(rows)
}
#[tauri::command]
pub async fn identities_associate(
    app: tauri::AppHandle,
    id: String,
    client: Client,
    confirmed: bool,
    on_progress: Channel<Progress>,
) -> Result<super::State, String> {
    if !super::valid_id(&id) {
        return Err("身份编号无效。".into());
    }
    let cleaner = app.state::<crate::cleaner::Cleaner>();
    cleaner.enter()?;
    let worker = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let reporter = Reporter::channel(Some(on_progress));
        let result = (|| {
            let mut store = Store::open(&super::vault(&worker)?, &identity::root()?)?;
            store.associate_client(&id, client, confirmed, &reporter)?;
            super::state(&worker)
        })();
        if let Err(e) = &result {
            reporter.fail(e);
        }
        result
    })
    .await
    .map_err(|_| "单端关联被中断；请刷新核对档案。".to_string())
    .and_then(|x| x);
    cleaner.leave();
    result
}
