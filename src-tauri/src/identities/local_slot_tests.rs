// [INPUT]: 隔离工作区、无 OAuth 账号元数据的本地档案与浏览器资料。
// [OUTPUT]: 有无 CLI 登录的 A/B 往返槽位、Cookie、命名保存/更新、空状态日志、联动重开/采用旧资料、目录边界及不同账号隔离回归。
// [POS]: 本地档案连续性证明；可选本地 Chromium 会话探针，不读取真实用户资料。
// [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
use super::{optimization_tests::Fixture, *};
use std::{fs, path::Path};

fn unknown(f: &Fixture, token: &str) {
    f.write(".claude.json", b"{}");
    f.write(".claude/.credentials.json", serde_json::to_string(
        &serde_json::json!({"claudeAiOauth":{"accessToken":token}})
    ).unwrap().as_bytes());
}

fn round_trip(mut browser: impl FnMut(&Path, &str, bool)) {
    let f = Fixture::new();
    let mut s = store::Store::open(&f.vault, &f.home).unwrap();
    let p = Policy::default();
    let r = Reporter::channel(None);
    s.db.configure_browser(true, false, None).unwrap();
    let slot_a = s.db.browser_slot().unwrap();
    let profile_a = f.root.join("browser/contexts").join(&slot_a).join("profile");
    unknown(&f, "a-first");
    browser(&profile_a, "A", true);
    s.switch("empty", "same name", &p, &r).unwrap();
    let a = s.records()[0].id.clone();
    let slot_b = s.db.browser_slot().unwrap();
    let profile_b = f.root.join("browser/contexts").join(&slot_b).join("profile");
    assert_ne!(slot_a, slot_b);
    unknown(&f, "b-first");
    browser(&profile_b, "B", true);
    s.switch(&a, "same name", &p, &r).unwrap();
    let b = s.records().into_iter().find(|row| row.id != a).unwrap().id;
    assert_eq!(s.records().len(), 2);
    assert_eq!(f.token(), "a-first");
    assert_eq!(s.db.browser_slot().unwrap(), slot_a);
    browser(&profile_a, "A", false);
    let seed_a = s.db.browser_seed(&slot_a).unwrap();
    let seed_b = s.db.browser_seed(&slot_b).unwrap();
    for n in 0..2 {
        unknown(&f, &format!("a-refresh-{n}"));
        s.switch(&b, "outgoing A", &p, &r).unwrap();
        assert_eq!(s.records().len(), 2, "A→B must update A, not create A′");
        assert_eq!(s.db.browser_slot().unwrap(), slot_b);
        assert_eq!(f.token(), if n == 0 { "b-first".into() } else { format!("b-refresh-{}", n-1) });
        browser(&profile_b, "B", false);
        unknown(&f, &format!("b-refresh-{n}"));
        s.switch(&a, "outgoing B", &p, &r).unwrap();
        assert_eq!(s.records().len(), 2, "B→A must update B, not create B′");
        assert_eq!(s.db.browser_slot().unwrap(), slot_a);
        assert_eq!(f.token(), format!("a-refresh-{n}"));
        browser(&profile_a, "A", false);
        assert_eq!(s.db.browser_for(&a).unwrap().as_deref(), Some(slot_a.as_str()));
        assert_eq!(s.db.browser_for(&b).unwrap().as_deref(), Some(slot_b.as_str()));
        assert_eq!(s.db.browser_seed(&slot_a).unwrap(), seed_a);
        assert_eq!(s.db.browser_seed(&slot_b).unwrap(), seed_b);
    }
    assert_eq!(s.db.browser_link().unwrap().bound_ids.len(), 2);
    assert!(catalog::current_id(&f.vault, &f.home, &s.db).unwrap().is_none(), "local continuity must not assert a recognized cloud account");
    assert_eq!(catalog::current_ids(&f.vault, &f.home, &s.db).unwrap(), (None, Some(a.clone())), "UI selection must use the same archive as snapshot updates");
    s.save_current("renamed local A", &p, &r).unwrap();
    assert_eq!(s.records().len(), 2, "in-place backup must update the selected local archive");
    assert_eq!(s.db.browser_slot().unwrap(), slot_a);
    browser(&profile_a, "A", false);
}

#[test]
fn unknown_local_archives_round_trip_without_new_cards_or_browser_profiles() {
    round_trip(|profile, account, login| {
        let cookie = profile.join("Default/Cookies");
        if login {
            fs::create_dir_all(cookie.parent().unwrap()).unwrap();
            fs::write(&cookie, format!("fixture-session-{account}")).unwrap();
        }
        assert_eq!(fs::read_to_string(cookie).unwrap(), format!("fixture-session-{account}"));
    });
}

#[test]
#[ignore = "requires the isolated local Chromium cookie probe runner"]
fn unknown_local_archives_keep_real_chromium_login_sessions() {
    let probe = std::env::var("CCBAGA_COOKIE_PROBE").expect("use scripts/test-local-identity-sessions.mjs");
    round_trip(|profile, account, login| {
        let status = std::process::Command::new("node").arg(&probe).arg("probe")
            .arg(profile).arg(account).arg(if login { "login" } else { "verify" })
            .status().unwrap();
        assert!(status.success(), "persistent Chromium session probe failed");
    });
}

#[test]
fn reselecting_unknown_local_archive_does_not_rewind_refreshed_credentials() {
    let f = Fixture::new();
    let mut s = store::Store::open(&f.vault, &f.home).unwrap();
    let p = Policy::default();
    let r = Reporter::channel(None);
    unknown(&f, "old");
    s.switch("empty", "A", &p, &r).unwrap();
    let a = s.records()[0].id.clone();
    s.switch(&a, "empty", &p, &r).unwrap();
    unknown(&f, "refreshed");
    s.switch(&a, "A", &p, &r).unwrap();
    assert_eq!(f.token(), "refreshed");
    assert_eq!(s.records().len(), 1);
}

#[test]
fn local_selection_does_not_overwrite_an_observed_other_account_or_merge_existing_duplicates() {
    let f = Fixture::new();
    let mut s = store::Store::open(&f.vault, &f.home).unwrap();
    let p = Policy::default();
    let r = Reporter::channel(None);
    unknown(&f, "a");
    s.switch("empty", "same name", &p, &r).unwrap();
    let a = s.records()[0].id.clone();
    unknown(&f, "b");
    s.switch(&a, "same name", &p, &r).unwrap();
    let b = s.records().into_iter().find(|row| row.id != a).unwrap().id;
    // A pre-existing duplicate is a separate saved record, never a name-based merge target.
    let (duplicate, stage) = s.save_checkpoint("same name", &p, &r).unwrap();
    s.db.remember(&duplicate).unwrap();
    drop(stage);
    f.login("external", "org", "external-token");
    assert_eq!(catalog::current_ids(&f.vault, &f.home, &s.db).unwrap(), (None, None), "a new observed account must not inherit the previous local card");
    s.switch(&b, "external", &p, &r).unwrap();
    assert_eq!(s.records().len(), 4);
    assert!(s.records().iter().any(|row| row.id == duplicate.id));
    s.switch(&a, "same name", &p, &r).unwrap();
    assert_eq!(s.records().len(), 4);
    assert_eq!(f.token(), "a", "the external account must not overwrite A");
    assert!(s.records().iter().any(|row| row.id == duplicate.id));
    let external = s.records().into_iter().find(|row| row.name == "external").unwrap();
    s.switch(&external.id, "A", &p, &r).unwrap();
    assert_eq!(catalog::current_ids(&f.vault, &f.home, &s.db).unwrap(), (Some(external.id.clone()), Some(external.id.clone())));
    unknown(&f, "unidentified-other");
    assert_eq!(catalog::current_ids(&f.vault, &f.home, &s.db).unwrap(), (None, None), "lost metadata on a known account must not advertise an in-place update");
    s.switch(&a, "unidentified", &p, &r).unwrap();
    assert_eq!(s.records().len(), 5, "missing metadata must not overwrite a known account");
    s.switch(&external.id, "A", &p, &r).unwrap();
    assert_eq!(f.token(), "external-token");
}

fn browser_only_profile(f: &Fixture, s: &store::Store, value: &str) -> std::path::PathBuf {
    let slot = s.db.browser_slot().unwrap();
    let profile = f.root.join("browser/contexts").join(slot).join("profile");
    private::directory(&profile.join("Default")).unwrap();
    private::write(&profile.join("Default/Cookies"), value.as_bytes()).unwrap();
    profile
}

#[test]
fn browser_only_named_backup_keeps_slot_and_can_be_updated_without_cli_login() {
    let f = Fixture::new();
    let mut s = store::Store::open(&f.vault, &f.home).unwrap();
    s.db.configure_browser(true, false, None).unwrap();
    let profile = browser_only_profile(&f, &s, "browser-only-login");
    let slot = s.db.browser_slot().unwrap();
    let active = s.db.get("active").unwrap();
    let r = Reporter::channel(None);
    assert!(!profile::read(&f.home).present);
    s.save_current("rio.wt", &Policy::default(), &r).unwrap();
    let record = s.records()[0].clone();
    assert_eq!(record.name, "rio.wt");
    assert!(!record.empty);
    assert_eq!(s.db.browser_for(&record.id).unwrap().as_deref(), Some(slot.as_str()));
    assert_eq!(s.db.browser_slot().unwrap(), slot);
    assert_eq!(s.db.get("active").unwrap(), active, "backup must not switch the CLI workspace");
    assert_eq!(catalog::current_ids(&f.vault, &f.home, &s.db).unwrap(), (None, Some(record.id.clone())));
    assert_eq!(s.db.browser_identity(&f.vault, &f.home).unwrap().status, "linked");
    s.save_current("rio.wt updated", &Policy::default(), &r).unwrap();
    assert_eq!(s.records().len(), 1, "repeat backup updates the same browser identity");
    assert_eq!(s.records()[0].id, record.id);
    assert_eq!(s.records()[0].name, "rio.wt updated");
    assert_eq!(fs::read_to_string(profile.join("Default/Cookies")).unwrap(), "browser-only-login");
}

fn browser_only_round_trip(mut browser: impl FnMut(&Path, &str, bool)) {
    let f = Fixture::new();
    let mut s = store::Store::open(&f.vault, &f.home).unwrap();
    s.db.configure_browser(true, false, None).unwrap();
    let r = Reporter::channel(None);
    let p = Policy::default();
    f.login("other", "org", "other-cli-token");
    let other_profile = f.root.join("browser/contexts").join(s.db.browser_slot().unwrap()).join("profile");
    browser(&other_profile, "B", true);
    s.switch("empty", "other", &p, &r).unwrap();
    let other = s.records()[0].id.clone();
    let slot = s.db.browser_slot().unwrap();
    let profile = f.root.join("browser/contexts").join(&slot).join("profile");
    browser(&profile, "A", true);
    assert!(!profile::read(&f.home).present);
    s.switch(&other, "rio.wt", &p, &r).unwrap();
    let saved = s.records().into_iter().find(|row| row.name == "rio.wt").expect("named browser state must not be discarded as empty");
    assert_eq!(s.records().len(), 2);
    assert_eq!(s.db.browser_for(&saved.id).unwrap().as_deref(), Some(slot.as_str()));
    assert_eq!(f.token(), "other-cli-token");
    browser(&other_profile, "B", false);
    s.switch(&saved.id, "other", &p, &r).unwrap();
    assert_eq!(s.db.browser_slot().unwrap(), slot);
    assert!(!profile::read(&f.home).present, "browser-only identity must not inherit the other CLI login");
    let credentials: serde_json::Value = serde_json::from_slice(&fs::read(f.home.join(".claude/.credentials.json")).unwrap()).unwrap();
    assert!(credentials.pointer("/claudeAiOauth/accessToken").is_none());
    assert_eq!(catalog::current_ids(&f.vault, &f.home, &s.db).unwrap(), (None, Some(saved.id.clone())));
    browser(&profile, "A", false);
    s.save_current("rio.wt", &p, &r).unwrap();
    assert_eq!(s.records().len(), 2);
    s.switch(&other, "ignored existing name", &p, &r).unwrap();
    assert_eq!(s.records().len(), 2);
    assert!(s.records().iter().any(|row| row.id == saved.id && row.name == "rio.wt"));
    browser(&profile, "A", false);
    browser(&other_profile, "B", false);
}

#[test]
fn browser_only_switch_retains_named_identity_and_round_trips_browser_and_cli_separately() {
    browser_only_round_trip(|profile, account, login| {
        let cookie = profile.join("Default/Cookies");
        if login {
            private::directory(cookie.parent().unwrap()).unwrap();
            private::write(&cookie, format!("browser-session-{account}").as_bytes()).unwrap();
        }
        assert_eq!(fs::read_to_string(cookie).unwrap(), format!("browser-session-{account}"));
    });
}

#[test]
#[ignore = "requires the isolated local Chromium cookie probe runner"]
fn browser_only_profiles_keep_real_chromium_login_sessions() {
    let probe = std::env::var("CCBAGA_COOKIE_PROBE").expect("use scripts/test-local-identity-sessions.mjs");
    browser_only_round_trip(|profile, account, login| {
        let status = std::process::Command::new("node").arg(&probe).arg("probe")
            .arg(profile).arg(account).arg(if login { "login" } else { "verify" })
            .status().unwrap();
        assert!(status.success(), "browser-only persistent Chromium session probe failed");
    });
}

#[test]
fn empty_unlinked_state_is_not_reported_as_saved() {
    let f = Fixture::new();
    let mut s = store::Store::open(&f.vault, &f.home).unwrap();
    s.db.configure_browser(false, false, None).unwrap();
    let r = Reporter::channel(None);
    assert!(s.save_current("empty", &Policy::default(), &r).is_err());
    s.switch("empty", "empty", &Policy::default(), &r).unwrap();
    assert!(s.records().is_empty());
    assert!(!r.events().iter().any(|event| event.message() == "身份切换完成，原身份已压缩暂存。"), "no saved identity must not be reported as a saved identity");
}

#[test]
fn browser_profile_presence_requires_linked_existing_safe_directory() {
    let f = Fixture::new();
    let mut s = store::Store::open(&f.vault, &f.home).unwrap();
    s.db.configure_browser(true, false, None).unwrap();
    assert!(!catalog::browser_profile_present(&f.vault, &s.db).unwrap());
    let profile = browser_only_profile(&f, &s, "safe-profile");
    assert!(catalog::browser_profile_present(&f.vault, &s.db).unwrap());
    s.db.configure_browser(false, false, None).unwrap();
    assert!(!catalog::browser_profile_present(&f.vault, &s.db).unwrap());
    #[cfg(unix)] {
        s.db.configure_browser(true, false, None).unwrap();
        let linked = f.root.join("browser/contexts").join(s.db.browser_slot().unwrap()).join("profile");
        private::directory(linked.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(profile, linked).unwrap();
        assert!(catalog::browser_profile_present(&f.vault, &s.db).is_err());
        assert!(s.save_current("unsafe", &Policy::default(), &Reporter::channel(None)).is_err());
        assert!(s.records().is_empty());
    }
}

fn relinked_profiles_round_trip(mut browser: impl FnMut(&Path, &str, bool)) {
    // Both existing choices remain usable: a fresh profile or adopting legacy data.
    for adopt in [false, true] {
        let f = Fixture::new();
        let mut s = store::Store::open(&f.vault, &f.home).unwrap();
        let p = Policy::default();
        let r = Reporter::channel(None);
        s.db.configure_browser(true, false, None).unwrap();
        let slot_a = s.db.browser_slot().unwrap();
        let profile_a = f.root.join("browser/contexts").join(&slot_a).join("profile");
        browser(&profile_a, "A", true);
        s.switch("empty", "first", &p, &r).unwrap();
        let first = s.records()[0].clone();
        let first_bytes = fs::read(f.vault.join(&first.archive)).unwrap();
        s.switch(&first.id, "empty", &p, &r).unwrap();
        let current = catalog::current_id(&f.vault, &f.home, &s.db).unwrap();
        assert!(current.is_none());
        s.db.configure_browser(false, false, current.as_deref()).unwrap();
        s.db.configure_browser(true, adopt, current.as_deref()).unwrap();
        let slot_b = s.db.browser_slot().unwrap();
        let profile_b = if slot_b == "legacy" { f.root.join("browser/profile") }
            else { f.root.join("browser/contexts").join(&slot_b).join("profile") };
        browser(&profile_b, "B", true);
        assert_eq!(catalog::current_ids(&f.vault, &f.home, &s.db).unwrap(), (None, None), "new browser context must not advertise updating the old local archive");
        s.save_current("second", &p, &r).unwrap();
        let second = s.records().into_iter().find(|row| row.name == "second").unwrap();
        assert_ne!(first.id, second.id);
        let seed_a = s.db.browser_seed(&slot_a).unwrap();
        let seed_b = s.db.browser_seed(&slot_b).unwrap();
        for _ in 0..2 {
            s.save_current("second updated", &p, &r).unwrap();
            assert_eq!(s.records().len(), 2, "repeated backup must reuse the new browser context's archive");
            assert_eq!(catalog::current_ids(&f.vault, &f.home, &s.db).unwrap(), (None, Some(second.id.clone())));
            let old = s.record(&first.id).unwrap();
            assert_eq!(old.name, "first");
            assert_eq!(old.archive, first.archive);
            assert_eq!(fs::read(f.vault.join(&old.archive)).unwrap(), first_bytes);
            assert_eq!(s.db.browser_for(&first.id).unwrap().as_deref(), Some(slot_a.as_str()));
            assert_eq!(s.db.browser_for(&second.id).unwrap().as_deref(), Some(slot_b.as_str()));
        }
        s.switch(&first.id, "second", &p, &r).unwrap();
        assert_eq!(s.db.browser_slot().unwrap(), slot_a);
        browser(&profile_a, "A", false);
        s.switch(&second.id, "first", &p, &r).unwrap();
        assert_eq!(s.db.browser_slot().unwrap(), slot_b);
        browser(&profile_b, "B", false);
        assert_eq!(s.db.browser_seed(&slot_a).unwrap(), seed_a);
        assert_eq!(s.db.browser_seed(&slot_b).unwrap(), seed_b);
        assert_eq!(s.records().len(), 2);
    }
}

#[test]
fn relinking_and_repeated_backup_preserve_both_browser_archives_and_adoption_choices() {
    relinked_profiles_round_trip(|profile, account, login| {
        let cookie = profile.join("Default/Cookies");
        if login {
            private::directory(cookie.parent().unwrap()).unwrap();
            private::write(&cookie, format!("browser-session-{account}").as_bytes()).unwrap();
        }
        assert_eq!(fs::read_to_string(cookie).unwrap(), format!("browser-session-{account}"));
    });
}

#[test]
#[ignore = "requires the isolated local Chromium cookie probe runner"]
fn relinked_profiles_keep_real_chromium_login_sessions() {
    let probe = std::env::var("CCBAGA_COOKIE_PROBE").expect("use scripts/test-local-identity-sessions.mjs");
    relinked_profiles_round_trip(|profile, account, login| {
        let status = std::process::Command::new("node").arg(&probe).arg("probe")
            .arg(profile).arg(account).arg(if login { "login" } else { "verify" })
            .status().unwrap();
        assert!(status.success(), "relinked persistent Chromium session probe failed");
    });
}
