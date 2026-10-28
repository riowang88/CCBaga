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
    unpack(selected_file(&file).unwrap(), password, parent, &Reporter::channel(None))
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
