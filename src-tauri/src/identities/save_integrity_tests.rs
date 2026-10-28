// [INPUT]: 隔离 HOME、虚构 CLI/Desktop/浏览器资料与 SQLite 故障注入。
// [OUTPUT]: 保存事务原子性、历史不回灌/不丢失、联动重开后正确保存资料与账号隔离反例。
// [POS]: 从独立对抗性审查保留的回归，不读取真实身份、Cookie 或钥匙串。
// [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
use super::{optimization_tests::Fixture, *};
use std::{fs, path::PathBuf};

// Retained adversarial cases. Synthetic HOME and browser directories only.
fn audit_browser(f: &Fixture, s: &store::Store, marker: &[u8]) -> PathBuf {
    let slot = s.db.browser_slot().unwrap();
    let path = f.root.join("browser/contexts").join(slot).join("profile");
    private::directory(&path.join("Default")).unwrap();
    private::write(&path.join("Default/Cookies"), marker).unwrap();
    path
}
#[test]
fn audit_relink_then_save_must_recover_the_profile_that_was_actually_saved() {
    let f=Fixture::new(); let mut s=store::Store::open(&f.vault,&f.home).unwrap();
    let p=Policy::default(); let r=Reporter::channel(None);
    s.db.configure_browser(true,false,None).unwrap();
    let first=audit_browser(&f,&s,b"first-web-session");
    s.switch("empty","first",&p,&r).unwrap();
    let a=s.records()[0].id.clone(); s.switch(&a,"empty",&p,&r).unwrap();
    // Mirrors identities_browser: current_id is cloud-account based and is None here.
    let current=catalog::current_id(&f.vault,&f.home,&s.db).unwrap(); assert!(current.is_none());
    s.db.configure_browser(false,false,current.as_deref()).unwrap();
    s.db.configure_browser(true,false,current.as_deref()).unwrap();
    let actual_slot=s.db.browser_slot().unwrap(); let second=audit_browser(&f,&s,b"second-web-session");
    s.save_current("first",&p,&r).unwrap();
    let saved=s.records().into_iter().find(|row|row.name=="first").unwrap();
    assert_eq!(s.db.browser_for(&saved.id).unwrap().as_deref(),Some(actual_slot.as_str()),"successful save must reference the actual current browser profile");
    s.switch("empty","second",&p,&r).unwrap(); s.switch(&saved.id,"empty",&p,&r).unwrap();
    assert_eq!(s.db.browser_slot().unwrap(),actual_slot);
    assert_eq!(fs::read(first.join("Default/Cookies")).unwrap(),b"first-web-session");
    assert_eq!(fs::read(second.join("Default/Cookies")).unwrap(),b"second-web-session");
}
#[test]
fn audit_history_only_reselection_must_not_rewind_unsaved_history() {
    let f=Fixture::new(); let mut s=store::Store::open(&f.vault,&f.home).unwrap();
    s.db.configure_browser(false,false,None).unwrap(); let p=Policy::default();let r=Reporter::channel(None);
    f.write(".claude/history.jsonl",b"old history");
    s.switch("empty","history-only",&p,&r).unwrap();let a=s.records()[0].id.clone();
    s.switch(&a,"empty",&p,&r).unwrap(); f.write(".claude/history.jsonl",b"new history that must survive");
    assert!(!profile::read(&f.home).present);
    s.switch(&a,"history-only",&p,&r).unwrap();
    assert_eq!(fs::read(f.home.join(".claude/history.jsonl")).unwrap(),b"new history that must survive","reselection must not overwrite newer local history with the old archive");
}
#[test]
fn audit_browser_binding_commit_failure_is_atomic_and_retryable() {
    let f=Fixture::new();let mut s=store::Store::open(&f.vault,&f.home).unwrap();
    s.db.configure_browser(true,false,None).unwrap(); let path=audit_browser(&f,&s,b"web-session");
    let slot=s.db.browser_slot().unwrap(); let active=s.db.get("active").unwrap();
    let c=rusqlite::Connection::open(f.vault.join("identities.sqlite")).unwrap();
    c.execute_batch("CREATE TRIGGER reject_binding BEFORE INSERT ON browser_bindings BEGIN SELECT RAISE(ABORT,'audit'); END;").unwrap();
    let r=Reporter::channel(None);assert!(s.save_current("rejected",&Policy::default(),&r).is_err());
    assert!(s.records().is_empty());assert!(s.db.summaries().unwrap().is_empty());
    assert_eq!(s.db.get("active").unwrap(),active);assert_eq!(s.db.browser_slot().unwrap(),slot);
    assert_eq!(fs::read(path.join("Default/Cookies")).unwrap(),b"web-session");
    assert!(!r.events().iter().any(|e|e.message()=="身份备份已保存。"));
    c.execute_batch("DROP TRIGGER reject_binding").unwrap();
    s.save_current("retry",&Policy::default(),&r).unwrap();assert_eq!(s.records().len(),1);
}
#[test]
fn audit_backup_summary_failure_preserves_previous_generation_and_live_state() {
    let f=Fixture::new();let mut s=store::Store::open(&f.vault,&f.home).unwrap();
    f.login("A","org","old");let p=Policy::default();let r=Reporter::channel(None);
    s.save_current("original",&p,&r).unwrap();let old=s.records()[0].clone();
    let old_bytes=fs::read(f.vault.join(&old.archive)).unwrap();f.login("A","org","refreshed");
    let c=rusqlite::Connection::open(f.vault.join("identities.sqlite")).unwrap();
    c.execute_batch("CREATE TRIGGER reject_summary BEFORE INSERT ON identity_summaries BEGIN SELECT RAISE(ABORT,'audit'); END;").unwrap();
    assert!(s.save_current("renamed",&p,&r).is_err());assert_eq!(f.token(),"refreshed");
    assert_eq!(s.records()[0].name,"original");assert_eq!(s.records()[0].archive,old.archive);
    assert_eq!(fs::read(f.vault.join(&old.archive)).unwrap(),old_bytes);assert!(s.db.get("recovery").unwrap().is_none());
    c.execute_batch("DROP TRIGGER reject_summary").unwrap();s.save_current("renamed",&p,&r).unwrap();
    assert_eq!(s.records().len(),1);assert_eq!(s.records()[0].name,"renamed");
}
#[test]
fn audit_browser_only_switch_failure_retains_both_profile_and_cli_archive() {
    let f=Fixture::new();let mut s=store::Store::open(&f.vault,&f.home).unwrap();
    let p=Policy::default();let r=Reporter::channel(None);s.db.configure_browser(true,false,None).unwrap();
    f.login("cli","org","saved-cli");s.switch("empty","cli",&p,&r).unwrap();let cli=s.records()[0].clone();
    let path=audit_browser(&f,&s,b"web-only");let slot=s.db.browser_slot().unwrap();
    s.fail_after_clear=true;assert!(s.switch(&cli.id,"web",&p,&r).is_err());
    assert_eq!(s.db.browser_slot().unwrap(),slot);assert!(!profile::read(&f.home).present);
    assert_eq!(fs::read(path.join("Default/Cookies")).unwrap(),b"web-only");
    assert!(s.records().iter().any(|r|r.name=="web"&&!r.empty));assert!(s.db.get("recovery").unwrap().is_none());
    s.fail_after_clear=false;s.switch(&cli.id,"web",&p,&r).unwrap();assert_eq!(f.token(),"saved-cli");
}
#[test]
fn audit_cli_backup_cannot_steal_a_different_accounts_browser_profile() {
    let f=Fixture::new();let mut s=store::Store::open(&f.vault,&f.home).unwrap();
    let p=Policy::default();let r=Reporter::channel(None);s.db.configure_browser(true,false,None).unwrap();
    f.login("A","org","a");let path=audit_browser(&f,&s,b"A-web");s.save_current("A",&p,&r).unwrap();
    let a=s.records()[0].clone();let slot=s.db.browser_slot().unwrap();f.login("B","org","b");
    s.save_current("B",&p,&r).unwrap();let b=s.records().into_iter().find(|row|row.name=="B").unwrap();
    assert_eq!(s.db.browser_for(&a.id).unwrap().as_deref(),Some(slot.as_str()));
    assert!(s.db.browser_for(&b.id).unwrap().is_none());assert_eq!(s.db.browser_slot().unwrap(),slot);
    assert_eq!(fs::read(path.join("Default/Cookies")).unwrap(),b"A-web");assert_eq!(f.token(),"b");
}
#[test]
fn audit_backup_archive_contains_selected_cli_desktop_history_and_no_shared_skills() {
    let f=Fixture::new();let mut s=store::Store::open(&f.vault,&f.home).unwrap();
    f.login("A","org","a");f.write(".claude/history.jsonl",b"private-history");f.write(".claude/skills/shared/SKILL.md",b"shared-skill");
    #[cfg(target_os="macos")]
    f.write("Library/Application Support/Claude/Local Storage/leveldb/fixture",b"desktop-login");
    let p=Policy::default();let r=Reporter::channel(None);s.save_current("all",&p,&r).unwrap();
    let stage=s.load(&s.records()[0]).unwrap();
    assert!(stage.manifest.sources.iter().any(|x|x.path.ends_with(".credentials.json")));
    assert!(stage.manifest.sources.iter().any(|x|x.path.ends_with("history.jsonl")));
    assert!(!stage.manifest.sources.iter().any(|x|x.path.components().any(|c|c.as_os_str()=="skills")));
    #[cfg(target_os="macos")]
    assert!(stage.manifest.sources.iter().any(|x|x.category=="desktop"));
    assert_eq!(f.token(),"a");assert_eq!(fs::read(f.home.join(".claude/history.jsonl")).unwrap(),b"private-history");
    assert_eq!(fs::read(f.home.join(".claude/skills/shared/SKILL.md")).unwrap(),b"shared-skill");
}

#[test]
fn audit_latest_history_remains_recoverable_after_reselect_and_leave() {
    let f=Fixture::new();let mut s=store::Store::open(&f.vault,&f.home).unwrap();
    s.db.configure_browser(false,false,None).unwrap();let p=Policy::default();let r=Reporter::channel(None);
    f.write(".claude/history.jsonl",b"old history");s.switch("empty","local",&p,&r).unwrap();
    let a=s.records()[0].id.clone();s.switch(&a,"empty",&p,&r).unwrap();
    f.write(".claude/history.jsonl",b"latest user history");
    s.switch(&a,"local",&p,&r).unwrap();s.switch("empty","local",&p,&r).unwrap();
    let mut found=false;
    for row in s.records(){let stage=s.load(&row).unwrap();
        for (i,source) in stage.manifest.sources.iter().enumerate(){
            if source.path.ends_with("history.jsonl") && fs::read(stage.path.join(format!("data/{i}"))).unwrap()==b"latest user history" {found=true;}
        }
    }
    assert!(found,"latest history must remain recoverable from at least one committed archive");
}

#[test]
fn empty_unknown_workspace_can_still_restore_the_selected_archive() {
    let f = Fixture::new();
    let mut s = store::Store::open(&f.vault, &f.home).unwrap();
    s.db.configure_browser(false, false, None).unwrap();
    let p = Policy::default();
    let r = Reporter::channel(None);
    f.write(".claude/history.jsonl", b"recoverable history");
    s.switch("empty", "local", &p, &r).unwrap();
    let a = s.records()[0].id.clone();
    s.switch(&a, "empty", &p, &r).unwrap();
    fs::remove_file(f.home.join(".claude/history.jsonl")).unwrap();
    s.switch(&a, "local", &p, &r).unwrap();
    assert_eq!(fs::read(f.home.join(".claude/history.jsonl")).unwrap(), b"recoverable history");
    assert_eq!(s.records().len(), 1);
}

#[test]
fn retained_history_is_not_rewound_when_reselecting_with_a_keep_policy() {
    let f = Fixture::new();
    let mut s = store::Store::open(&f.vault, &f.home).unwrap();
    s.db.configure_browser(false, false, None).unwrap();
    let r = Reporter::channel(None);
    f.write(".claude/history.jsonl", b"old selected history");
    s.switch("empty", "local", &Policy::default(), &r).unwrap();
    let a = s.records()[0].id.clone();
    s.switch(&a, "empty", &Policy::default(), &r).unwrap();
    f.write(".claude/history.jsonl", b"shared live history");
    let keep = Policy { history: true, ..Policy::default() };
    s.switch(&a, "local", &keep, &r).unwrap();
    s.switch("empty", "local", &keep, &r).unwrap();
    assert_eq!(fs::read(f.home.join(".claude/history.jsonl")).unwrap(), b"shared live history");
    assert_eq!(s.records().len(), 1);
}

#[cfg(target_os="macos")]
#[test]
fn manual_window_full_identity_to_browser_only_and_back_preserves_each_client() {
    let f=Fixture::new(); let mut s=store::Store::open(&f.vault,&f.home).unwrap();
    let p=Policy::default(); let r=Reporter::channel(None);
    let desktop="Library/Application Support/Claude/Local Storage/leveldb/fixture";
    f.login("B","org","B-token"); f.write(desktop,b"B-desktop");
    let b_profile=audit_browser(&f,&s,b"B-web");
    s.switch("empty","B",&p,&r).unwrap(); let b=s.records()[0].id.clone();
    let a_profile=audit_browser(&f,&s,b"A-web");
    s.save_current("A-browser-only",&p,&r).unwrap();
    let a=s.records().into_iter().find(|x|x.name=="A-browser-only").unwrap().id;
    s.switch(&b,"A-browser-only",&p,&r).unwrap();
    assert_eq!(f.token(),"B-token"); assert_eq!(fs::read(f.home.join(desktop)).unwrap(),b"B-desktop");
    s.switch(&a,"B",&p,&r).unwrap();
    assert!(!profile::read(&f.home).present);
    assert!(!f.home.join(desktop).exists());
    let credentials:serde_json::Value=serde_json::from_slice(&fs::read(f.home.join(".claude/.credentials.json")).unwrap()).unwrap();
    assert!(credentials.get("claudeAiOauth").is_none());
    s.switch(&b,"A-browser-only",&p,&r).unwrap();
    assert_eq!(f.token(),"B-token"); assert_eq!(fs::read(f.home.join(desktop)).unwrap(),b"B-desktop");
    assert_eq!(fs::read(a_profile.join("Default/Cookies")).unwrap(),b"A-web");
    assert_eq!(fs::read(b_profile.join("Default/Cookies")).unwrap(),b"B-web");
}
