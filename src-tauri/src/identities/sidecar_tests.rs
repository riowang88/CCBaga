// [INPUT]: 临时 HOME/保险库、虚构 CLI 账号与本机回环 HTTP 桩。
// [OUTPUT]: 凭据侧车的种子/覆盖/代次失效/篡改回退/删除回收，以及代为续期的轮换先落盘、失效与保存失败回归。
// [POS]: 侧车与 usage 续期的隔离回归；不访问真实账号、钥匙串或网络。
// [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
use super::optimization_tests::Fixture;
use super::*;
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
};

fn sidecars(f: &Fixture) -> Vec<String> {
    fs::read_dir(&f.vault)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| sidecar::parse_name(n).is_some())
        .collect()
}

#[test]
fn renewed_sidecar_is_restored_and_superseded_by_next_generation() {
    let f = Fixture::new();
    let p = Policy::default();
    let r = Reporter::channel(None);
    let mut s = store::Store::open(&f.vault, &f.home).unwrap();
    f.login("A", "org", "a");
    s.switch("empty", "A", &p, &r).unwrap();
    let a = s.records()[0].clone();
    // Saving seeds the renewable CLI sidecar and the read-only usage seed.
    let names = sidecars(&f);
    assert_eq!(names.iter().filter(|n| n.ends_with(".cred")).count(), 1);
    assert_eq!(names.iter().filter(|n| n.ends_with(".usage")).count(), 1);
    assert!(sidecar::read(&f.vault, &s.key, &a).unwrap().unwrap().contains("\"a\""));
    sidecar::write(
        &f.vault,
        &s.key,
        &a,
        &serde_json::json!({"accessToken":"renewed","refreshToken":"r2","expiresAt":9}),
    )
    .unwrap();
    f.login("B", "org", "b");
    s.switch(&a.id, "B", &p, &r).unwrap();
    assert_eq!(f.token(), "renewed");
    // Switching A out writes a new generation containing the renewed token; the old
    // generation's sidecar is collected and a fresh one is seeded.
    let b = s.records().into_iter().find(|v| v.id != a.id).unwrap();
    s.switch(&b.id, "A", &p, &r).unwrap();
    let a2 = s.record(&a.id).unwrap();
    assert_ne!(a2.archive, a.archive);
    assert!(sidecars(&f).iter().all(|n| !n.contains(a.archive.trim_end_matches(".baga"))));
    assert!(sidecar::read(&f.vault, &s.key, &a2).unwrap().unwrap().contains("renewed"));
    assert_eq!(f.token(), "b");
}

#[test]
fn unreadable_sidecar_falls_back_to_archive_and_delete_removes_it() {
    let f = Fixture::new();
    let p = Policy::default();
    let r = Reporter::channel(None);
    let mut s = store::Store::open(&f.vault, &f.home).unwrap();
    f.login("A", "org", "a");
    s.switch("empty", "A", &p, &r).unwrap();
    let a = s.records()[0].clone();
    let name = sidecars(&f).pop().unwrap();
    fs::write(f.vault.join(&name), b"BAGAID01 tampered").unwrap();
    f.login("B", "org", "b");
    s.switch(&a.id, "B", &p, &r).unwrap();
    assert_eq!(f.token(), "a");
    let b = s.records().into_iter().find(|v| v.id != a.id).unwrap();
    assert!(sidecars(&f).iter().any(|n| n.starts_with(&b.id)));
    s.delete(&b.id, &p, &r).unwrap();
    assert!(sidecars(&f).iter().all(|n| !n.starts_with(&b.id)));
}

#[test]
fn sidecar_rejects_other_identity_and_names_are_strict() {
    let f = Fixture::new();
    let s = store::Store::open(&f.vault, &f.home).unwrap();
    let record = |id: &str, archive: &str| database::Record {
        id: id.into(),
        name: "x".into(),
        archive: archive.into(),
        saved_at: 0,
        bytes: 0,
        policy: Policy::default(),
        empty: false,
        account_key: None,
    };
    let (one, two, gen) = (new_id().unwrap(), new_id().unwrap(), format!("{}.baga", new_id().unwrap()));
    sidecar::write(&f.vault, &s.key, &record(&one, &gen), &serde_json::json!({"accessToken":"x"})).unwrap();
    let file = f.vault.join(format!("{one}-{}.cred", gen.trim_end_matches(".baga")));
    fs::copy(&file, f.vault.join(format!("{two}-{}.cred", gen.trim_end_matches(".baga")))).unwrap();
    assert!(sidecar::read(&f.vault, &s.key, &record(&two, &gen)).is_err());
    assert!(sidecar::read(&f.vault, &[7; 32], &record(&one, &gen)).is_err());
    assert!(sidecar::parse_name("x-y.cred").is_none());
    assert!(sidecar::write(&f.vault, &s.key, &record(&one, "bad"), &serde_json::json!({})).is_err());
}

// ----- 代为续期：本机回环 HTTP 桩 -----
fn stub(responses: Vec<(&'static str, &'static str)>) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for (status, body) in responses {
            let Ok((mut stream, _)) = listener.accept() else { return };
            let mut buffer = vec![0; 16 * 1024];
            let n = stream.read(&mut buffer).unwrap_or(0);
            log.lock().unwrap().push(String::from_utf8_lossy(&buffer[..n]).into_owned());
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    (base, seen)
}
fn endpoints(base: &str) -> usage::Endpoints {
    usage::Endpoints { usage: format!("{base}/usage"), token: format!("{base}/token") }
}
fn expired() -> zeroize::Zeroizing<String> {
    zeroize::Zeroizing::new(
        serde_json::json!({"accessToken":"old","refreshToken":"r1","expiresAt":1,"subscriptionType":"pro"}).to_string(),
    )
}

#[test]
fn renewal_persists_rotation_before_reading_usage() {
    let (base, seen) = stub(vec![
        ("200 OK", r#"{"access_token":"new","refresh_token":"r2","expires_in":28800}"#),
        ("200 OK", r#"{"five_hour":{"utilization":39.0,"resets_at":"2026-11-05T00:00:00Z"},"seven_day":{"utilization":6.0}}"#),
    ]);
    let saved = Arc::new(Mutex::new(None));
    let sink = saved.clone();
    let usage = tauri::async_runtime::block_on(usage::fetch_saved(expired(), &endpoints(&base), move |next| {
        *sink.lock().unwrap() = Some(next);
        Ok(())
    }));
    assert_eq!(usage.status, "ok");
    assert_eq!(usage.session.unwrap().percent, 39);
    assert_eq!(usage.plan.as_deref(), Some("Pro"));
    let next = saved.lock().unwrap().clone().unwrap();
    assert_eq!(next["refreshToken"], "r2");
    assert_eq!(next["subscriptionType"], "pro");
    let requests = seen.lock().unwrap();
    assert!(requests[0].starts_with("POST /token") && requests[0].contains("\"refresh_token\":\"r1\""));
    assert!(requests[1].starts_with("GET /usage") && requests[1].contains("Bearer new"));
}

#[test]
fn rejected_refresh_asks_for_relogin_and_unsaved_rotation_is_reported() {
    let (base, _) = stub(vec![("400 Bad Request", r#"{"error":"invalid_grant"}"#)]);
    let usage = tauri::async_runtime::block_on(usage::fetch_saved(expired(), &endpoints(&base), |_| {
        panic!("a rejected refresh must not persist anything")
    }));
    assert_eq!(usage.status, "relogin");

    let (base, seen) = stub(vec![("200 OK", r#"{"access_token":"new","refresh_token":"r2","expires_in":60}"#)]);
    let usage = tauri::async_runtime::block_on(usage::fetch_saved(expired(), &endpoints(&base), |_| {
        Err("disk full".into())
    }));
    assert_eq!(usage.status, "rotation_unsaved");
    // The usage read is skipped once the rotation could not be saved.
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[test]
fn unexpired_saved_token_reads_usage_without_renewal() {
    let (base, seen) = stub(vec![("200 OK", r#"{"seven_day":{"utilization":12}}"#)]);
    let fresh = zeroize::Zeroizing::new(
        serde_json::json!({"accessToken":"live","refreshToken":"r1","expiresAt":i64::MAX}).to_string(),
    );
    let usage = tauri::async_runtime::block_on(usage::fetch_saved(fresh, &endpoints(&base), |_| {
        panic!("no renewal expected")
    }));
    assert_eq!(usage.status, "ok");
    assert_eq!(usage.week.unwrap().percent, 12);
    assert!(seen.lock().unwrap()[0].starts_with("GET /usage"));
}
