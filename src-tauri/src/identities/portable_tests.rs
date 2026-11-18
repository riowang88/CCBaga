// [INPUT]: 可丢弃身份目录、合成令牌/密钥、认证迁移流和 SQLite 故障触发器。
// [OUTPUT]: 同 OS 重定位/本机重封存、密码/成员/长度/认证拒绝、一次性文件选择与失败不覆写回归。
// [POS]: portable 的离线攻防证明；不操作真实 Claude、钥匙串、浏览器或网络。
// [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
use super::*;
use crate::identities::{optimization_tests::Fixture, Policy};
const PASSWORD: &str = "synthetic-transfer-password";
fn saved() -> (Fixture, Store, Record) {
    let fixture = Fixture::new();
    fixture.login("portable-A", "organization", "synthetic-token");
    fixture.write(".claude/history.jsonl", b"synthetic-history");
    fixture.write(".claude/skills/private-keep", b"not-exported");
    let mut store = Store::open(&fixture.vault, &fixture.home).unwrap();
    store.save_current("Portable A", &Policy::default(), &Reporter::channel(None)).unwrap();
    let record = store.records()[0].clone();
    (fixture, store, record)
}
fn metadata(store: &Store, record: &Record) -> Metadata {
    let stage = store.load(record).unwrap();
    let mut manifest: Manifest = serde_json::from_value(serde_json::to_value(&stage.manifest).unwrap()).unwrap();
    manifest.credentials = credentials(&manifest.credentials, &manifest.policy).unwrap();
    manifest.account_key = None;
    Metadata { version: 1, name: record.name.clone(), manifest, browser: None }
}
fn authenticated_tar(raw: &[u8]) -> Vec<u8> {
    let salt = codec::random::<32>().unwrap();
    let key = password_key(PASSWORD, &salt).unwrap();
    let seal = codec::Seal::new(Vec::new(), key.as_ref()).unwrap();
    let mut encoder = compression::encode(seal).unwrap();
    encoder.write_all(raw).unwrap();
    let ciphertext = encoder.finish().unwrap().finish().unwrap();
    let mut bytes = Vec::from(MAGIC.as_slice());
    bytes.extend(ITERATIONS.to_be_bytes()); bytes.extend(salt); bytes.extend(ciphertext); bytes
}
fn custom_pack(metadata: &serde_json::Value, entries: &[(&str, tar::EntryType, &[u8])], trailing: &[u8]) -> Vec<u8> {
    let mut tar = tar::Builder::new(Vec::new());
    append(&mut tar, Path::new("transfer.json"), &serde_json::to_vec(metadata).unwrap()).unwrap();
    for (path, kind, bytes) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(*kind); header.set_size(bytes.len() as u64); header.set_mode(0o600);
        // 直接构造恶意头，不能借 tar::Builder 的前置路径保护掩盖生产解包漏洞。
        header.as_mut_bytes()[..path.len()].copy_from_slice(path.as_bytes());
        if kind.is_symlink() || kind.is_hard_link() { header.set_link_name("outside").unwrap(); }
        header.set_cksum();
        tar.append(&header, *bytes).unwrap();
    }
    let mut raw = tar.into_inner().unwrap();
    raw.extend(trailing);
    authenticated_tar(&raw)
}
fn read_pack(bytes: &[u8], password: &str, parent: &Path) -> Result<Imported, String> {
    let file = parent.join(format!("{}.bagapack", crate::identities::new_id().unwrap()));
    private::write(&file, bytes).unwrap();
    unpack(selected_file(&file).unwrap(), password, parent, None, &Reporter::channel(None))
}

#[test]
fn portable_round_trip_relocates_and_reencrypts_without_switching_or_exporting_shared_assets() {
    let (origin, source, record) = saved();
    let destination = Fixture::new();
    destination.login("existing-B", "other-org", "keep-live-token");
    let target = Store::open(&destination.vault, &destination.home).unwrap();
    let active = target.db.get("active").unwrap();
    let browser_slot = target.db.browser_slot().unwrap();
    let master = fs::read(destination.vault.join("key.bin")).unwrap();
    let export = origin.root.join("roundtrip.bagapack");
    source.export_pack(&export, PASSWORD, &record.id, None, &Reporter::channel(None)).unwrap();
    let bytes = fs::read(export).unwrap();
    assert!(!bytes.windows(b"synthetic-token".len()).any(|w| w == b"synthetic-token"));
    let imported = read_pack(&bytes, PASSWORD, &destination.root).unwrap();
    assert!(!imported.stage.manifest.sources.iter().any(|s| s.path.ends_with("skills")));
    let row = target.import_pack(imported, None, &Reporter::channel(None)).unwrap();
    assert_ne!(row.id, record.id);
    let stage = target.load(&row).unwrap();
    assert_eq!(stage.manifest.home, destination.home);
    assert!(stage.manifest.sources.iter().all(|s| s.path.starts_with(&destination.home)));
    let history = stage.manifest.sources.iter().position(|s| s.path.ends_with("history.jsonl")).unwrap();
    assert_eq!(fs::read(stage.path.join(format!("data/{history}"))).unwrap(), b"synthetic-history");
    assert_ne!(row.account_key, record.account_key);
    assert_eq!(target.db.get("active").unwrap(), active);
    assert_eq!(target.db.browser_slot().unwrap(), browser_slot);
    assert_eq!(destination.token(), "keep-live-token");
    assert_eq!(fs::read(destination.vault.join("key.bin")).unwrap(), master);
    assert!(codec::Open::new(fs::File::open(destination.vault.join(&row.archive)).unwrap(), &source.key).is_err());
}

#[test]
fn wrong_password_tampering_truncation_and_trailing_ciphertext_do_not_import() {
    let (origin, source, row) = saved();
    let export = origin.root.join("auth.bagapack");
    source.export_pack(&export, PASSWORD, &row.id, None, &Reporter::channel(None)).unwrap();
    let valid = fs::read(export).unwrap();
    assert!(read_pack(&valid, "a-different-password", &origin.root).is_err());
    for position in [0, 9, 30, 75, valid.len() - 1] {
        let mut damaged = valid.clone(); damaged[position] ^= 1;
        assert!(read_pack(&damaged, PASSWORD, &origin.root).is_err());
    }
    assert!(read_pack(&valid[..valid.len() - 16], PASSWORD, &origin.root).is_err());
    let mut extra = valid; extra.push(1);
    assert!(read_pack(&extra, PASSWORD, &origin.root).is_err());
    assert!(!fs::read_dir(&origin.root).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().starts_with(".identity-transfer-")));
}

#[test]
fn unsafe_members_unknown_fields_and_authenticated_hidden_tar_payload_are_rejected() {
    let (fixture, source, row) = saved();
    let mut value = serde_json::to_value(metadata(&source, &row)).unwrap();
    // 先证明最小白名单成员通过，再验证链接/特殊头/重复和尾随载荷的具体拒绝。
    value["manifest"]["sources"] = serde_json::json!([value["manifest"]["sources"][0].clone()]);
    value["manifest"]["fingerprints"] = serde_json::json!([value["manifest"]["fingerprints"][0].clone()]);
    assert!(read_pack(&custom_pack(&value, &[("data/0", tar::EntryType::Regular, b"{}")], &[]), PASSWORD, &fixture.root).is_ok());
    for (path, kind) in [("../outside", tar::EntryType::Regular), ("/absolute", tar::EntryType::Regular),
                         ("key.bin", tar::EntryType::Regular), ("data/99", tar::EntryType::Regular),
                         ("data/0", tar::EntryType::Symlink), ("data/0", tar::EntryType::Link),
                         ("data/0", tar::EntryType::XHeader)] {
        let bytes = custom_pack(&value, &[(path, kind, b"payload")], &[]);
        assert!(read_pack(&bytes, PASSWORD, &fixture.root).is_err(), "{path}");
    }
    let duplicate = custom_pack(&value, &[("data/0", tar::EntryType::Regular, b"{}"), ("data/0", tar::EntryType::Regular, b"{}")], &[]);
    assert!(read_pack(&duplicate, PASSWORD, &fixture.root).is_err());
    let bytes = custom_pack(&value, &[("data/0", tar::EntryType::Regular, b"{}")], b"authenticated-but-hidden-payload");
    assert!(read_pack(&bytes, PASSWORD, &fixture.root).is_err());
    value["manifest"]["unrecognized"] = true.into();
    assert!(read_pack(&custom_pack(&value, &[], &[]), PASSWORD, &fixture.root).is_err());
}

#[test]
fn source_os_and_paths_cannot_escape_or_change_the_destination() {
    let (fixture, source, row) = saved();
    let mut value = serde_json::to_value(metadata(&source, &row)).unwrap();
    value["manifest"]["platform"] = "another-os".into();
    assert!(read_pack(&custom_pack(&value, &[], &[]), PASSWORD, &fixture.root).is_err());
    value["manifest"]["platform"] = std::env::consts::OS.into();
    value["manifest"]["sources"][0]["path"] = fixture.root.join("unrelated").to_string_lossy().as_ref().into();
    assert!(read_pack(&custom_pack(&value, &[], &[]), PASSWORD, &fixture.root).is_err());
    let mut value = serde_json::to_value(metadata(&source, &row)).unwrap();
    value["manifest"]["sources"][0]["unexpected"] = true.into();
    assert!(read_pack(&custom_pack(&value, &[], &[]), PASSWORD, &fixture.root).is_err());
}

#[test]
fn password_bounds_and_fixed_cost_header_prevent_empty_or_attacker_selected_kdf() {
    for value in ["short".to_string(), "a".repeat(1025), "中".repeat(342)] {
        assert!(password_key(&value, &[0; 32]).is_err());
    }
    assert!(password_key("十二个字符必须足够强并且独立", &[0; 32]).is_ok());
    let mut reader = Budget { inner: &b"12345"[..], remaining: 4 };
    assert!(reader.read_to_end(&mut Vec::new()).is_err());
}

#[test]
fn export_refuses_to_overwrite_and_file_grants_are_single_use_expiring_and_kind_bound() {
    let (fixture, source, row) = saved();
    let path = fixture.root.join("中文 空格.bagapack");
    private::write(&path, b"keep-existing").unwrap();
    assert!(source.export_pack(&path, PASSWORD, &row.id, None, &Reporter::channel(None)).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"keep-existing");
    let grants = Transfers::default();
    let ticket = grants.issue(path.clone(), Kind::Import).unwrap();
    assert_eq!(ticket.file_name, "中文 空格.bagapack");
    assert!(grants.take(&ticket.token, Kind::Export).is_err());
    assert!(grants.take("unrelated-token", Kind::Import).is_err());
    assert_eq!(grants.take(&ticket.token, Kind::Import).unwrap().path, path);
    assert!(grants.take(&ticket.token, Kind::Import).is_err());
    let ticket = grants.issue(path, Kind::Import).unwrap();
    grants.grant.lock().unwrap().as_mut().unwrap().created = Instant::now() - TTL;
    assert!(grants.take(&ticket.token, Kind::Import).is_err());
}

#[test]
fn failed_database_commit_and_duplicate_account_preserve_existing_records_files_and_browser() {
    let (origin, source, row) = saved();
    let path = origin.root.join("duplicate.bagapack");
    source.export_pack(&path, PASSWORD, &row.id, None, &Reporter::channel(None)).unwrap();
    let bytes = fs::read(path).unwrap();
    let target = Fixture::new();
    let store = Store::open(&target.vault, &target.home).unwrap();
    let imported = read_pack(&bytes, PASSWORD, &target.root).unwrap();
    let saved = store.import_pack(imported, None, &Reporter::channel(None)).unwrap();
    let baseline = fs::read(target.vault.join(&saved.archive)).unwrap();
    let imported = read_pack(&bytes, PASSWORD, &target.root).unwrap();
    assert!(store.import_pack(imported, None, &Reporter::channel(None)).is_err());
    assert_eq!(store.records().len(), 1);
    assert_eq!(fs::read(target.vault.join(saved.archive)).unwrap(), baseline);
    let another = Fixture::new();
    let fail = Store::open(&another.vault, &another.home).unwrap();
    let active = fail.db.browser_slot().unwrap();
    fail.db.connection.execute_batch("CREATE TRIGGER reject_import BEFORE INSERT ON identity_summaries BEGIN SELECT RAISE(ABORT,'synthetic'); END;").unwrap();
    let imported = read_pack(&bytes, PASSWORD, &another.root).unwrap();
    assert!(fail.import_pack(imported, None, &Reporter::channel(None)).is_err());
    assert!(fail.records().is_empty());
    assert_eq!(fail.db.browser_slot().unwrap(), active);
    assert!(!fs::read_dir(&another.vault).unwrap().any(|e| e.unwrap().path().extension().is_some_and(|x| x == "baga")));
}

#[test]
fn optional_browser_transfers_only_fingerprint_settings_and_never_proxy_dns_or_login_data() {
    let (fixture, source, row) = saved();
    let mut config = crate::browser::config::Config::default();
    config.seed = 123456; config.follow_machine = false; config.language = "ja".into(); config.timezone = "Asia/Tokyo".into();
    config.proxy = "http://private-proxy.example.invalid:8080".into();
    config.dns = serde_json::from_value(serde_json::json!({"mode":"secure","provider":"custom","custom_url":"https://private-resolver.invalid/dns-query"})).unwrap();
    let path = fixture.root.join("browser.bagapack");
    source.export_pack(&path, PASSWORD, &row.id, Some(config), &Reporter::channel(None)).unwrap();
    let target = Fixture::new();
    let imported = read_pack(&fs::read(path).unwrap(), PASSWORD, &target.root).unwrap();
    let config = imported.browser.unwrap().config().unwrap();
    assert_eq!(config.seed, 123456);
    assert_eq!(config.timezone, "Asia/Tokyo");
    assert_eq!(config.language, "ja");
    assert!(!config.follow_machine);
    assert!(config.proxy.is_empty());
    assert_eq!(serde_json::to_value(&config.dns).unwrap()["mode"], "inherit");
    assert!(config.dns.custom_url.is_empty());
}

#[test]
fn gnu_long_paths_round_trip_with_a_tar_budget_that_accounts_for_all_extension_blocks() {
    let (fixture, mut store, _) = saved();
    let relative = format!(".claude/projects/{}/{}/session.jsonl", "a".repeat(120), "b".repeat(120));
    fixture.write(&relative, b"synthetic-long-path-history");
    store.save_current("Long path", &Policy::default(), &Reporter::channel(None)).unwrap();
    let row = store.records()[0].clone();
    let pack = fixture.root.join("long-path.bagapack");
    store.export_pack(&pack, PASSWORD, &row.id, None, &Reporter::channel(None)).unwrap();
    let decoded = read_pack(&fs::read(pack).unwrap(), PASSWORD, &fixture.root).unwrap();
    let n = decoded.stage.manifest.sources.iter().position(|source| source.path == fixture.home.join(".claude/projects")).unwrap();
    assert_eq!(fs::read(decoded.stage.path.join(format!("data/{n}" )).join("a".repeat(120)).join("b".repeat(120)).join("session.jsonl")).unwrap(), b"synthetic-long-path-history");
    // 4097 字节 GNU 长名区需 4608 字节填充；每成员上限足以覆盖扩展头与普通文件填充。
    let overhead = 512 + (MAX_NAME_BYTES as u64 + 1).div_ceil(512) * 512 + 512 + 511;
    assert!(overhead <= 6144);
    assert!(MAX_TAR >= MAX_BYTES + MAX_METADATA + MAX_ENTRIES as u64 * overhead);
}

#[test]
fn oversized_declared_members_and_gnu_extension_are_rejected_before_payload_read() {
    let (fixture, source, row) = saved();
    let value = serde_json::to_value(metadata(&source, &row)).unwrap();
    for (kind, size, expected) in [(tar::EntryType::Regular, MAX_BYTES + 1, limit()),
                                  (tar::EntryType::GNULongName, 1024 * 1024 * 1024, invalid())] {
        let mut tar = tar::Builder::new(Vec::new());
        append(&mut tar, Path::new("transfer.json"), &serde_json::to_vec(&value).unwrap()).unwrap();
        let mut raw = tar.into_inner().unwrap();
        raw.truncate(raw.len() - 1024);
        let mut header = tar::Header::new_gnu();
        header.set_path("data/0").unwrap(); header.set_entry_type(kind); header.set_size(size); header.set_cksum();
        raw.extend(header.as_bytes());
        raw.extend([0; 1024]);
        let error = match read_pack(&authenticated_tar(&raw), PASSWORD, &fixture.root) {
            Err(error) => error,
            Ok(_) => panic!("oversized declared member was accepted"),
        };
        assert_eq!(error, expected);
    }
}

#[test]
fn single_client_local_attestation_is_not_exported_as_destination_evidence() {
    let fixture=Fixture::new();fixture.login("A","org","cli-fixture");let mut store=Store::open(&fixture.vault,&fixture.home).unwrap();store.db.configure_browser(false,false,None).unwrap();let reporter=Reporter::channel(None);store.save_current("A",&Policy::default(),&reporter).unwrap();let id=store.records()[0].id.clone();store.associate_client(&id,super::super::association::Client::Cli,true,&reporter).unwrap();
    assert!(!store.load(&store.record(&id).unwrap()).unwrap().manifest.client_checks.is_empty());
    let path=fixture.root.join("associated.bagapack");store.export_pack(&path,PASSWORD,&id,None,&reporter).unwrap();let imported=read_pack(&fs::read(path).unwrap(),PASSWORD,&fixture.root).unwrap();assert!(imported.stage.manifest.client_checks.is_empty());
}

#[cfg(unix)]
#[test]
fn export_skips_local_links_from_v2_archives_and_imports_as_plain_v1() {
    // 默认保留策略会把 ~/.claude/debug 打进归档；Claude Code 在其中维护 latest 链接。
    let origin = Fixture::new();
    origin.login("portable-L", "organization", "synthetic-token");
    origin.write(".claude/debug/session.txt", b"synthetic-debug");
    std::os::unix::fs::symlink(origin.home.join(".claude/debug/session.txt"), origin.home.join(".claude/debug/latest")).unwrap();
    std::os::unix::fs::symlink("session.txt", origin.home.join(".claude/debug/relative")).unwrap();
    std::os::unix::fs::symlink("/etc/hosts", origin.home.join(".claude/debug/outside")).unwrap();
    let mut source = Store::open(&origin.vault, &origin.home).unwrap();
    source.save_current("Linked", &Policy::default(), &Reporter::channel(None)).unwrap();
    let record = source.records()[0].clone();
    let stage = source.load(&record).unwrap();
    assert_eq!(stage.manifest.version, 2);
    assert_eq!(checked_data(&stage).unwrap().2, 3);
    drop(stage);
    let export = origin.root.join("linked.bagapack");
    source.export_pack(&export, PASSWORD, &record.id, None, &Reporter::channel(None)).unwrap();
    let destination = Fixture::new();
    let target = Store::open(&destination.vault, &destination.home).unwrap();
    let imported = read_pack(&fs::read(export).unwrap(), PASSWORD, &destination.root).unwrap();
    assert_eq!(imported.stage.manifest.version, 1);
    let row = target.import_pack(imported, None, &Reporter::channel(None)).unwrap();
    let stage = target.load(&row).unwrap();
    let debug = stage.manifest.sources.iter().position(|s| s.path.ends_with("debug")).unwrap();
    let folder = stage.path.join(format!("data/{debug}"));
    assert_eq!(fs::read(folder.join("session.txt")).unwrap(), b"synthetic-debug");
    for name in ["latest", "relative", "outside"] { assert!(fs::symlink_metadata(folder.join(name)).is_err(), "{name} must not travel"); }
}

#[test]
fn failed_export_returns_the_ticket_and_default_names_are_sanitized() {
    let fixture = Fixture::new();
    let path = fixture.root.join("retry.bagapack");
    let grants = Transfers::default();
    let ticket = grants.issue(path.clone(), Kind::Export).unwrap();
    let grant = grants.take(&ticket.token, Kind::Export).unwrap();
    assert!(grants.take(&ticket.token, Kind::Export).is_err());
    grants.restore(grant);
    let grant = grants.take(&ticket.token, Kind::Export).unwrap();
    // 目标已经落盘时不归还：不能让同一张票据覆盖已有文件。
    private::write(&path, b"written").unwrap();
    grants.restore(grant);
    assert!(grants.take(&ticket.token, Kind::Export).is_err());
    assert_eq!(suggested_name(Some("工作账号 2026-11-18")), "工作账号 2026-11-18.bagapack");
    assert_eq!(suggested_name(Some("../../etc/passwd\u{0}")), "etcpasswd.bagapack");
    assert_eq!(suggested_name(Some("///")), "identity.bagapack");
    assert_eq!(suggested_name(None), "identity.bagapack");
}

// ----- 导入冲突前移：预检/提前查重只拒绝，不写保险库；事务内同一检查兜底 -----
fn tree(root: &Path) -> std::collections::BTreeSet<PathBuf> {
    let mut found = std::collections::BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(folder) = pending.pop() {
        let Ok(entries) = fs::read_dir(&folder) else { continue };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.is_dir() { pending.push(path.clone()); }
            found.insert(path.strip_prefix(root).unwrap().to_path_buf());
        }
    }
    found
}
fn staged(parent: &Path) -> bool {
    fs::read_dir(parent).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().starts_with(".identity-transfer-"))
}
fn stages(reporter: &Reporter) -> Vec<String> { reporter.events().into_iter().map(|e| e.stage().to_owned()).collect() }
fn exported(store: &Store, record: &Record, folder: &Path, browser: Option<crate::browser::config::Config>) -> Vec<u8> {
    let path = folder.join(format!("{}.bagapack", crate::identities::new_id().unwrap()));
    store.export_pack(&path, PASSWORD, &record.id, browser, &Reporter::channel(None)).unwrap();
    fs::read(path).unwrap()
}
fn seeded(seed: u32) -> crate::browser::config::Config {
    let mut config = crate::browser::config::Config::default();
    config.seed = seed;
    config
}
/// 已有一个 Portable A 的目标机：返回（夹具、Store、A 的迁移包）。
fn imported_once(browser: Option<crate::browser::config::Config>) -> (Fixture, Store, Vec<u8>) {
    let (origin, source, row) = saved();
    let seed = browser.as_ref().map(|c| c.seed);
    let bytes = exported(&source, &row, &origin.root, browser);
    let target = Fixture::new();
    let store = Store::open(&target.vault, &target.home).unwrap();
    let slot = crate::identities::new_id().unwrap();
    let imported = read_pack(&bytes, PASSWORD, &target.root).unwrap();
    store.import_pack(imported, seed.map(|s| (slot.as_str(), s)), &Reporter::channel(None)).unwrap();
    (target, store, bytes)
}

#[test]
fn duplicate_account_fails_in_precheck_before_resealing_and_leaves_the_vault_untouched() {
    let (target, store, bytes) = imported_once(None);
    let before = tree(&target.vault);
    let imported = read_pack(&bytes, PASSWORD, &target.root).unwrap();
    let reporter = Reporter::channel(None);
    let error = store.import_pack(imported, None, &reporter).err().unwrap();
    assert_eq!(error, super::super::database::duplicate_account("Portable A"));
    let stages = stages(&reporter);
    assert!(stages.iter().any(|s| s == "transfer_precheck"));
    assert!(!stages.iter().any(|s| s == "transfer_archive" || s == "transfer_commit"), "{stages:?}");
    assert_eq!(tree(&target.vault), before);
    assert_eq!(store.records().len(), 1);
    // 文案只给档案名称，不泄露账号 UUID 或 HMAC。
    assert!(!error.contains("portable-A") && !error.contains(store.records()[0].account_key.as_deref().unwrap()));
}

#[test]
fn early_probe_rejects_duplicate_account_while_unpacking_without_creating_vault_files() {
    let (target, store, bytes) = imported_once(None);
    let before = tree(&target.vault);
    let probe = Probe::open(&target.vault).unwrap();
    let file = target.root.join("again.bagapack");
    private::write(&file, &bytes).unwrap();
    let error = unpack(selected_file(&file).unwrap(), PASSWORD, &target.root, Some(&probe), &Reporter::channel(None)).err().unwrap();
    assert_eq!(error, super::super::database::duplicate_account("Portable A"));
    assert!(!staged(&target.root));
    drop(probe);
    assert_eq!(tree(&target.vault), before);
    assert_eq!(store.records().len(), 1);
    // 不存在的保险库不会因探针被创建；探针缺席时解包照常完成，后续预检再决定。
    let fresh = Fixture::new();
    let missing = fresh.root.join("no-vault-yet");
    assert!(Probe::open(&missing).is_none());
    assert!(fs::symlink_metadata(&missing).is_err());
}

#[test]
fn exported_packs_put_the_account_member_first_so_the_probe_fires_before_bulk_data() {
    let (origin, source, row) = saved();
    let bytes = exported(&source, &row, &origin.root, None);
    let salt: [u8; 32] = bytes[12..44].try_into().unwrap();
    let key = password_key(PASSWORD, &salt).unwrap();
    let names: Vec<String> = {
        let decoded = compression::decode(codec::Open::new(io::Cursor::new(bytes[44..].to_vec()), key.as_ref()).unwrap()).unwrap();
        let mut archive = tar::Archive::new(decoded);
        let names = archive.entries().unwrap().take(2).map(|e| e.unwrap().path().unwrap().to_string_lossy().into_owned()).collect();
        names
    };
    let metadata = metadata(&source, &row);
    let account = account_source(&metadata.manifest).unwrap();
    assert_ne!(account, 0, "fixture must sort .claude.json after .claude/* to prove reordering");
    assert_eq!(names, ["transfer.json".to_string(), format!("data/{account}")]);
    // 账号成员之后紧跟一个非法成员：提前查重先于它触发，证明拒绝发生在解包中途。
    let mut value = serde_json::to_value(metadata).unwrap();
    value["manifest"]["sources"] = serde_json::json!([value["manifest"]["sources"][account].clone()]);
    value["manifest"]["fingerprints"] = serde_json::json!([value["manifest"]["fingerprints"][account].clone()]);
    let claude = fs::read(origin.home.join(".claude.json")).unwrap();
    let pack = custom_pack(&value, &[("data/0", tar::EntryType::Regular, &claude), ("../outside", tar::EntryType::Regular, b"x")], &[]);
    let target = Fixture::new();
    let store = Store::open(&target.vault, &target.home).unwrap();
    let imported = read_pack(&bytes, PASSWORD, &target.root).unwrap();
    store.import_pack(imported, None, &Reporter::channel(None)).unwrap();
    let probe = Probe::open(&target.vault).unwrap();
    let file = target.root.join("crafted.bagapack");
    private::write(&file, &pack).unwrap();
    let error = unpack(selected_file(&file).unwrap(), PASSWORD, &target.root, Some(&probe), &Reporter::channel(None)).err().unwrap();
    assert_eq!(error, super::super::database::duplicate_account("Portable A"));
    // 同一恶意包在没有冲突的保险库里仍按原规则拒绝：探针不放行任何东西。
    let empty = Fixture::new();
    let _ = Store::open(&empty.vault, &empty.home).unwrap();
    let probe = Probe::open(&empty.vault).unwrap();
    let file = empty.root.join("crafted.bagapack");
    private::write(&file, &pack).unwrap();
    assert_eq!(unpack(selected_file(&file).unwrap(), PASSWORD, &empty.root, Some(&probe), &Reporter::channel(None)).err().unwrap(), invalid());
    // 正常包配合探针在无冲突时完整导入。
    let imported = unpack({ let f = empty.root.join("ok.bagapack"); private::write(&f, &bytes).unwrap(); selected_file(&f).unwrap() }, PASSWORD, &empty.root, Some(&probe), &Reporter::channel(None)).unwrap();
    drop(probe);
    let store = Store::open(&empty.vault, &empty.home).unwrap();
    assert_eq!(store.import_pack(imported, None, &Reporter::channel(None)).unwrap().name, "Portable A");
}

#[test]
fn browser_seed_conflict_fails_before_unpacking_data_and_before_resealing() {
    let (target, store, _) = imported_once(Some(seeded(424242)));
    // 另一账号携带同一指纹种子。
    let origin = Fixture::new();
    origin.login("portable-B", "organization", "synthetic-token-b");
    let mut source = Store::open(&origin.vault, &origin.home).unwrap();
    source.save_current("Portable B", &Policy::default(), &Reporter::channel(None)).unwrap();
    let row = source.records()[0].clone();
    let bytes = exported(&source, &row, &origin.root, Some(seeded(424242)));
    let before = tree(&target.vault);
    let expected = super::super::database::duplicate_seed(Some("Portable A"));
    let probe = Probe::open(&target.vault).unwrap();
    let file = target.root.join("seed.bagapack");
    private::write(&file, &bytes).unwrap();
    let reporter = Reporter::channel(None);
    assert_eq!(unpack(selected_file(&file).unwrap(), PASSWORD, &target.root, Some(&probe), &reporter).err().unwrap(), expected);
    // 种子在 transfer.json 之后立即拒绝，尚未报告任何已认证数据成员。
    assert!(reporter.events().iter().all(|e| e.current() == 0));
    assert!(!staged(&target.root));
    drop(probe);
    let imported = read_pack(&bytes, PASSWORD, &target.root).unwrap();
    let reporter = Reporter::channel(None);
    let slot = crate::identities::new_id().unwrap();
    assert_eq!(store.import_pack(imported, Some((slot.as_str(), 424242)), &reporter).err().unwrap(), expected);
    assert!(!stages(&reporter).iter().any(|s| s == "transfer_archive"));
    assert_eq!(tree(&target.vault), before);
    assert_eq!(store.records().len(), 1);
}

#[test]
fn transaction_still_rejects_a_duplicate_inserted_after_precheck_and_leaves_no_files() {
    let (origin, source, row) = saved();
    let bytes = exported(&source, &row, &origin.root, None);
    let target = Fixture::new();
    let store = Store::open(&target.vault, &target.home).unwrap();
    let imported = read_pack(&bytes, PASSWORD, &target.root).unwrap();
    let checked = store.import_precheck(&imported, None, &Reporter::channel(None)).unwrap();
    let account = checked.account.clone().unwrap();
    // 模拟预检之后、提交之前另一路径写入了同账号档案。
    store.db.connection.execute(
        "INSERT INTO identities(id,name,archive,saved_at,bytes,policy,empty,account_key) VALUES(?1,'Raced',?2,1,0,?3,0,?4)",
        rusqlite::params![crate::identities::new_id().unwrap(), format!("{}.baga", crate::identities::new_id().unwrap()), serde_json::to_string(&Policy::default()).unwrap(), account],
    ).unwrap();
    let before = tree(&target.vault);
    let reporter = Reporter::channel(None);
    let error = store.import_checked(imported, checked, None, &reporter).err().unwrap();
    assert_eq!(error, super::super::database::duplicate_account("Raced"));
    assert!(stages(&reporter).iter().any(|s| s == "transfer_commit"));
    assert_eq!(tree(&target.vault), before);
    assert_eq!(store.records().len(), 1);
}

#[test]
fn export_rejects_bad_password_existing_target_and_missing_record_before_decrypting() {
    let (fixture, source, row) = saved();
    let path = fixture.root.join("early.bagapack");
    for (password, id) in [("short", row.id.as_str()), (PASSWORD, "00000000000000000000000000000000")] {
        let reporter = Reporter::channel(None);
        assert!(source.export_pack(&path, password, id, None, &reporter).is_err());
        assert!(reporter.events().is_empty(), "rejected before transfer_validate");
        assert!(fs::symlink_metadata(&path).is_err());
    }
    private::write(&path, b"keep-existing").unwrap();
    let reporter = Reporter::channel(None);
    assert_eq!(source.export_pack(&path, PASSWORD, &row.id, None, &reporter).err().unwrap(), "导出目标已存在；请选择新文件名。");
    assert!(reporter.events().is_empty());
    assert_eq!(fs::read(&path).unwrap(), b"keep-existing");
}
