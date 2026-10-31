// [INPUT]: 隔离账号目录、平台原生绝对项目路径、合成索引/消息和显式元数据回放样本。
// [OUTPUT]: 共享默认关闭、白名单、既有字段保留/IPC taskId 补齐、正常后继自动跟进、分叉选择及过期拒绝、冲突/脱离/链接/进程拒绝与切换保留回归。
// [POS]: 不读取真实资料，不运行或终止真实客户端。
// [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
use super::*;
use crate::identities::{optimization_tests::Fixture, Policy, store::Store};
use crate::progress::Reporter;
const A:&str="11111111-1111-1111-1111-111111111111";
const B:&str="22222222-2222-2222-2222-222222222222";
const ID:&str="local_33333333-3333-3333-3333-333333333333";
const CLI:&str="44444444-4444-4444-4444-444444444444";
fn setup() -> (Fixture,PathBuf,PathBuf,PathBuf) {
    let f=Fixture::new(); let root=f.home.join("Library/Application Support/Claude/claude-code-sessions");
    let a=root.join(A).join(A); let b=root.join(B).join(B);
    private::directory(&a).unwrap();private::directory(&b).unwrap();
    let project=f.home.join("project");assert!(project.is_absolute());
    let data=serde_json::json!({"sessionId":ID,"cliSessionId":CLI,"cwd":project,"originCwd":project,"createdAt":100,"lastActivityAt":200,"title":"Test","permissionMode":"bypassPermissions","sessionPermissionUpdates":[{"type":"addRules"}],"remoteMcpServersConfig":{"secret":"do-not-copy"},"alwaysAllowedReasons":["unsafe"],"envScopeId":"old-account","bridgeSessionId":"old-bridge","armedWorkAtQuit":true});
    private::write(&a.join(format!("{ID}.json")),&serde_json::to_vec(&data).unwrap()).unwrap();
    private::write(&a.join("scheduled-tasks.json"),b"[]").unwrap();
    private::write(&b.join("scheduled-tasks.json"),b"[1]").unwrap();
    f.write(&format!(".claude/projects/project/{CLI}.jsonl"),b"body not read");
    (f,root,a,b)
}
#[test] fn shares_only_resume_fields_and_never_overwrites() {
    let(f,root,a,b)=setup();let original=fs::read(a.join(format!("{ID}.json"))).unwrap();
    let r=sync_at(&f.home,&root,||Ok(())).unwrap();assert_eq!((r.accounts,r.added,r.skipped),(2,1,0));
    let dest=b.join(format!("{ID}.json"));let v=read(&dest).unwrap();
    assert_eq!(v["permissionMode"],"default");assert_eq!(v["cliSessionId"],CLI);
    for key in ["sessionPermissionUpdates","alwaysAllowedReasons","envScopeId","remoteMcpServersConfig","bridgeSessionId","armedWorkAtQuit"] {assert!(v.get(key).is_none());}
    assert_eq!(fs::read(a.join(format!("{ID}.json"))).unwrap(),original);
    assert_eq!(fs::read(b.join("scheduled-tasks.json")).unwrap(),b"[1]");
    let mut v=v;v["title"]="Target title".into();private::write(&dest,&serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(sync_at(&f.home,&root,||Ok(())).unwrap().added,0);assert_eq!(read(&dest).unwrap()["title"],"Target title");
}
#[test] fn missing_body_and_remote_are_skipped() {
    let(f,root,a,b)=setup();fs::remove_file(f.home.join(format!(".claude/projects/project/{CLI}.jsonl"))).unwrap();
    assert_eq!(sync_at(&f.home,&root,||Ok(())).unwrap().skipped,1);
    f.write(&format!(".claude/projects/project/{CLI}.jsonl"),b"body");
    let p=a.join(format!("{ID}.json"));let mut v=read(&p).unwrap();v["sshConfig"]=serde_json::json!({"host":"remote"});private::write(&p,&serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(sync_at(&f.home,&root,||Ok(())).unwrap().skipped,1);assert!(!b.join(format!("{ID}.json")).exists());
}
#[test] fn conflicts_and_running_clients_do_not_write() {
    let(f,root,a,b)=setup();assert!(sync_at(&f.home,&root,||Err("running".into())).is_err());assert!(!b.join(format!("{ID}.json")).exists());
    let other=serde_json::json!(f.home.join("other-project"));
    let mut v=read(&a.join(format!("{ID}.json"))).unwrap();v["cwd"]=other.clone();private::write(&b.join(format!("{ID}.json")),&serde_json::to_vec(&v).unwrap()).unwrap();
    let error=sync_at(&f.home,&root,||Ok(())).unwrap_err();assert!(error.contains("项目路径不同"));assert_eq!(read(&b.join(format!("{ID}.json"))).unwrap()["cwd"],other);
}
#[cfg(unix)]
#[test] fn linked_destination_is_rejected() {
    let(f,root,a,b)=setup();std::os::unix::fs::symlink(a.join(format!("{ID}.json")),b.join(format!("{ID}.json"))).unwrap();
    assert!(sync_at(&f.home,&root,||Ok(())).is_err());
}
#[cfg(target_os="macos")]
#[test] fn switch_keeps_body_when_sharing_and_off_stops_future_sync() {
    let(f,_root,_a,b)=setup();f.login("a","org","token-a");
    let mut store=Store::open(&f.vault,&f.home).unwrap();
    assert!(!Policy::default().desktop_sessions);
    let policy=Policy{desktop_sessions:true,history:false,..Policy::default()};
    assert!(policy.keeps("history"));
    store.switch("empty","A",&policy,&Reporter::channel(None)).unwrap();
    assert!(b.join(format!("{ID}.json")).exists());assert!(f.home.join(format!(".claude/projects/project/{CLI}.jsonl")).exists());
    let saved=store.db.records().unwrap().into_iter().find(|r|!r.empty).unwrap();
    store.switch(&saved.id,"Empty",&policy,&Reporter::channel(None)).unwrap();assert_eq!(f.token(),"token-a");
    let off=Policy{history:true,..Policy::default()};
    store.switch("empty","A",&off,&Reporter::channel(None)).unwrap();assert!(b.join(format!("{ID}.json")).exists());
    fs::remove_file(b.join(format!("{ID}.json"))).unwrap();
    store.switch(&saved.id,"Empty",&off,&Reporter::channel(None)).unwrap();assert!(!b.join(format!("{ID}.json")).exists());
}
#[test] fn partial_failure_reports_committed_entries_and_retry_completes() {
    let(f,root,_a,b)=setup();let c=root.join("55555555-5555-5555-5555-555555555555").join(B);private::directory(&c).unwrap();
    let error=sync_at(&f.home,&root,||{if b.join(format!("{ID}.json")).exists() {Err("running".into())}else{Ok(())}}).unwrap_err();
    assert!(error.contains("已新增 1 个入口"));assert!(b.join(format!("{ID}.json")).exists());assert!(!c.join(format!("{ID}.json")).exists());
    assert_eq!(sync_at(&f.home,&root,||Ok(())).unwrap().added,1);
}
#[test] fn malformed_index_is_rejected_and_legacy_policy_remains_off() {
    let(f,root,a,b)=setup();let p=a.join(format!("{ID}.json"));let mut v=read(&p).unwrap();v["createdAt"]="unsupported-format".into();private::write(&p,&serde_json::to_vec(&v).unwrap()).unwrap();
    assert!(sync_at(&f.home,&root,||Ok(())).is_err());assert!(!b.join(format!("{ID}.json")).exists());
    let policy:Policy=serde_json::from_value(serde_json::json!({"history":true,"skills":false})).unwrap();assert!(policy.history);assert!(!policy.skills);assert!(!policy.desktop_sessions);
}

#[test] fn not_started_session_is_skipped_without_blocking_sync() {
    let(f,root,a,b)=setup();let p=a.join(format!("{ID}.json"));let mut v=read(&p).unwrap();v.as_object_mut().unwrap().remove("cliSessionId");private::write(&p,&serde_json::to_vec(&v).unwrap()).unwrap();
    let report=sync_at(&f.home,&root,||Ok(())).unwrap();assert_eq!((report.added,report.skipped),(0,1));assert!(!b.join(format!("{ID}.json")).exists());
}

#[test] fn lineage_survives_new_sharing_and_repairs_old_copies_without_overwriting_settings() {
    let(f,root,a,b)=setup();let name=format!("{ID}.json");
    let mut original=read(&a.join(&name)).unwrap();
    original["spawnedFrom"]=serde_json::json!({"sessionId":"local_55555555-5555-5555-5555-555555555555","taskId":"task_fixture","title":"Child title","name":"child-agent","sideSession":true,"handOff":true,"secret":"do-not-copy"});
    original["forkedFromSessionId"]="local_55555555-5555-5555-5555-555555555555".into();
    original["forkedAtMessageUuid"]="fixture-message".into();
    private::write(&a.join(&name),&serde_json::to_vec(&original).unwrap()).unwrap();
    assert_eq!(sync_at(&f.home,&root,||Ok(())).unwrap().added,1);
    let mut target=read(&b.join(&name)).unwrap();
    assert_eq!(target["spawnedFrom"]["sessionId"],original["spawnedFrom"]["sessionId"]);
    assert_eq!(target["spawnedFrom"]["sideSession"],true);assert_eq!(target["spawnedFrom"]["handOff"],true);
    assert!(target["spawnedFrom"].get("secret").is_none());
    for key in LINEAGE {target.as_object_mut().unwrap().remove(key);}
    target["title"]="My title".into();target["permissionMode"]="plan".into();
    target["sessionPermissionUpdates"]=serde_json::json!([{"keep":"existing"}]);
    private::write(&b.join(&name),&serde_json::to_vec(&target).unwrap()).unwrap();
    let r=sync_at(&f.home,&root,||Ok(())).unwrap();assert_eq!((r.added,r.repaired),(0,1));
    let mut repaired=read(&b.join(&name)).unwrap();
    assert_eq!(repaired["forkedAtMessageUuid"],"fixture-message");
    for key in LINEAGE {repaired.as_object_mut().unwrap().remove(key);}
    assert_eq!(repaired,target);assert_eq!(read(&a.join(&name)).unwrap(),original);
    assert_eq!(sync_at(&f.home,&root,||Ok(())).unwrap().repaired,0);
}
#[test] fn lineage_source_can_follow_old_copy_and_explicit_detachment_is_preserved() {
    let(f,root,a,b)=setup();let name=format!("{ID}.json");
    sync_at(&f.home,&root,||Ok(())).unwrap();
    let mut source=read(&b.join(&name)).unwrap();source["spawnedFrom"]=serde_json::json!({"sessionId":"parent","taskId":"task_fixture","userFork":true});
    private::write(&b.join(&name),&serde_json::to_vec(&source).unwrap()).unwrap();
    assert_eq!(sync_at(&f.home,&root,||Ok(())).unwrap().repaired,1);
    assert_eq!(read(&a.join(&name)).unwrap()["spawnedFrom"],source["spawnedFrom"]);
    let mut detached=read(&a.join(&name)).unwrap();for key in LINEAGE {detached.as_object_mut().unwrap().remove(key);}
    detached["lineageDetached"]=true.into();private::write(&a.join(&name),&serde_json::to_vec(&detached).unwrap()).unwrap();
    assert_eq!(sync_at(&f.home,&root,||Ok(())).unwrap().repaired,0);
    assert_eq!(read(&a.join(&name)).unwrap(),detached);
}
#[test] fn lineage_repair_refuses_changed_target() {
    let(f,root,a,b)=setup();let name=format!("{ID}.json");sync_at(&f.home,&root,||Ok(())).unwrap();
    let mut source=read(&a.join(&name)).unwrap();source["forkedFromSessionId"]="parent".into();
    private::write(&a.join(&name),&serde_json::to_vec(&source).unwrap()).unwrap();
    let calls=std::cell::Cell::new(0);
    let r=sync_at(&f.home,&root,||{calls.set(calls.get()+1);if calls.get()==3 {
        let mut target=read(&b.join(&name)).unwrap();target["title"]="Changed concurrently".into();private::write(&b.join(&name),&serde_json::to_vec(&target).unwrap()).unwrap();
    }Ok(())});
    assert!(r.is_err());let v=read(&b.join(&name)).unwrap();assert_eq!(v["title"],"Changed concurrently");assert!(v.get("forkedFromSessionId").is_none());
}

#[test] fn contradictory_relations_do_not_guess_a_parent_for_another_account() {
    let(f,root,a,b)=setup();let name=format!("{ID}.json");sync_at(&f.home,&root,||Ok(())).unwrap();
    for (folder,parent) in [(&a,"parent-a"),(&b,"parent-b")] {
        let mut v=read(&folder.join(&name)).unwrap();v["forkedFromSessionId"]=parent.into();
        private::write(&folder.join(&name),&serde_json::to_vec(&v).unwrap()).unwrap();
    }
    let c=root.join("66666666-6666-6666-6666-666666666666").join(A);private::directory(&c).unwrap();
    let report=sync_at(&f.home,&root,||Ok(())).unwrap();assert_eq!((report.added,report.repaired),(1,0));
    assert!(!has_lineage(&read(&c.join(&name)).unwrap()));
    assert_eq!(read(&a.join(&name)).unwrap()["forkedFromSessionId"],"parent-a");
    assert_eq!(read(&b.join(&name)).unwrap()["forkedFromSessionId"],"parent-b");
}

#[test] fn repairs_previous_partial_spawn_metadata_without_overwriting_existing_fields() {
    let(f,root,a,b)=setup();let name=format!("{ID}.json");
    let mut original=read(&a.join(&name)).unwrap();
    original["spawnedFrom"]=serde_json::json!({"sessionId":"parent","taskId":"task_fixture","title":"Child","name":"worker","sideSession":true,"wakeParent":true,"pendingPop":true,"proposalPicks":{"secret":"never-copy"}});
    private::write(&a.join(&name),&serde_json::to_vec(&original).unwrap()).unwrap();
    let mut partial=original.clone();partial["spawnedFrom"]=serde_json::json!({"sessionId":"parent","sideSession":true});partial["title"]="Keep target title".into();
    partial["permissionMode"]="plan".into();
    private::write(&b.join(&name),&serde_json::to_vec(&partial).unwrap()).unwrap();
    let report=sync_at(&f.home,&root,||Ok(())).unwrap();assert_eq!((report.added,report.repaired),(0,1));
    let actual=read(&b.join(&name)).unwrap();assert_eq!(actual["spawnedFrom"],serde_json::json!({"sessionId":"parent","taskId":"task_fixture","title":"Child","name":"worker","sideSession":true}));
    assert_eq!(actual["title"],"Keep target title");assert_eq!(actual["permissionMode"],"plan");
    assert_eq!(read(&a.join(&name)).unwrap(),original);
    assert_eq!(sync_at(&f.home,&root,||Ok(())).unwrap().repaired,0);
    // This synthetic result can be checked against the installed Claude IPC validator.
    println!("CLAUDE_SPAWN_FIXTURE={}",actual["spawnedFrom"]);
}
#[test] fn partial_spawn_before_complete_source_is_repaired_but_missing_task_is_not_invented() {
    let(f,root,a,b)=setup();let name=format!("{ID}.json");let mut original=read(&a.join(&name)).unwrap();
    original["spawnedFrom"]=serde_json::json!({"sessionId":"parent","sideSession":true});
    private::write(&a.join(&name),&serde_json::to_vec(&original).unwrap()).unwrap();
    let r=sync_at(&f.home,&root,||Ok(())).unwrap();assert_eq!(r.added,0);assert!(!b.join(&name).exists());
    let mut complete=original.clone();complete["spawnedFrom"]["taskId"]="task_fixture".into();
    private::write(&b.join(&name),&serde_json::to_vec(&complete).unwrap()).unwrap();
    assert_eq!(sync_at(&f.home,&root,||Ok(())).unwrap().repaired,1);
    assert_eq!(read(&a.join(&name)).unwrap()["spawnedFrom"],complete["spawnedFrom"]);
}
#[test] fn conflicting_task_identity_and_detached_child_remain_unchanged() {
    let(f,root,a,b)=setup();let name=format!("{ID}.json");let mut v=read(&a.join(&name)).unwrap();
    v["spawnedFrom"]=serde_json::json!({"sessionId":"parent","taskId":"task_original","sideSession":true});
    private::write(&a.join(&name),&serde_json::to_vec(&v).unwrap()).unwrap();
    v["spawnedFrom"]["taskId"]="task_other".into();private::write(&b.join(&name),&serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(sync_at(&f.home,&root,||Ok(())).unwrap().repaired,0);assert_eq!(read(&b.join(&name)).unwrap(),v);
    v["spawnedFrom"].as_object_mut().unwrap().remove("taskId");v["lineageDetached"]=true.into();
    private::write(&b.join(&name),&serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(sync_at(&f.home,&root,||Ok(())).unwrap().repaired,0);assert_eq!(read(&b.join(&name)).unwrap(),v);
}

#[cfg(target_os="macos")]
#[test] fn switching_automatically_repairs_partial_spawn_without_manual_sync() {
    let(f,_root,a,b)=setup();let name=format!("{ID}.json");let mut original=read(&a.join(&name)).unwrap();
    original["spawnedFrom"]=serde_json::json!({"sessionId":"parent","taskId":"task_fixture","sideSession":true});
    private::write(&a.join(&name),&serde_json::to_vec(&original).unwrap()).unwrap();
    let mut partial=original.clone();partial["spawnedFrom"].as_object_mut().unwrap().remove("taskId");partial["permissionMode"]="plan".into();
    private::write(&b.join(&name),&serde_json::to_vec(&partial).unwrap()).unwrap();
    f.login("a","org","token-a");let mut store=Store::open(&f.vault,&f.home).unwrap();
    let policy=Policy{desktop_sessions:true,history:false,..Policy::default()};
    store.switch("empty","A",&policy,&Reporter::channel(None)).unwrap();
    let restored=read(&b.join(&name)).unwrap();assert_eq!(restored["spawnedFrom"]["taskId"],"task_fixture");assert_eq!(restored["permissionMode"],"plan");
    let saved=store.db.records().unwrap().into_iter().find(|r|!r.empty).unwrap();
    store.switch(&saved.id,"Empty",&policy,&Reporter::channel(None)).unwrap();
    assert_eq!(read(&b.join(&name)).unwrap(),restored);assert_eq!(f.token(),"token-a");
}

const NEXT_CLI:&str="77777777-7777-7777-7777-777777777777";
const THIRD_CLI:&str="88888888-8888-8888-8888-888888888888";
fn write_body(f:&Fixture,id:&str,messages:&[(u32,u32)]) {
    let rows=messages.iter().map(|(id,second)|serde_json::json!({"type":"user","uuid":format!("00000000-0000-0000-0000-{id:012x}"),"timestamp":format!("1970-01-01T00:00:{second:02}.000Z"),"message":{"content":"fixture"}}).to_string()).collect::<Vec<_>>().join("\n");
    f.write(&format!(".claude/projects/project/{id}.jsonl"),rows.as_bytes());
}
fn rewind(original:&Value,parent:&str,child:&str)->Value {
    let mut v=original.clone();v["cliSessionId"]=child.into();v["priorCliSessionIds"]=serde_json::json!([parent]);v["lastActivityAt"]=400.into();
    v["rewindEdges"]=serde_json::json!([{"id":"99999999-9999-9999-9999-999999999999","parent":parent,"child":child,"at":20000,"forkPoint":"00000000-0000-0000-0000-000000000001"}]);v
}
fn choice(error:&str,id:&str)->Selection {
    let v:Value=serde_json::from_str(error.strip_prefix(CHOICE_PREFIX).unwrap()).unwrap();
    assert!(v["options"].as_array().unwrap().iter().any(|o|o["cli_id"]==id));
    Selection{session:v["session"].as_str().unwrap().into(),revision:v["revision"].as_str().unwrap().into(),cli_id:id.into()}
}
#[test] fn native_rewind_advances_existing_accounts_and_preserves_old_body_and_index() {
    let(f,root,a,b)=setup();let name=format!("{ID}.json");let original=read(&a.join(&name)).unwrap();
    let mut newer=rewind(&original,CLI,NEXT_CLI);newer["title"]="New title".into();newer["transcriptCuts"]=serde_json::json!({CLI:"00000000-0000-0000-0000-000000000001"});
    private::write(&b.join(&name),&serde_json::to_vec(&newer).unwrap()).unwrap();
    write_body(&f,CLI,&[(1,10),(2,15)]);write_body(&f,NEXT_CLI,&[(1,10),(3,30)]);
    let body_before=fs::read(f.home.join(format!(".claude/projects/project/{CLI}.jsonl"))).unwrap();
    let c=root.join(A).join(B);private::directory(&c).unwrap();
    let report=sync_at(&f.home,&root,||Ok(())).unwrap();assert_eq!((report.added,report.repaired),(1,1));
    let updated=read(&a.join(&name)).unwrap();assert_eq!(updated["cliSessionId"],NEXT_CLI);assert_eq!(updated["rewindEdges"],newer["rewindEdges"]);assert_eq!(updated["transcriptCuts"],newer["transcriptCuts"]);
    for key in ["title","permissionMode","sessionPermissionUpdates","alwaysAllowedReasons","envScopeId","remoteMcpServersConfig"] {assert_eq!(updated[key],original[key]);}
    assert_eq!(read(&b.join(&name)).unwrap(),newer);assert_eq!(read(&c.join(&name)).unwrap()["permissionMode"],"default");
    assert_eq!(fs::read(f.home.join(format!(".claude/projects/project/{CLI}.jsonl"))).unwrap(),body_before);
    let backups=entries(&a.join(".baga-history")).unwrap();assert_eq!(backups.len(),1);assert_eq!(read(&backups[0]).unwrap(),original);
    assert_eq!(sync_at(&f.home,&root,||Ok(())).unwrap().repaired,0);
}
#[test] fn rewind_evidence_arriving_after_incomplete_copy_updates_every_account() {
    let(f,root,a,b)=setup();let name=format!("{ID}.json");let old=read(&a.join(&name)).unwrap();
    private::write(&b.join(&name),&serde_json::to_vec(&old).unwrap()).unwrap();
    let mut incomplete=old.clone();incomplete["cliSessionId"]=NEXT_CLI.into();private::write(&a.join(&name),&serde_json::to_vec(&incomplete).unwrap()).unwrap();
    let c=root.join(A).join(B);private::directory(&c).unwrap();let complete=rewind(&old,CLI,NEXT_CLI);private::write(&c.join(&name),&serde_json::to_vec(&complete).unwrap()).unwrap();
    write_body(&f,CLI,&[(1,10),(2,15)]);write_body(&f,NEXT_CLI,&[(1,10),(3,30)]);
    sync_at(&f.home,&root,||Ok(())).unwrap();
    for dir in [&a,&b,&c] {let actual=read(&dir.join(&name)).unwrap();assert_eq!(actual["cliSessionId"],NEXT_CLI);assert_eq!(actual["rewindEdges"],complete["rewindEdges"]);}
}
#[test] fn writing_on_both_sides_requires_choice_before_any_write_and_choice_keeps_both_bodies() {
    let(f,root,a,b)=setup();let name=format!("{ID}.json");let old=read(&a.join(&name)).unwrap();let newer=rewind(&old,CLI,NEXT_CLI);
    private::write(&b.join(&name),&serde_json::to_vec(&newer).unwrap()).unwrap();write_body(&f,CLI,&[(1,10),(2,25)]);write_body(&f,NEXT_CLI,&[(1,10),(3,30)]);
    let err=sync_at(&f.home,&root,||Ok(())).unwrap_err();let selected=choice(&err,CLI);
    assert_eq!(read(&a.join(&name)).unwrap(),old);assert_eq!(read(&b.join(&name)).unwrap(),newer);assert!(!a.join(".baga-history").exists());
    let before:Vec<_>=[CLI,NEXT_CLI].map(|id|fs::read(f.home.join(format!(".claude/projects/project/{id}.jsonl"))).unwrap()).into();
    sync_at_selected(&f.home,&root,&[selected],||Ok(())).unwrap();assert_eq!(read(&b.join(&name)).unwrap()["cliSessionId"],CLI);
    assert_eq!(read(&entries(&b.join(".baga-history")).unwrap()[0]).unwrap(),newer);
    for (i,id) in [CLI,NEXT_CLI].iter().enumerate(){assert_eq!(fs::read(f.home.join(format!(".claude/projects/project/{id}.jsonl"))).unwrap(),before[i]);}
    assert_eq!(sync_at(&f.home,&root,||Ok(())).unwrap().repaired,0);
}
#[test] fn stale_choice_and_unknown_choice_never_overwrite_new_messages() {
    let(f,root,a,b)=setup();let name=format!("{ID}.json");let old=read(&a.join(&name)).unwrap();let newer=rewind(&old,CLI,NEXT_CLI);
    private::write(&b.join(&name),&serde_json::to_vec(&newer).unwrap()).unwrap();write_body(&f,CLI,&[(1,10),(2,25)]);write_body(&f,NEXT_CLI,&[(1,10),(3,30)]);
    let selected=choice(&sync_at(&f.home,&root,||Ok(())).unwrap_err(),CLI);
    write_body(&f,NEXT_CLI,&[(1,10),(3,30),(4,35)]);
    let err=sync_at_selected(&f.home,&root,&[selected],||Ok(())).unwrap_err();let mut fresh=choice(&err,CLI);fresh.cli_id=THIRD_CLI.into();
    assert!(sync_at_selected(&f.home,&root,&[fresh],||Ok(())).is_err());assert_eq!(read(&a.join(&name)).unwrap(),old);assert_eq!(read(&b.join(&name)).unwrap(),newer);
}
#[test] fn unrelated_sibling_and_contradictory_versions_all_require_explicit_choice() {
    for mode in 0..3 {
        let(f,root,a,b)=setup();let name=format!("{ID}.json");let mut old=read(&a.join(&name)).unwrap();let mut other=old.clone();other["cliSessionId"]=NEXT_CLI.into();
        if mode==1 {old=rewind(&old,THIRD_CLI,CLI);other=rewind(&other,THIRD_CLI,NEXT_CLI);}
        if mode==2 {old=rewind(&old,NEXT_CLI,CLI);other=rewind(&other,CLI,NEXT_CLI);}
        private::write(&a.join(&name),&serde_json::to_vec(&old).unwrap()).unwrap();private::write(&b.join(&name),&serde_json::to_vec(&other).unwrap()).unwrap();
        write_body(&f,CLI,&[(1,25)]);write_body(&f,NEXT_CLI,&[(2,30)]);
        let err=sync_at(&f.home,&root,||Ok(())).unwrap_err();choice(&err,NEXT_CLI);assert_eq!(read(&a.join(&name)).unwrap(),old);assert_eq!(read(&b.join(&name)).unwrap(),other);
    }
}
#[test] fn strict_message_extension_without_rewind_follows_the_complete_transcript() {
    let(f,root,a,b)=setup();let name=format!("{ID}.json");let mut newer=read(&a.join(&name)).unwrap();newer["cliSessionId"]=NEXT_CLI.into();
    private::write(&b.join(&name),&serde_json::to_vec(&newer).unwrap()).unwrap();write_body(&f,CLI,&[(1,10)]);write_body(&f,NEXT_CLI,&[(1,10),(2,30)]);
    sync_at(&f.home,&root,||Ok(())).unwrap();assert_eq!(read(&a.join(&name)).unwrap()["cliSessionId"],NEXT_CLI);
}
#[cfg(target_os="macos")]
#[test] fn switching_follows_native_rewind_automatically_and_keeps_both_bodies() {
    let(f,_root,a,b)=setup();let name=format!("{ID}.json");let newer=rewind(&read(&a.join(&name)).unwrap(),CLI,NEXT_CLI);
    private::write(&b.join(&name),&serde_json::to_vec(&newer).unwrap()).unwrap();write_body(&f,CLI,&[(1,10),(2,15)]);write_body(&f,NEXT_CLI,&[(1,10),(3,30)]);f.login("a","org","token-a");
    let mut store=Store::open(&f.vault,&f.home).unwrap();let policy=Policy{desktop_sessions:true,history:false,..Policy::default()};
    store.switch("empty","A",&policy,&Reporter::channel(None)).unwrap();
    assert_eq!(read(&a.join(&name)).unwrap()["cliSessionId"],NEXT_CLI);assert_eq!(read(&b.join(&name)).unwrap()["cliSessionId"],NEXT_CLI);
    for id in [CLI,NEXT_CLI] {assert!(f.home.join(format!(".claude/projects/project/{id}.jsonl")).exists());}
}
#[cfg(target_os="macos")]
#[test] fn switch_conflict_leaves_credentials_until_explicit_selection() {
    let(f,_root,a,b)=setup();let name=format!("{ID}.json");let newer=rewind(&read(&a.join(&name)).unwrap(),CLI,NEXT_CLI);
    private::write(&b.join(&name),&serde_json::to_vec(&newer).unwrap()).unwrap();write_body(&f,CLI,&[(1,10),(2,25)]);write_body(&f,NEXT_CLI,&[(1,10),(3,30)]);f.login("a","org","token-a");
    let mut store=Store::open(&f.vault,&f.home).unwrap();let policy=Policy{desktop_sessions:true,history:false,..Policy::default()};let reporter=Reporter::channel(None);
    let err=store.switch("empty","A",&policy,&reporter).unwrap_err();let selected=choice(&err,NEXT_CLI);assert_eq!(f.token(),"token-a");assert!(store.db.get("recovery").unwrap().is_none());
    store.switch_with_sessions("empty","A",&policy,&reporter,&[selected]).unwrap();assert_eq!(read(&a.join(&name)).unwrap()["cliSessionId"],NEXT_CLI);
}

#[test]
#[ignore = "Requires an explicitly supplied metadata-only fixture; writes only inside Fixture"]
fn replay_desktop_metadata_fixture() {
    let input=std::env::var("CCBAGA_DESKTOP_REPLAY_FIXTURE").expect("metadata-only fixture path required");
    let fixture:Value=serde_json::from_slice(&fs::read(input).unwrap()).unwrap();let f=Fixture::new();
    let root=f.home.join("Library/Application Support/Claude/claude-code-sessions");
    let mut originals=Vec::new();
    for record in fixture["records"].as_array().unwrap() {
        let bucket=record["bucket"].as_u64().unwrap();let dir=root.join(format!("00000000-0000-0000-0000-{bucket:012x}")).join(A);
        private::directory(&dir).unwrap();let path=dir.join(record["name"].as_str().unwrap());
        private::write(&path,&serde_json::to_vec(&record["value"]).unwrap()).unwrap();originals.push((path,record["value"].clone()));
    }
    let mut bodies=Vec::new();
    for (id,rows) in fixture["bodies"].as_object().unwrap() {
        let bytes=rows.as_array().unwrap().iter().map(|r|r.to_string()).collect::<Vec<_>>().join("\n").into_bytes();
        let relative=format!(".claude/projects/project/{id}.jsonl");f.write(&relative,&bytes);bodies.push((f.home.join(relative),bytes));
    }
    let report=sync_at(&f.home,&root,||Ok(())).unwrap();let mut advanced=0;
    for (path,old) in originals {
        let updated=read(&path).unwrap();if updated["cliSessionId"]!=old["cliSessionId"] {
            advanced+=1;let old_bytes=serde_json::to_vec(&old).unwrap();let backup=path.parent().unwrap().join(".baga-history").join(format!("{}-{}.json",old["sessionId"].as_str().unwrap(),digest(&old_bytes)));assert_eq!(read(&backup).unwrap(),old);
        }
        for field in ["title","permissionMode","lineageDetached"] { assert_eq!(updated[field],old[field]); }
    }
    for (path,bytes) in bodies {assert_eq!(fs::read(path).unwrap(),bytes);}
    let second=sync_at(&f.home,&root,||Ok(())).unwrap();assert_eq!((second.added,second.repaired),(0,0));assert!(advanced>0);
    println!("REAL_METADATA_REPLAY accounts={} added={} updated={} advanced={} bodies_unchanged=true idempotent=true",report.accounts,report.added,report.repaired,advanced);
}

#[test] fn equivalent_body_with_one_way_native_successor_needs_no_choice() {
    let(f,root,a,b)=setup();let name=format!("{ID}.json");let old=read(&a.join(&name)).unwrap();
    let mut newer=old.clone();newer["cliSessionId"]=NEXT_CLI.into();newer["priorCliSessionIds"]=serde_json::json!([CLI]);
    private::write(&b.join(&name),&serde_json::to_vec(&newer).unwrap()).unwrap();
    write_body(&f,CLI,&[(1,10)]);write_body(&f,NEXT_CLI,&[(1,10)]);
    sync_at(&f.home,&root,||Ok(())).unwrap();assert_eq!(read(&a.join(&name)).unwrap()["cliSessionId"],NEXT_CLI);
}
