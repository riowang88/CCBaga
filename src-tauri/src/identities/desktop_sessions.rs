// [INPUT]: macOS 默认 Claude 桌面索引、正文消息标识/时间及原生回退记录。
// [OUTPUT]: 自动跟进明确后继；分叉返回有版本校验的选择，保存旧索引与正文，不复制授权/任务。
// [POS]: 显式可选共享；调用方持锁并检查客户端停止，关闭后不撤销已复制入口。
// [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
use super::private;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use serde_json::{Map, Value};
use std::{collections::BTreeMap, fs, io::{Read, Write}, path::{Path, PathBuf}};
const LIMIT: usize = 10_000;
#[derive(Default, Serialize, Debug)]
pub struct Report { pub accounts: usize, pub added: usize, pub repaired: usize, pub skipped: usize }
fn invalid() -> String { "桌面会话索引无法安全读取或存在冲突；未完成共享。".into() }
fn uuid(s: &str) -> bool {
    s.len() == 36 && s.bytes().enumerate().all(|(i,b)| if [8,13,18,23].contains(&i) { b == b'-' } else { b.is_ascii_hexdigit() })
}
fn entries(path: &Path) -> Result<Vec<PathBuf>, String> {
    private::parents(path)?;
    if !path.try_exists().map_err(|_| invalid())? { return Ok(vec![]); }
    private::safe(path)?;
    let mut result = Vec::new();
    for entry in fs::read_dir(path).map_err(|_| invalid())? {
        if result.len() >= LIMIT { return Err(invalid()); }
        result.push(entry.map_err(|_| invalid())?.path());
    }
    result.sort(); Ok(result)
}
fn regular(path: &Path) -> Result<(), String> {
    private::parents(path.parent().ok_or_else(invalid)?)?;
    private::safe(path)?;
    let meta = fs::symlink_metadata(path).map_err(|_| invalid())?;
    if !meta.is_file() { return Err(invalid()); }
    #[cfg(unix)] {
        use std::os::unix::fs::MetadataExt;
        if meta.nlink() != 1 { return Err(invalid()); }
    }
    Ok(())
}
fn read(path: &Path) -> Result<Value, String> {
    regular(path)?;
    let mut data = Vec::new();
    fs::File::open(path).map_err(|_| invalid())?.take(10*1024*1024+1).read_to_end(&mut data).map_err(|_| invalid())?;
    if data.len() > 10*1024*1024 { return Err(invalid()); }
    serde_json::from_slice(&data).map_err(|_| invalid())
}
fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> { v[key].as_str().ok_or_else(invalid) }
fn core(v: &Value, filename: &str) -> Result<(String,String), String> {
    let id = text(v,"sessionId")?;
    if id.strip_prefix("local_").is_none_or(|s| !uuid(s)) || filename != format!("{id}.json") { return Err(invalid()); }
    let cli = text(v,"cliSessionId")?;
    let cwd = text(v,"cwd")?;
    if !uuid(cli) || !Path::new(cwd).is_absolute() { return Err(invalid()); }
    Ok((cli.into(),cwd.into()))
}
fn prior_cli_ids(v: &Value) -> Vec<String> {
    // Match Claude's persisted-history normalization: unique valid UUIDs, last 200.
    let mut ids=Vec::new();
    if let Some(items)=v["priorCliSessionIds"].as_array() {
        for id in items.iter().filter_map(Value::as_str).filter(|id|uuid(id)) {
            if !ids.iter().any(|old|old==id) { ids.push(id.to_owned()); }
        }
    }
    if ids.len()>200 { ids.drain(..ids.len()-200); }
    ids
}
fn history_contains(v: &Value, id: &str) -> bool { v["priorCliSessionIds"].as_array().is_some_and(|ids|ids.iter().any(|old|old.as_str()==Some(id))) }
type SessionRecord = ((String,String),Value);
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct Selection { pub session: String, pub revision: String, pub cli_id: String }
const CHOICE_PREFIX: &str = "DESKTOP_SESSION_CHOICE:";
const RESUME: [&str;8] = ["cliSessionId","priorCliSessionIds","rewindEdges","transcriptCuts","transcriptModelStates","historyOnlyCliSessionId","lastAssistantUuid","completedTurns"];
fn digest(bytes: &[u8]) -> String { Sha256::digest(bytes).iter().map(|b|format!("{b:02x}")).collect() }
#[derive(Default)]
struct Body { messages: BTreeMap<String,String>, hash: String }
fn body(path: &Path) -> Result<Body,String> {
    regular(path)?;
    let mut bytes=Vec::new();
    fs::File::open(path).map_err(|_|invalid())?.take(128*1024*1024+1).read_to_end(&mut bytes).map_err(|_|invalid())?;
    if bytes.len()>128*1024*1024 { return Err(invalid()); }
    let mut out=Body{hash:digest(&bytes),..Body::default()};
    for line in bytes.split(|b|*b==b'\n').filter(|line|!line.is_empty()) {
        let v:Value=serde_json::from_slice(line).map_err(|_|invalid())?;
        if !matches!(v["type"].as_str(),Some("user"|"assistant")) || v["isSidechain"]==true { continue; }
        let id=text(&v,"uuid")?;
        let timestamp=text(&v,"timestamp")?;
        if !uuid(id) || out.messages.len()>=100_000 { return Err(invalid()); }
        out.messages.insert(id.into(),timestamp.into());
    }
    Ok(out)
}
// Native rewind edges distinguish a deliberate rewind from writing on both branches.
// An ancestor message absent from the child is safe only if it predates the rewind.
fn succeeds(new: &SessionRecord, old: &SessionRecord, bodies: &BTreeMap<String,Body>) -> bool {
    if new.0.0==old.0.0 { return true; }
    let newer=&bodies[&new.0.0]; let older=&bodies[&old.0.0];
    if !older.messages.is_empty() && older.messages.keys().all(|id|newer.messages.contains_key(id)) && newer.messages.len()>older.messages.len() { return true; }
    if !history_contains(&new.1,&old.0.0) { return false; }
    if !history_contains(&old.1,&new.0.0) && older.messages.keys().all(|id|newer.messages.contains_key(id)) { return true; }
    let Some(edges)=new.1["rewindEdges"].as_array() else { return false; };
    let mut cursor=new.0.0.as_str(); let mut seen=std::collections::BTreeSet::new();
    let cutoff=loop {
        if !seen.insert(cursor) { return false; }
        let matching:Vec<_>=edges.iter().filter(|e|e["child"].as_str()==Some(cursor)).collect();
        if matching.len()!=1 { return false; }
        let edge=matching[0]; let Some(parent)=edge["parent"].as_str() else { return false; };
        if parent==old.0.0 { break edge["at"].as_i64(); }
        cursor=parent;
    };
    let Some(cutoff)=cutoff else { return false; };
    let Ok(clock)=rusqlite::Connection::open_in_memory() else { return false; };
    older.messages.iter().filter(|(id,_)|!newer.messages.contains_key(*id)).all(|(_,timestamp)| {
        clock.query_row("SELECT CAST(unixepoch(?1, 'subsec') * 1000 AS INTEGER)",[timestamp],|r|r.get::<_,i64>(0)).is_ok_and(|time|time<=cutoff)
    })
}
fn resolve_versions(name: &str, versions: &[SessionRecord], paths: &BTreeMap<String,Vec<PathBuf>>, choices: &[Selection], checked_bodies: &mut BTreeMap<PathBuf,String>) -> Result<SessionRecord,String> {
    let first=versions.first().ok_or_else(invalid)?;
    if versions.iter().any(|(key,_)|key.1!=first.0.1) {
        return Err(format!("桌面会话索引冲突：{} 的项目路径不同；未覆盖已有记录。",name));
    }
    let mut unique=BTreeMap::<String,SessionRecord>::new();
    for record in versions {
        let rank=|v:&Value|(v["lastActivityAt"].as_u64().unwrap_or(0),prior_cli_ids(v).len(),RESUME.iter().filter(|k|v.get(**k).is_some()).count());
        if unique.get(&record.0.0).is_none_or(|old|rank(&record.1)>rank(&old.1)) { unique.insert(record.0.0.clone(),record.clone()); }
    }
    if unique.len()==1 { return Ok(unique.into_values().next().unwrap()); }
    let mut bodies=BTreeMap::new();
    for id in unique.keys() { let path=&paths[id][0];let b=body(path)?;checked_bodies.insert(path.clone(),b.hash.clone());bodies.insert(id.clone(),b); }
    let revision=digest(&serde_json::to_vec(&(versions,bodies.iter().map(|(id,b)|(id,&b.hash)).collect::<Vec<_>>())).map_err(|_|invalid())?);
    let tips:Vec<_>=unique.values().filter(|new|unique.values().all(|old|succeeds(new,old,&bodies))).collect();
    if let Some(choice)=choices.iter().find(|c|c.session==name) {
        if choice.revision==revision {
            return unique.get(&choice.cli_id).cloned().ok_or_else(invalid);
        }
        // Changed input requires a fresh choice; never silently apply an old selection.
    } else if tips.len()==1 { return Ok(tips[0].clone()); }
    let options:Vec<_>=unique.keys().map(|id|serde_json::json!({"cli_id":id,"messages":bodies[id].messages.len(),"last_message_at":bodies[id].messages.values().max()})).collect();
    Err(format!("{CHOICE_PREFIX}{}",serde_json::json!({"session":name,"title":first.1["title"],"revision":revision,"options":options})))
}
const LINEAGE: [&str;4] = ["spawnedFrom","forkedFromSessionId","forkedAtMessageUuid","lineageDetached"];
#[cfg(test)]
fn has_lineage(v: &Value) -> bool { LINEAGE.iter().any(|k| v.get(*k).is_some_and(|v| !v.is_null())) }
// Only called with the narrow lineage whitelist below. Fill missing metadata;
// conflicting existing values (including a different parent) are never replaced.
fn merge_lineage(target: &mut Map<String,Value>, source: &Map<String,Value>) -> bool {
    for (key,value) in source {
        match target.get_mut(key) {
            None => { target.insert(key.clone(),value.clone()); }
            Some(existing) if existing == value => {}
            Some(Value::Object(existing)) if value.is_object() => {
                if !merge_lineage(existing,value.as_object().unwrap()) { return false; }
            }
            Some(_) => return false,
        }
    }
    true
}
fn valid_spawn(v: &Value) -> bool {
    v.get("spawnedFrom").is_none_or(|s| s["sessionId"].is_string() && s["taskId"].is_string())
}
fn lineage(v: &Value) -> Map<String,Value> {
    let mut out=Map::new();
    for k in ["forkedFromSessionId","forkedAtMessageUuid"] {
        if let Some(s)=v[k].as_str() { out.insert(k.into(),s.into()); }
    }
    if let Some(b)=v["lineageDetached"].as_bool() { out.insert("lineageDetached".into(),b.into()); }
    if let Some(id)=v["spawnedFrom"]["sessionId"].as_str() {
        let source=&v["spawnedFrom"];let mut parent=Map::new();parent.insert("sessionId".into(),id.into());
        // taskId is required by Claude's IPC validator, even for stopped children.
        // title/name are display metadata; no task, wake or permission state is copied.
        for k in ["taskId","title","name"] {
            if let Some(s)=source[k].as_str() { parent.insert(k.into(),s.into()); }
        }
        for k in ["sideSession","userFork","handOff"] {
            if let Some(b)=source[k].as_bool() { parent.insert(k.into(),b.into()); }
        }
        if let Some(id)=source["deletedParent"]["sessionId"].as_str() {
            parent.insert("deletedParent".into(),serde_json::json!({"sessionId":id}));
        }
        out.insert("spawnedFrom".into(),Value::Object(parent));
    }
    out
}
fn clean(v: &Value) -> Result<Value, String> {
    let mut out = lineage(v);
    let prior=prior_cli_ids(v);
    if !prior.is_empty() { out.insert("priorCliSessionIds".into(),serde_json::json!(prior)); }
    for key in ["historyOnlyCliSessionId","lastAssistantUuid"] {
        if let Some(id)=v[key].as_str().filter(|id|uuid(id)) { out.insert(key.into(),id.into()); }
    }
    if let Some(turns)=v["completedTurns"].as_u64() { out.insert("completedTurns".into(),turns.into()); }
    if let Some(edges)=v["rewindEdges"].as_array() {
        let mut normalized=Vec::new();
        for edge in edges.iter().rev().take(40).rev() {
            let mut e=Map::new();
            for key in ["id","parent","child","forkPoint","anchor"] {
                if let Some(id)=edge[key].as_str().filter(|id|uuid(id)) { e.insert(key.into(),id.into()); }
            }
            if !["id","parent","child"].iter().all(|key|e.contains_key(*key)) { continue; }
            if let Some(at)=edge["at"].as_u64() { e.insert("at".into(),at.into()); } else { continue; }
            if let Some(cwd)=edge["cwd"].as_str() { e.insert("cwd".into(),cwd.into()); }
            normalized.push(Value::Object(e));
        }
        if !normalized.is_empty() { out.insert("rewindEdges".into(),normalized.into()); }
    }
    for key in ["transcriptCuts","transcriptModelStates"] {
        if let Some(states)=v[key].as_object() {
            let mut normalized=Map::new();
            for (id,state) in states.iter().filter(|(id,_)|uuid(id)).take(200) {
                if key=="transcriptCuts" {
                    if state.as_str().is_some_and(uuid) { normalized.insert(id.clone(),state.clone()); }
                } else {
                    let mut model=Map::new();
                    for field in ["model","preRefusalModel","refusedUserMessageUuid","refusalFallbackTurnUuid"] {
                        if let Some(value)=state[field].as_str() { model.insert(field.into(),value.into()); }
                    }
                    if model.contains_key("model") { normalized.insert(id.clone(),model.into()); }
                }
            }
            if !normalized.is_empty() { out.insert(key.into(),normalized.into()); }
        }
    }
    for key in ["sessionId","cliSessionId","cwd","originCwd","worktreePath","worktreeName","branch","sourceBranch","title","titleSource"] {
        if let Some(value) = v.get(key).filter(|v| !v.is_null()) {
            if !value.is_string() { return Err(invalid()); }
            out.insert(key.into(),value.clone());
        }
    }
    for key in ["createdAt","lastActivityAt"] {
        if !v[key].as_u64().is_some_and(|n| n <= 8_640_000_000_000_000) { return Err(invalid()); }
        out.insert(key.into(),v[key].clone());
    }
    if let Some(value) = v.get("isArchived") {
        if !value.is_boolean() { return Err(invalid()); }
        out.insert("isArchived".into(),value.clone());
    }
    out.insert("permissionMode".into(),Value::String("default".into()));
    Ok(Value::Object(out))
}
pub fn sync_selected(home: &Path, choices: &[Selection], check: impl Fn() -> Result<(),String>) -> Result<Report,String> {
    #[cfg(not(target_os="macos"))] {
        let _ = (home,choices,check); return Err("桌面会话共享目前仅支持 macOS 默认 Claude App。".into());
    }
    #[cfg(target_os="macos")]
    sync_at_selected(home, &home.join("Library/Application Support/Claude/claude-code-sessions"), choices, check)
}
#[cfg(test)]
fn sync_at(home: &Path, root: &Path, check: impl Fn() -> Result<(),String>) -> Result<Report,String> {
    sync_at_selected(home,root,&[],check)
}
fn sync_at_selected(home: &Path, root: &Path, choices: &[Selection], check: impl Fn() -> Result<(),String>) -> Result<Report,String> {
    check()?;
    let mut report = Report::default();
    let mut buckets = Vec::new();
    for account in entries(root)? {
        if !account.file_name().and_then(|s|s.to_str()).is_some_and(uuid) { continue; }
        for org in entries(&account)? {
            if !org.file_name().and_then(|s|s.to_str()).is_some_and(uuid) { continue; }
            private::safe(&org)?; private::parents(&org)?;
            if !org.is_dir() || buckets.len() >= 100 { return Err(invalid()); }
            buckets.push(org);
        }
    }
    report.accounts = buckets.len();
    // Read message identifiers/times only when distinct transcript versions need comparison.
    let mut bodies = BTreeMap::<String,Vec<PathBuf>>::new();
    for project in entries(&home.join(".claude/projects"))? {
        // Existing unrelated linked project folders are not followed or changed.
        if fs::symlink_metadata(&project).map_err(|_| invalid())?.file_type().is_symlink() { continue; }
        if !project.is_dir() { continue; }
        for file in entries(&project)? {
            let Some(id) = file.file_stem().and_then(|s| s.to_str()).filter(|s| uuid(s)) else { continue; };
            if file.extension().is_none_or(|s|s!="jsonl") { continue; }
            regular(&file)?;
            bodies.entry(id.into()).or_default().push(file.clone());
            if bodies.len() > LIMIT { return Err(invalid()); }
        }
    }
    let mut candidates = BTreeMap::<String,Vec<SessionRecord>>::new();
    // Missing relation fields are old shared copies. Conflicting nonempty relations
    // have no authoritative winner, so never use them to repair another account.
    let mut relations = BTreeMap::<String,Option<Map<String,Value>>>::new();
    let mut count = 0;
    let mut originals=BTreeMap::new();
    for bucket in &buckets {
        for file in entries(bucket)? {
            let Some(name) = file.file_name().and_then(|s|s.to_str()).filter(|s|s.starts_with("local_")&&s.ends_with(".json")) else { continue; };
            count += 1; if count > LIMIT { return Err(invalid()); }
            let value = read(&file)?;
            originals.insert(file.clone(),value.clone());
            // Remote sessions and incomplete starts cannot be resumed from the local body.
            if value.get("cliSessionId").is_none_or(Value::is_null) || ["sshConfig","wslConfig","pendingFirstStart","scheduledTaskId"].iter().any(|k| value.get(*k).is_some_and(|v|!v.is_null())) {
                report.skipped += 1; continue;
            }
            let key = core(&value,name)?;
            if bodies.get(&key.0).is_none_or(|paths|paths.len()!=1) { report.skipped += 1; continue; }
            let filtered = clean(&value)?;
            let relation=lineage(&value);
            if !relation.is_empty() {
                relations.entry(name.into()).and_modify(|old| {
                    if let Some(existing)=old {
                        let detached = |r: &Map<String,Value>| r.get("lineageDetached")==Some(&Value::Bool(true));
                        if detached(existing)!=detached(&relation) || !merge_lineage(existing,&relation) { *old=None; }
                    }
                }).or_insert(Some(relation));
            }
            candidates.entry(name.into()).or_default().push((key,filtered));
        }
    }
    let mut checked_bodies=BTreeMap::new();
    let records: BTreeMap<String,SessionRecord>=candidates.iter().map(|(name,versions)|Ok((name.clone(),resolve_versions(name,versions,&bodies,choices,&mut checked_bodies)?))).collect::<Result<_,String>>()?;
    let mut plan = Vec::new();
    for bucket in &buckets {
        for (name,(key,source)) in &records {
            let path = bucket.join(name);
            let mut value=source.clone();
            if let Some(Some(relation))=relations.get(name) { value.as_object_mut().unwrap().extend(relation.clone()); }
            if matches!(relations.get(name),Some(None)) { for k in LINEAGE { value.as_object_mut().unwrap().remove(k); } }
            match fs::symlink_metadata(&path) {
                Ok(_) => {
                    let current=read(&path)?;
                    if current.get("cliSessionId").is_none_or(Value::is_null) || ["sshConfig","wslConfig","pendingFirstStart","scheduledTaskId"].iter().any(|k|current.get(*k).is_some_and(|v|!v.is_null())) { continue; }
                    let current_key=core(&current, name)?;
                    if current_key.1!=key.1 { return Err(format!("桌面会话索引冲突：{} 的项目路径不同；未覆盖已有记录。",name)); }
                    // Never convert remote/incomplete/missing-body entries into local ones.
                    if !candidates.get(name).is_some_and(|versions|versions.iter().any(|(k,_)|k==&current_key)) { continue; }
                    let mut repaired=current.clone();
                    for field in RESUME {
                        if let Some(v)=source.get(field) { repaired[field]=v.clone(); }
                        else { repaired.as_object_mut().unwrap().remove(field); }
                    }
                    if current_key.0!=key.0 {
                        repaired["lastActivityAt"]=source["lastActivityAt"].clone();
                        // State tied to the previous body must not redirect the selected version.
                        for field in ["stagedTranscriptPath","resumeConfirmed","postTurnSummary","postTurnSummaryFor"] { repaired.as_object_mut().unwrap().remove(field); }
                    } else if source["lastActivityAt"].as_u64()>current["lastActivityAt"].as_u64() {
                        repaired["lastActivityAt"]=source["lastActivityAt"].clone();
                    }
                    if current["lineageDetached"]!=true {
                        if let Some(Some(relation))=relations.get(name) {
                            let mut linked=repaired.clone();
                            if merge_lineage(linked.as_object_mut().ok_or_else(invalid)?,relation) && valid_spawn(&linked) { repaired=linked; }
                        }
                    }
                    if repaired!=current {
                        if plan.len() >= LIMIT { return Err(invalid()); }
                        plan.push((path,serde_json::to_vec(&repaired).map_err(|_|invalid())?,Some(current)));
                    }
                }
                Err(e) if e.kind()==std::io::ErrorKind::NotFound => {
                    if !valid_spawn(&value) { report.skipped+=1;continue; }
                    if plan.len() >= LIMIT { return Err(invalid()); }
                    plan.push((path,serde_json::to_vec(&value).map_err(|_| invalid())?,None));
                }
                Err(_) => return Err(invalid()),
            }
        }
    }
    check()?;
    for (path,expected) in &originals { if read(path)?!=*expected { return Err(invalid()); } }
    for (path,expected) in &checked_bodies { if body(path)?.hash!=*expected { return Err(invalid()); } }
    // Retain the complete old index privately before replacing its resume metadata.
    for (path, bytes, expected) in plan {
        let result = (|| {
            check()?; private::parents(path.parent().ok_or_else(invalid)?)?;
            let temp = path.with_file_name(format!(".baga-session-{}", super::new_id()?));
            let result = (|| {
                let mut file = private::create(&temp)?;
                file.write_all(&bytes).and_then(|_|file.sync_all()).map_err(|_| invalid())?;
                check()?;
                if let Some(expected)=expected {
                    if read(&path)?!=expected { return Err(invalid()); }
                    let old=serde_json::to_vec(&expected).map_err(|_|invalid())?;
                    let history=path.parent().ok_or_else(invalid)?.join(".baga-history");
                    private::directory(&history)?;
                    let backup=history.join(format!("{}-{}.json",expected["sessionId"].as_str().ok_or_else(invalid)?,digest(&old)));
                    if backup.try_exists().map_err(|_|invalid())? {
                        if read(&backup)?!=expected { return Err(invalid()); }
                    } else { let mut file=private::create(&backup)?;file.write_all(&old).and_then(|_|file.sync_all()).map_err(|_|invalid())?;private::sync_parent(&backup); }
                    if read(&path)?!=expected { return Err(invalid()); }
                    fs::rename(&temp,&path).map_err(|_|invalid())?;
                    report.repaired+=1;
                } else {
                    fs::hard_link(&temp,&path).map_err(|_| invalid())?;
                    report.added += 1;
                }
                Ok::<_,String>(())
            })();
            let removed = fs::remove_file(&temp);
            result?; if let Err(e)=removed { if e.kind()!=std::io::ErrorKind::NotFound { return Err(invalid()); } }
            private::sync_parent(&path); Ok::<_,String>(())
        })();
        if let Err(e) = result { return Err(format!("桌面会话共享中断；已新增 {} 个入口、更新 {} 个会话，旧索引与正文保留。{}",report.added,report.repaired,e)); }
    }
    Ok(report)
}
#[cfg(test)]
#[path="desktop_sessions_tests.rs"]
mod tests;
