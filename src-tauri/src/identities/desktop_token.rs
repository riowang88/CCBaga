// [INPUT]: macOS 默认 Claude Desktop 的 config.json `oauth:tokenCacheV2`（Electron safeStorage v10）与 `Claude Safe Storage` 钥匙串口令，来自实时本机或已认证归档暂存。
// [OUTPUT]: 指定账号（或未知时任一）尚未到期、到期最晚的 Desktop OAuth 访问令牌及订阅档位，只供只读用量请求。
// [POS]: Desktop 令牌的只读解码边界：不续期、不写回、不修改 Desktop 资料；令牌与口令只在原生内存，不进入 UI、日志或数据库。
// [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::Value;
use zeroize::Zeroizing;

pub(super) const CACHE_KEY: &str = "oauth:tokenCacheV2";
const SAFE_STORAGE: &str = "Claude Safe Storage";
/// Desktop tokens this close to expiry are treated as unusable (never renewed here).
const MARGIN_MS: i64 = 5 * 60 * 1000;

pub(super) struct DesktopToken {
    pub access: Zeroizing<String>,
    pub expires_at: i64,
    pub plan: Option<String>,
}

/// Electron safeStorage on macOS: "v10" + AES-128-CBC(PKCS7), key = PBKDF2-HMAC-SHA1(password,
/// "saltysalt", 1003, 16), IV = 16 spaces.
#[cfg(target_os = "macos")]
pub(super) fn decrypt(password: &[u8], blob: &[u8]) -> Option<Zeroizing<Vec<u8>>> {
    use std::ffi::c_void;
    #[link(name = "System", kind = "dylib")]
    extern "C" {
        fn CCCrypt(
            op: u32, alg: u32, options: u32, key: *const c_void, key_length: usize, iv: *const c_void,
            data_in: *const c_void, data_in_length: usize, data_out: *mut c_void, data_out_available: usize,
            data_out_moved: *mut usize,
        ) -> i32;
    }
    let body = blob.strip_prefix(b"v10")?;
    if body.is_empty() || body.len() % 16 != 0 {
        return None;
    }
    let mut key = Zeroizing::new([0u8; 16]);
    ring::pbkdf2::derive(
        ring::pbkdf2::PBKDF2_HMAC_SHA1,
        std::num::NonZeroU32::new(1003).unwrap(),
        b"saltysalt",
        password,
        key.as_mut(),
    );
    let iv = [b' '; 16];
    let mut out = Zeroizing::new(vec![0u8; body.len()]);
    let mut moved = 0usize;
    // kCCDecrypt = 1, kCCAlgorithmAES128 = 0, kCCOptionPKCS7Padding = 1
    let status = unsafe {
        CCCrypt(1, 0, 1, key.as_ptr().cast(), 16, iv.as_ptr().cast(), body.as_ptr().cast(), body.len(),
            out.as_mut_ptr().cast(), out.len(), &mut moved)
    };
    if status != 0 {
        return None;
    }
    out.truncate(moved);
    Some(out)
}
#[cfg(not(target_os = "macos"))]
pub(super) fn decrypt(_: &[u8], _: &[u8]) -> Option<Zeroizing<Vec<u8>>> {
    None
}

fn cache_account(key: &str) -> Option<&str> {
    key.strip_prefix("acct:")?.split('|').next().filter(|v| !v.is_empty())
}

/// Picks the usable entry expiring last, restricted to `account` when it is known.
pub(super) fn pick(cache: &Value, account: Option<&str>, now: i64) -> Option<DesktopToken> {
    let mut best: Option<DesktopToken> = None;
    for (key, entry) in cache.as_object()? {
        let owner = cache_account(key);
        if account.is_some_and(|wanted| owner != Some(wanted)) {
            continue;
        }
        let Some(token) = entry.get("token").and_then(Value::as_str).filter(|t| !t.trim().is_empty()) else { continue };
        let Some(expires_at) = entry.get("expiresAt").and_then(Value::as_i64) else { continue };
        if expires_at <= now + MARGIN_MS || best.as_ref().is_some_and(|b| b.expires_at >= expires_at) {
            continue;
        }
        best = Some(DesktopToken {
            access: Zeroizing::new(token.into()),
            expires_at,
            plan: super::profile::format_plan(
                entry.get("subscriptionType").and_then(Value::as_str),
                entry.get("rateLimitTier").and_then(Value::as_str),
                None,
            ),
        });
    }
    best
}

/// Decodes the encrypted cache value with any of the supplied Safe Storage items.
pub(super) fn from_parts(config: &Value, items: &[Value], account: Option<&str>, now: i64) -> Option<DesktopToken> {
    let blob = Zeroizing::new(STANDARD.decode(config.get(CACHE_KEY)?.as_str()?).ok()?);
    // Prefer Electron's own "<App> Key" entry; other entries are tried only as fallbacks.
    let mut candidates: Vec<&Value> = items.iter().filter(|i| i["service"] == SAFE_STORAGE).collect();
    candidates.sort_by_key(|i| i["account"] != "Claude Key");
    for item in candidates {
        let Some(password) = item["data"].as_str().and_then(|d| STANDARD.decode(d).ok()).map(Zeroizing::new) else { continue };
        let Some(plain) = decrypt(&password, &blob) else { continue };
        let Ok(cache) = serde_json::from_slice::<Value>(&plain) else { continue };
        return pick(&cache, account, now);
    }
    None
}

fn desktop_config(home: &std::path::Path) -> std::path::PathBuf {
    home.join("Library/Application Support/Claude/config.json")
}

/// Live default Desktop token. Reads the Safe Storage item through the existing native bridge.
pub(super) fn live(home: &std::path::Path, account: Option<&str>) -> Option<DesktopToken> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let path = desktop_config(home);
    super::private::safe(&path).ok()?;
    let text = Zeroizing::new(std::fs::read(&path).ok().filter(|b| b.len() <= 8 * 1024 * 1024)?);
    let config: Value = serde_json::from_slice(&text).ok()?;
    let items = crate::native::identity_snapshot_client(true).ok()?;
    from_parts(&config, &items, account, super::profile::now_ms())
}

/// Desktop token stored in an authenticated stage (Safe Storage item + config.json member).
pub(super) fn from_stage(stage: &super::archive::Stage, account: Option<&str>) -> Option<DesktopToken> {
    let target = desktop_config(&stage.manifest.home);
    let n = stage.manifest.sources.iter().position(|s| s.path == target && !s.keys.is_empty())?;
    let text = Zeroizing::new(std::fs::read(stage.path.join(format!("data/{n}"))).ok()?);
    let config: Value = serde_json::from_slice(&text).ok()?;
    from_parts(&config, &stage.manifest.credentials, account, super::profile::now_ms())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn picks_latest_unexpired_entry_for_the_account() {
        let cache = serde_json::json!({
            "acct:A|client:org:url": {"token":"a-old","expiresAt": 2_000_000, "subscriptionType":"pro"},
            "acct:A|client2:org:url": {"token":"a-new","expiresAt": 9_000_000, "subscriptionType":"max", "rateLimitTier":"default_claude_max_5x"},
            "acct:B|client:org:url": {"token":"b","expiresAt": 99_000_000},
            "acct:A|expired:org:url": {"token":"gone","expiresAt": 1},
        });
        let a = pick(&cache, Some("A"), 1_000_000).unwrap();
        assert_eq!(&*a.access, "a-new");
        assert_eq!(a.plan.as_deref(), Some("Max 5x"));
        assert_eq!(&*pick(&cache, None, 1_000_000).unwrap().access, "b");
        assert!(pick(&cache, Some("C"), 1_000_000).is_none());
        assert!(pick(&cache, Some("A"), 9_000_000).is_none());
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn decrypts_electron_v10_vector() {
        // Independent vector: Python `cryptography` with Electron's macOS parameters
        // (password "fixture", plaintext {"x":1}).
        let blob = STANDARD.decode(VECTOR).unwrap();
        let plain = decrypt(b"fixture", &blob).unwrap();
        assert_eq!(&*plain, br#"{"x":1}"#);
        assert!(decrypt(b"wrong", &blob).is_none_or(|p| &*p != br#"{"x":1}"#));
        assert!(decrypt(b"fixture", b"v11abc").is_none());
    }
    #[cfg(target_os = "macos")]
    const VECTOR: &str = "djEwiYbEE8/VWuN43f+/8u1WzQ==";
    #[cfg(target_os = "macos")]
    #[test]
    fn decodes_config_cache_with_safe_storage_item_and_matches_account() {
        // Independent Python vector of a real-shaped tokenCacheV2 (password "fixture").
        let config = serde_json::json!({ CACHE_KEY: "djEwrc3+iGXkGBIb9gpNviCqDoxqAo99wHmLcFgdJFkVZ4gx6yHvZHPNEbPorkglT5FKGLOdnp0tiGstK6dk5/luLJSSOGrWQEk8UYQbFXGVjjH3wR/9m4OJ9ytd3AOwHQmVTAfg9zaxvZqJCjVGukL+fKL+RYhVbaMXnC277j1wrdADpH+DttoF6dhRSxCax1eO" });
        let item = |account: &str, password: &str| serde_json::json!({"service": SAFE_STORAGE, "account": account, "data": STANDARD.encode(password)});
        let items = [item("other", "wrong"), item("Claude Key", "fixture")];
        let token = from_parts(&config, &items, Some("A"), 0).unwrap();
        assert_eq!(&*token.access, "desk-A");
        assert_eq!(token.plan.as_deref(), Some("Pro"));
        assert!(from_parts(&config, &items, Some("B"), 0).is_none());
        assert!(from_parts(&config, &[item("Claude Key", "wrong")], None, 0).is_none());
        assert!(from_parts(&serde_json::json!({}), &items, None, 0).is_none());
    }
}
