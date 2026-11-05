// [INPUT]: 用户点击触发；当前默认 Claude Code 凭据（macOS 钥匙串/其他平台 .credentials.json），或已存身份的加密凭据侧车/归档。
// [OUTPUT]: 官方 OAuth 用量（5 小时会话/7 天全部模型/7 天分模型）百分比与重置时间、订阅档位及读取时间；令牌不出原生层。
// [POS]: 身份页唯一的账号联网读取。当前账号只读、不续期（CLI 拥有令牌）；已存账号由 CCBaga 代为续期，轮换结果先原子写入侧车再使用。
// [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use zeroize::Zeroizing;

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";
const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
/// Saved tokens this close to expiry are renewed before the usage read.
const RENEW_MARGIN_MS: i64 = 5 * 60 * 1000;
pub(super) struct Endpoints {
    pub usage: String,
    pub token: String,
}
impl Default for Endpoints {
    fn default() -> Self {
        Self { usage: USAGE_URL.into(), token: TOKEN_URL.into() }
    }
}
const OAUTH_BETA: &str = "oauth-2025-04-20";
const MAX_BODY: usize = 64 * 1024;
/// Tokens this close to expiry are left for the official client to refresh.
const EXPIRY_MARGIN_MS: i64 = 60 * 1000;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Window {
    pub percent: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<String>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    /// "ok" | "expired" | "no_token" | "rejected" | "failed" | "active" | "relogin" | "rotation_unsaved"
    pub status: String,
    pub fetched_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<Window>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub week: Option<Window>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub week_opus: Option<Window>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub week_sonnet: Option<Window>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// "cli" or "desktop": which local sign-in supplied the token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

struct Token {
    access: Zeroizing<String>,
    expires_at: Option<i64>,
    plan: Option<String>,
}

fn token_from(value: &Value) -> Option<Token> {
    let oauth = value.get("claudeAiOauth")?;
    let access = oauth.get("accessToken")?.as_str()?.trim();
    if access.is_empty() {
        return None;
    }
    Some(Token {
        access: Zeroizing::new(access.to_string()),
        expires_at: oauth.get("expiresAt").and_then(Value::as_i64),
        plan: super::profile::format_plan(
            oauth.get("subscriptionType").and_then(Value::as_str),
            oauth.get("rateLimitTier").and_then(Value::as_str),
            None,
        ),
    })
}

fn current_token(home: &std::path::Path) -> Result<Option<Token>, String> {
    #[cfg(target_os = "macos")]
    {
        let _ = home;
        for item in crate::native::identity_snapshot_client(false)? {
            if item["service"] != "Claude Code-credentials" {
                continue;
            }
            let Some(data) = item["data"].as_str() else { continue };
            let Ok(bytes) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data)
            else {
                continue;
            };
            let bytes = zeroize::Zeroizing::new(bytes);
            if let Some(token) = serde_json::from_slice::<Value>(&bytes).ok().as_ref().and_then(token_from) {
                return Ok(Some(token));
            }
        }
        Ok(None)
    }
    #[cfg(not(target_os = "macos"))]
    {
        use std::io::Read;
        let path = home.join(".claude/.credentials.json");
        let Ok(file) = std::fs::File::open(&path) else { return Ok(None) };
        let mut bytes = zeroize::Zeroizing::new(Vec::new());
        file.take(1024 * 1024).read_to_end(&mut bytes).map_err(|_| "本地凭据无法读取。")?;
        Ok(serde_json::from_slice::<Value>(&bytes).ok().as_ref().and_then(token_from))
    }
}

fn percent(value: Option<&Value>) -> u8 {
    value
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite())
        .map(|v| v.clamp(0.0, 100.0).round() as u8)
        .unwrap_or(0)
}
fn window(raw: &Value, key: &str) -> Option<Window> {
    let item = raw.get(key).filter(|v| v.is_object())?;
    Some(Window {
        percent: percent(item.get("utilization")),
        resets_at: item
            .get("resets_at")
            .and_then(Value::as_str)
            .filter(|s| s.len() <= 64 && !s.chars().any(char::is_control))
            .map(str::to_string),
    })
}
pub(super) fn parse(raw: &Value, plan: Option<String>, now: i64) -> Usage {
    Usage {
        status: "ok".into(),
        fetched_at: now,
        plan,
        session: window(raw, "five_hour"),
        week: window(raw, "seven_day"),
        week_opus: window(raw, "seven_day_opus"),
        week_sonnet: window(raw, "seven_day_sonnet"),
        message: None,
        source: None,
    }
}
fn outcome(status: &str, plan: Option<String>, now: i64, message: Option<String>) -> Usage {
    Usage { status: status.into(), fetched_at: now, plan, message, ..Usage::default() }
}

fn client() -> Result<reqwest::Client, (String, String)> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .connect_timeout(Duration::from_secs(6))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| ("failed".to_string(), "无法建立 HTTPS 客户端。".to_string()))
}
fn send_error(e: reqwest::Error) -> (String, String) {
    let kind = if e.is_timeout() { "请求超时" } else if e.is_connect() { "无法连接 Claude 服务" } else { "请求失败" };
    ("failed".to_string(), kind.to_string())
}
async fn bounded(mut response: reqwest::Response) -> Result<(reqwest::StatusCode, Zeroizing<Vec<u8>>), (String, String)> {
    let status = response.status();
    let mut body = Zeroizing::new(Vec::new());
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ("failed".to_string(), "响应读取中断。".to_string()))?
    {
        if body.len() + chunk.len() > MAX_BODY {
            return Err(("failed".into(), "响应超出大小限制。".into()));
        }
        body.extend_from_slice(&chunk);
    }
    Ok((status, body))
}
async fn request(url: &str, access: &str) -> Result<Value, (String, String)> {
    let response = client()?
        .get(url)
        .bearer_auth(access)
        .header("anthropic-beta", OAUTH_BETA)
        .header(reqwest::header::USER_AGENT, "CCBaga")
        .send()
        .await
        .map_err(send_error)?;
    let (status, body) = bounded(response).await?;
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(("rejected".into(), format!("服务端拒绝当前令牌（HTTP {}）。", status.as_u16())));
    }
    if !status.is_success() {
        return Err(("failed".into(), format!("服务端返回 HTTP {}。", status.as_u16())));
    }
    serde_json::from_slice(&body).map_err(|_| ("failed".into(), "用量响应格式无法识别。".into()))
}

/// Reads usage for the live default Claude Code login. Never refreshes: a refresh rotates the
/// refresh token, and a concurrent CLI refresh would then invalidate one of the two copies.
pub(super) async fn fetch_current(home: std::path::PathBuf) -> Result<Usage, String> {
    let now = super::profile::now_ms();
    let desktop_home = home.clone();
    let token = tauri::async_runtime::spawn_blocking(move || current_token(&home))
        .await
        .map_err(|_| "读取本地凭据被中断。".to_string())??;
    let cli = match token {
        None => outcome("no_token", None, now, None),
        Some(token) if token.expires_at.is_some_and(|exp| exp <= now + EXPIRY_MARGIN_MS) => {
            outcome("expired", token.plan, now, None)
        }
        Some(token) => match request(USAGE_URL, &token.access).await {
            Ok(raw) => return Ok(sourced(parse(&raw, token.plan, now), "cli")),
            Err((status, message)) => outcome(&status, token.plan, now, Some(message)),
        },
    };
    // The CLI login is missing, expired or refused: a Desktop sign-in for the same account
    // (or any Desktop sign-in when the CLI account is unknown) can still be read, never renewed.
    let desktop = tauri::async_runtime::spawn_blocking(move || {
        let account = live_account(&desktop_home);
        super::desktop_token::live(&desktop_home, account.as_deref())
    })
    .await
    .ok()
    .flatten();
    let Some(desktop) = desktop else { return Ok(cli) };
    let usage = read_with(USAGE_URL, &desktop.access, desktop.plan.clone(), now).await;
    Ok(if usage.status == "ok" { usage } else { cli })
}

fn sourced(mut usage: Usage, source: &str) -> Usage {
    usage.source = Some(source.into());
    usage
}
/// Read-only usage with a Desktop access token.
pub(super) async fn read_with(url: &str, access: &str, plan: Option<String>, now: i64) -> Usage {
    match request(url, access).await {
        Ok(raw) => sourced(parse(&raw, plan, now), "desktop"),
        Err((status, message)) => sourced(outcome(&status, plan, now, Some(message)), "desktop"),
    }
}
/// Account UUID of the live default CLI login, used to match Desktop tokens to the same account.
fn live_account(home: &std::path::Path) -> Option<String> {
    use std::io::Read;
    let path = home.join(".claude.json");
    super::private::safe(&path).ok()?;
    let mut bytes = Zeroizing::new(Vec::new());
    std::fs::File::open(path).ok()?.take(8 * 1024 * 1024).read_to_end(&mut bytes).ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    value["oauthAccount"]["accountUuid"].as_str().map(str::to_string)
}

/// Applies an OAuth token response to the saved CLI OAuth object, keeping subscription
/// metadata. Returns None when the response carries no usable access token.
pub(super) fn renewed(old: &Value, payload: &Value, now: i64) -> Option<Value> {
    let access = payload.get("access_token")?.as_str()?.trim();
    if access.is_empty() {
        return None;
    }
    let mut next = old.clone();
    let object = next.as_object_mut()?;
    object.insert("accessToken".into(), Value::String(access.into()));
    if let Some(refresh) = payload.get("refresh_token").and_then(Value::as_str).filter(|v| !v.trim().is_empty()) {
        object.insert("refreshToken".into(), Value::String(refresh.into()));
    }
    if let Some(seconds) = payload.get("expires_in").and_then(Value::as_i64).filter(|v| *v > 0) {
        object.insert("expiresAt".into(), Value::from(now.saturating_add(seconds.saturating_mul(1000))));
    }
    if let Some(seconds) = payload.get("refresh_token_expires_in").and_then(Value::as_i64).filter(|v| *v > 0) {
        object.insert("refreshTokenExpiresAt".into(), Value::from(now.saturating_add(seconds.saturating_mul(1000))));
    }
    if let Some(scope) = payload.get("scope").and_then(Value::as_str) {
        let scopes: Vec<Value> = scope.split_whitespace().map(|s| Value::String(s.into())).collect();
        if !scopes.is_empty() {
            object.insert("scopes".into(), Value::Array(scopes));
        }
    }
    Some(next)
}

/// Exchanges the refresh token. `Err("relogin")` means the server no longer accepts it.
async fn renew(url: &str, oauth: &Value, now: i64) -> Result<Value, (String, String)> {
    let refresh = oauth
        .get("refreshToken")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| ("relogin".to_string(), "该账号没有可用的续期令牌。".to_string()))?;
    let body = Zeroizing::new(
        serde_json::to_vec(&serde_json::json!({
            "grant_type": "refresh_token",
            "refresh_token": refresh,
            "client_id": CLIENT_ID,
        }))
        .map_err(|_| ("failed".to_string(), "续期请求无法生成。".to_string()))?,
    );
    let response = client()?
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(reqwest::header::USER_AGENT, "CCBaga")
        .body(body.to_vec())
        .send()
        .await
        .map_err(send_error)?;
    let (status, bytes) = bounded(response).await?;
    if status == reqwest::StatusCode::BAD_REQUEST
        || status == reqwest::StatusCode::UNAUTHORIZED
        || status == reqwest::StatusCode::FORBIDDEN
    {
        return Err(("relogin".into(), format!("服务端不再接受该账号的续期令牌（HTTP {}）。", status.as_u16())));
    }
    if !status.is_success() {
        return Err(("failed".into(), format!("续期服务返回 HTTP {}。", status.as_u16())));
    }
    let payload: Value = serde_json::from_slice(&bytes)
        .map_err(|_| ("failed".to_string(), "续期响应格式无法识别。".to_string()))?;
    renewed(oauth, &payload, now).ok_or_else(|| ("failed".into(), "续期响应缺少访问令牌。".into()))
}

/// What the blocking vault step found for a saved identity.
pub(super) enum Saved {
    /// The identity is the live workspace: its token belongs to the running CLI.
    Active,
    Found {
        record: super::database::Record,
        /// Renewable CLI OAuth object (JSON text), if the archive has one.
        cli: Option<Zeroizing<String>>,
        /// Unexpired Desktop access token and plan; read-only, never renewed.
        desktop: Option<(Zeroizing<String>, Option<String>)>,
    },
}

/// Reads the usage material for a saved identity. The archive is decrypted at most once per
/// generation (to seed the sidecars); later reads use only the small encrypted sidecars.
/// Must run while the caller holds the global identity transaction lock.
pub(super) fn saved_oauth(vault: &std::path::Path, home: &std::path::Path, id: &str) -> Result<Saved, String> {
    let store = super::store::Store::open_for_transfer(vault, home)?;
    let record = store.record(id)?;
    let (account_id, workspace_id) = super::catalog::current_ids(vault, home, &store.db)?;
    if workspace_id.as_deref() == Some(id) || account_id.as_deref() == Some(id) {
        return Ok(Saved::Active);
    }
    let mut cli = super::sidecar::read(vault, &store.key, &record).ok().flatten();
    let seed = match super::sidecar::read_seed(vault, &store.key, &record).ok().flatten() {
        Some(seed) => seed,
        None => {
            let stage = store.load(&record)?;
            let seed = super::sidecar::seed_from_stage(&stage, &record);
            if cli.is_none() {
                if let Some(oauth) = super::sidecar::stage_oauth(&stage)? {
                    super::sidecar::write(vault, &store.key, &record, &oauth)?;
                    cli = Some(Zeroizing::new(oauth.to_string()));
                }
            }
            drop(stage);
            super::sidecar::write_seed(vault, &store.key, &seed)?;
            seed
        }
    };
    if !seed.cli_present {
        cli = None;
    }
    let now = super::profile::now_ms();
    let desktop = seed
        .desktop_token
        .as_ref()
        .filter(|_| seed.desktop_expires_at.is_some_and(|exp| exp > now + RENEW_MARGIN_MS))
        .map(|token| (Zeroizing::new(token.clone()), seed.desktop_plan.clone()));
    Ok(Saved::Found { record, cli, desktop })
}

/// Saved-identity usage: renew if needed (persisting the rotation first), then read usage.
/// `persist` stores renewed OAuth; returning Err after a successful renewal is reported as
/// `rotation_unsaved` because the server has already retired the previous refresh token.
pub(super) async fn fetch_saved(
    oauth_text: Zeroizing<String>,
    endpoints: &Endpoints,
    persist: impl FnOnce(Value) -> Result<(), String>,
) -> Usage {
    let now = super::profile::now_ms();
    let Ok(mut oauth) = serde_json::from_str::<Value>(&oauth_text) else {
        return outcome("no_token", None, now, None);
    };
    let plan = token_from(&serde_json::json!({ "claudeAiOauth": oauth.clone() })).and_then(|t| t.plan);
    let expires = oauth.get("expiresAt").and_then(Value::as_i64);
    if expires.is_none_or(|exp| exp <= now + RENEW_MARGIN_MS) {
        match renew(&endpoints.token, &oauth, now).await {
            Ok(next) => {
                if let Err(error) = persist(next.clone()) {
                    return outcome(
                        "rotation_unsaved",
                        plan,
                        now,
                        Some(format!("续期成功但保存失败：{error}")),
                    );
                }
                oauth = next;
            }
            Err((status, message)) => return outcome(&status, plan, now, Some(message)),
        }
    }
    let Some(token) = token_from(&serde_json::json!({ "claudeAiOauth": oauth })) else {
        return outcome("no_token", plan, now, None);
    };
    sourced(match request(&endpoints.usage, &token.access).await {
        Ok(raw) => parse(&raw, token.plan.or(plan), now),
        Err((status, message)) => outcome(&status, token.plan.or(plan), now, Some(message)),
    }, "cli")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_official_usage_shape_and_clamps() {
        let raw = serde_json::json!({
            "five_hour": {"utilization": 12.6, "resets_at": "2026-11-05T08:00:00Z"},
            "seven_day": {"utilization": 140.0, "resets_at": null},
            "seven_day_opus": null,
            "seven_day_sonnet": {"utilization": -3}
        });
        let usage = parse(&raw, Some("Pro".into()), 7);
        assert_eq!(usage.session, Some(Window { percent: 13, resets_at: Some("2026-11-05T08:00:00Z".into()) }));
        assert_eq!(usage.week.as_ref().unwrap().percent, 100);
        assert!(usage.week_opus.is_none());
        assert_eq!(usage.week_sonnet.as_ref().unwrap().percent, 0);
        assert_eq!(usage.plan.as_deref(), Some("Pro"));
    }
    #[test]
    fn token_requires_access_and_reads_plan_without_exposing_refresh() {
        let value = serde_json::json!({"claudeAiOauth": {"accessToken": "a", "refreshToken": "r", "expiresAt": 5, "subscriptionType": "max", "rateLimitTier": "default_claude_max_20x"}});
        let token = token_from(&value).unwrap();
        assert_eq!(token.expires_at, Some(5));
        assert_eq!(token.plan.as_deref(), Some("Max 20x"));
        assert!(token_from(&serde_json::json!({"claudeAiOauth": {"accessToken": " "}})).is_none());
        let public = serde_json::to_string(&outcome("expired", token.plan, 1, None)).unwrap();
        assert!(!public.contains("\"a\"") && !public.contains("refresh"));
    }
}
