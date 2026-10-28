// [INPUT]: 用户指定的目标邮箱、官方 CLI 登录子进程、当前受管 B 资料与 Cleaner 互斥。
// [OUTPUT]: 受控官方链接与有界登录结果与登录专属进程冲突提示；不保存档案、不输出授权 URL 或凭据。
// [POS]: 首版 macOS 原生 CLI 登录边界，取消不承诺撤销官方已经提交的登录。
// [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn official_authorization_is_narrow_and_callback_remains_local() {
        let valid="https://claude.ai/oauth/authorize?redirect_uri=http%3A%2F%2Flocalhost%3A49152%2Fcallback&response_type=code&client_id=test&state=opaque&code_challenge=opaque&code_challenge_method=S256";
        assert!(authorization_url(valid).is_ok());
        assert!(authorization_url(&valid.replace("claude.ai", "claude.com")).is_ok());
        for value in [
            valid.replace("claude.ai", "evil.example"),
            valid.replace("https:", "http:"),
            valid.replace("localhost", "evil.example"),
            valid.replace("S256", "plain"),
            valid.replace("/oauth/authorize", "/redirect"),
            format!("{valid}&redirect_uri=https://evil.example"),
            format!("{valid}\n"),
        ] {
            assert!(authorization_url(&value).is_err());
        }
    }
    #[test]
    fn native_2_1_307_claudeai_authorization_endpoint_is_supported() {
        // Endpoint and parameter shape from the installed official 2.1.307 binary.
        // State and challenge are synthetic; no real authorization material is retained.
        let url = "https://claude.com/cai/oauth/authorize?code=true&client_id=9d1c250a-e61b-44d9-88ed-5944d1962f5e&response_type=code&redirect_uri=http%3A%2F%2Flocalhost%3A49152%2Fcallback&scope=user%3Ainference&code_challenge=fixture&code_challenge_method=S256&state=fixture";
        assert!(authorization_url(url).is_ok(), "official CLI endpoint must reach the selected browser");
        for invalid in [
            url.replace("claude.com/cai", "claude.ai/cai"),
            url.replace("claude.com/cai", "evil.example/cai"),
            url.replace("/cai/oauth/authorize", "/cai/oauth/authorize/extra"),
            url.replace("https://claude.com", "http://claude.com"),
            url.replace("localhost", "evil.example"),
            url.replace("%2Fcallback", "%2Fother"),
            url.replace("S256", "plain"),
            format!("{url}&redirect_uri=http://localhost:49153/callback"),
        ] {
            assert!(authorization_url(&invalid).is_err());
        }
    }
    #[test]
    fn login_conflicts_name_clients_without_changing_the_exit_requirement() {
        assert!(login_processes_clear(Ok(vec![])).is_ok());
        let unknown = login_processes_clear(Err("fixture query failed".into())).unwrap_err();
        assert!(unknown.contains("未发起登录") && unknown.contains("fixture query failed"));
        for (name, label) in [("Claude Code", "CLI"), ("Claude Desktop", "App"), ("Claude Helper", "Claude Helper")] {
            let error = login_processes_clear(Ok(vec![crate::cleaner::ClaudeProcess {
                pid: 42, name: name.into(), executable: "/private/fixture-must-not-leak".into(),
                #[cfg(any(windows, target_os = "macos"))]
                created: 1,
            }])).unwrap_err();
            assert!(error.contains(label) && error.contains("PID 42"));
            assert!(error.contains("B 浏览器保持打开"));
            assert!(!error.contains("切换身份") && !error.contains("fixture-must-not-leak"));
        }
    }
    #[test]
    fn only_official_logged_in_matching_email_can_complete() {
        let mut value = serde_json::json!({"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty","email":"one@example.test","orgId":"org","subscriptionType":"pro"});
        assert!(matching_status(&value, "ONE@example.test").is_ok());
        assert!(matching_status(&value, "other@example.test").is_err());
        value["loggedIn"] = false.into();
        assert!(matching_status(&value, "one@example.test").is_err());
        value["loggedIn"] = true.into();
        value["authMethod"] = "api_key".into();
        assert!(matching_status(&value, "one@example.test").is_err());
    }
}
use serde::Serialize;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;
#[derive(Default)]
pub struct Login {
    cancelled: AtomicBool,
}
#[derive(Serialize)]
pub struct LoginResult {
    email: String,
    organization: Option<String>,
    subscription: Option<String>,
}
fn field(value: &Value, key: &str) -> Option<String> {
    value[key]
        .as_str()
        .map(str::trim)
        .filter(|v| !v.is_empty() && v.len() <= 254 && !v.chars().any(char::is_control))
        .map(str::to_owned)
}
fn matching_status(value: &Value, expected: &str) -> Result<LoginResult, String> {
    let email = field(value, "email").ok_or("CLI 身份无法确认；未关联档案。")?;
    if value["loggedIn"] != true
        || value["authMethod"] != "claude.ai"
        || value["apiProvider"] != "firstParty"
        || !email.eq_ignore_ascii_case(expected.trim())
    {
        return Err("CLI 登录身份与目标不符或无法确认；未登出、未关联档案。".into());
    }
    Ok(LoginResult {
        email,
        organization: field(value, "orgName"),
        subscription: field(value, "subscriptionType"),
    })
}
fn authorization_url(raw: &str) -> Result<reqwest::Url, String> {
    let bad = || "CLI 授权链接不受支持；未打开浏览器。".to_string();
    if raw.len() > 8192 || raw.chars().any(char::is_control) {
        return Err(bad());
    }
    let url = reqwest::Url::parse(raw).map_err(|_| bad())?;
    if url.scheme() != "https"
        || !matches!(
            (url.host_str(), url.path()),
            (Some("claude.ai" | "claude.com"), "/oauth/authorize")
                | (Some("claude.com"), "/cai/oauth/authorize")
        )
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
    {
        return Err(bad());
    }
    let mut pairs = std::collections::HashMap::new();
    for (key, value) in url.query_pairs() {
        if pairs.insert(key.to_string(), value.to_string()).is_some() {
            return Err(bad());
        }
    }
    for key in ["client_id", "state", "code_challenge"] {
        if pairs.get(key).is_none_or(String::is_empty) {
            return Err(bad());
        }
    }
    if pairs.get("response_type").map(String::as_str) != Some("code")
        || pairs.get("code_challenge_method").map(String::as_str) != Some("S256")
    {
        return Err(bad());
    }
    let callback =
        reqwest::Url::parse(pairs.get("redirect_uri").ok_or_else(bad)?).map_err(|_| bad())?;
    if callback.scheme() != "http"
        || !matches!(callback.host_str(), Some("localhost" | "127.0.0.1"))
        || callback.port().is_none()
        || callback.path() != "/callback"
        || !callback.username().is_empty()
        || callback.password().is_some()
        || callback.query().is_some()
        || callback.fragment().is_some()
    {
        return Err(bad());
    }
    Ok(url)
}
// Preserve the existing CLI/App exclusion; report the actual login blocker, not a switch error.
fn login_processes_clear(processes: Result<Vec<crate::cleaner::ClaudeProcess>, String>) -> Result<(), String> {
    let processes = processes.map_err(|error| format!("无法确认 CLI 登录前的进程状态；未发起登录。详情：{error}"))?;
    if processes.is_empty() { return Ok(()); }
    let mut entries = processes.iter().take(32).map(|process| {
        let name = match process.name.as_str() {
            "Claude Code" => "CLI",
            "Claude Desktop" => "App",
            name => name,
        };
        let name: String = name.chars().filter(|c| !c.is_control()).take(120).collect();
        format!("{name} (PID {})", process.pid)
    }).collect::<Vec<_>>();
    if processes.len() > 32 { entries.push("仅列出前 32 个进程".into()); }
    Err(format!("CLI 登录尚未开始。请先保存任务并退出以下程序，再重试（B 浏览器保持打开）：{}", entries.join(", ")))
}

#[tauri::command]
pub fn identities_login_cancel(app: tauri::AppHandle) {
    app.state::<Login>().cancelled.store(true, Ordering::SeqCst);
}
#[tauri::command]
pub async fn identities_login(
    app: tauri::AppHandle,
    email: String,
    on_progress: tauri::ipc::Channel<crate::progress::Progress>,
) -> Result<LoginResult, String> {
    let email = email.trim().to_owned();
    if email.len() > 254
        || !email.contains('@')
        || email.chars().any(char::is_whitespace)
        || email.chars().any(char::is_control)
    {
        return Err("请填写要在 B 中授权的账号邮箱。".into());
    }
    let cleaner = app.state::<crate::cleaner::Cleaner>();
    cleaner.enter()?;
    app.state::<Login>()
        .cancelled
        .store(false, Ordering::SeqCst);
    let worker = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let reporter = crate::progress::Reporter::channel(Some(on_progress));
        #[cfg(target_os = "macos")]
        {
            run(&worker, &email, &reporter)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (worker, email, reporter);
            Err("受控登录目前仅验证 macOS 原生 CLI；现有保存与切换仍可使用。".into())
        }
    })
    .await
    .map_err(|_| "CLI 登录任务被中断；请核对官方登录状态。".to_string())
    .and_then(|x| x);
    cleaner.leave();
    result
}
#[cfg(target_os = "macos")]
use std::{
    fs,
    io::Read,
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
#[cfg(target_os = "macos")]
struct Temporary(PathBuf);
#[cfg(target_os = "macos")]
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[cfg(target_os = "macos")]
struct ChildGuard(Child);
#[cfg(target_os = "macos")]
impl Drop for ChildGuard {
    fn drop(&mut self) {
        unsafe {
            kill(-(self.0.id() as i32), 9);
        }
        let _ = self.0.wait();
    }
}
#[cfg(target_os = "macos")]
extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
}
#[cfg(target_os = "macos")]
fn output(mut command: Command, seconds: u64) -> Result<(bool, Vec<u8>), String> {
    command.process_group(0);
    let mut child = ChildGuard(
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "无法启动官方 CLI。")?,
    );
    let mut pipe = child.0.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.by_ref()
            .take(32769)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let start = Instant::now();
    let status = loop {
        match child.0.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() < Duration::from_secs(seconds) => {
                std::thread::sleep(Duration::from_millis(50))
            }
            _ => return Err("CLI 状态查询超时或失败。".into()),
        }
    };
    drop(child);
    let bytes = reader
        .join()
        .map_err(|_| "CLI 状态查询失败。")?
        .map_err(|_| "CLI 状态查询失败。")?;
    if bytes.len() > 32768 {
        return Err("CLI 状态回报过大。".into());
    }
    Ok((status.success(), bytes))
}
#[cfg(target_os = "macos")]
fn command(binary: &Path, cwd: &Path) -> Command {
    let mut cmd = Command::new(binary);
    cmd.arg("--safe-mode").current_dir(cwd);
    cmd
}
#[cfg(target_os = "macos")]
fn auth_status(binary: &Path, cwd: &Path) -> Result<Value, String> {
    let mut cmd = command(binary, cwd);
    cmd.args(["auth", "status"]);
    let (ok, bytes) = output(cmd, 20)?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| "CLI 身份状态无法读取。")?;
    if value["loggedIn"] == true && !ok {
        return Err("CLI 身份状态无法读取。".into());
    }
    Ok(value)
}
#[cfg(target_os = "macos")]
fn receiver(root: &Path) -> Result<(), String> {
    for (name,script) in [("capture", "#!/bin/sh\numask 077\n[ \"$#\" -eq 1 ] || exit 64\nprintf '%s' \"$1\" > \"$CCBAGA_LOGIN_CAPTURE.tmp\" && /bin/mv \"$CCBAGA_LOGIN_CAPTURE.tmp\" \"$CCBAGA_LOGIN_CAPTURE\"\n"),("open","#!/bin/sh\numask 077\nprintf blocked > \"$CCBAGA_LOGIN_BLOCKED\"\nexit 64\n")] {
        let path=root.join(name);super::private::write(&path,script.as_bytes())?;fs::set_permissions(path,fs::Permissions::from_mode(0o700)).map_err(|_|"无法准备受控登录。")?;
    }
    Ok(())
}
#[cfg(target_os = "macos")]
fn login_wait(
    mut cmd: Command,
    root: &Path,
    cancel: &AtomicBool,
    timeout: Duration,
    mut open: impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    cmd.env("BROWSER", root.join("capture"))
        .env("CCBAGA_LOGIN_CAPTURE", root.join("url"))
        .env("CCBAGA_LOGIN_BLOCKED", root.join("blocked"))
        .env(
            "PATH",
            format!(
                "{}:{}",
                root.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    let mut child = ChildGuard(cmd.spawn().map_err(|_| "无法启动官方登录。")?);
    let start = Instant::now();
    let mut opened = false;
    loop {
        if cancel.load(Ordering::SeqCst) || start.elapsed() >= timeout {
            return Err(
                "CLI 登录已取消或超时；官方可能已写入凭据，请刷新核对，未关联档案。".into(),
            );
        }
        if root.join("blocked").exists() {
            return Err("CLI 尝试默认浏览器跳转，已中止；未关联档案。".into());
        }
        if !opened && root.join("url").exists() {
            let path = root.join("url");
            super::private::safe(&path)?;
            let mut bytes = Vec::new();
            fs::File::open(&path)
                .map_err(|_| "授权链接无法读取。")?
                .take(8193)
                .read_to_end(&mut bytes)
                .map_err(|_| "授权链接无法读取。")?;
            let raw = String::from_utf8(bytes).map_err(|_| "授权链接无效。")?;
            authorization_url(&raw)?;
            open(&raw)?;
            fs::remove_file(path).map_err(|_| "授权链接无法清理。")?;
            opened = true;
        }
        if let Some(status) = child.0.try_wait().map_err(|_| "无法查询登录进程。")? {
            return if status.success() && opened {
                Ok(())
            } else {
                Err("官方登录未完成；请核对授权页、账号权益或网络，未关联档案。".into())
            };
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
#[cfg(target_os = "macos")]
fn run(
    app: &tauri::AppHandle,
    email: &str,
    reporter: &crate::progress::Reporter,
) -> Result<LoginResult, String> {
    let home = crate::cleaner::identity::root()?;
    for key in [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_TOKEN_FILE_DESCRIPTOR",
        "ANTHROPIC_CUSTOM_HEADERS",
        "ANTHROPIC_BASE_URL",
        "CLAUDE_CONFIG_DIR",
        "CLAUDE_CODE_USE_BEDROCK",
        "CLAUDE_CODE_USE_VERTEX",
        "CLAUDE_CODE_USE_FOUNDRY",
    ] {
        if std::env::var_os(key).is_some() {
            return Err(
                "检测到自定义 CLI 认证或目录配置；受控登录不支持此环境，现有保存与切换仍可使用。"
                    .into(),
            );
        }
    }
    for name in ["settings.json", "settings.local.json"] {
        let path = home.join(".claude").join(name);
        if path.exists() {
            super::private::parents(path.parent().unwrap())?;
            super::private::safe(&path)?;
            let mut bytes = Vec::new();
            fs::File::open(path)
                .map_err(|_| "CLI 设置无法读取。")?
                .take(1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "CLI 设置无法读取。")?;
            if bytes.len() > 1024 * 1024 {
                return Err("CLI 设置过大，无法确认登录环境。".into());
            }
            let value: Value = serde_json::from_slice(&bytes).map_err(|_| "CLI 设置无法读取。")?;
            if value.get("apiKeyHelper").is_some_and(|x| !x.is_null())
                || value
                    .get("env")
                    .is_some_and(|x| x.as_object().is_none_or(|m| !m.is_empty()))
            {
                return Err("CLI 设置包含自定义环境或认证命令；未发起受控登录。".into());
            }
        }
    }
    let binary =
        fs::canonicalize(home.join(".local/bin/claude")).map_err(|_| "未找到受支持的原生 CLI。")?;
    let versions = fs::canonicalize(home.join(".local/share/claude/versions"))
        .map_err(|_| "未找到受支持的原生 CLI。")?;
    if binary.parent() != Some(versions.as_path()) {
        return Err("受控登录仅支持默认原生 CLI 安装。".into());
    }
    let root = Temporary(
        fs::canonicalize("/tmp")
            .map_err(|_| "无法定位私有临时目录。")?
            .join(format!("ccbaga-login-{}", super::new_id()?)),
    );
    super::private::create_directory(&root.0)?;
    receiver(&root.0)?;
    let mut version = Command::new(&binary);
    version.arg("--version").current_dir(&root.0);
    let (ok, bytes) = output(version, 20)?;
    if !ok || String::from_utf8_lossy(&bytes).trim() != "2.1.307 (Claude Code)" {
        return Err(
            "当前 CLI 版本尚未验证受控链接交付；未发起登录，现有保存与切换仍可使用。".into(),
        );
    }
    let status = auth_status(&binary, &root.0)?;
    if status["loggedIn"] == true {
        return matching_status(&status, email);
    }
    if status["loggedIn"] != false
        || status["authMethod"] != "none"
        || status["apiProvider"] != "firstParty"
    {
        return Err("CLI 认证状态未知；未发起登录。".into());
    }
    login_processes_clear(crate::cleaner::identity::running_processes())?;
    let target = crate::browser::login_target(app)?;
    let mut cmd = command(&binary, &root.0);
    cmd.args(["auth", "login", "--claudeai"]);
    reporter.log(
        "login",
        "running",
        "等待官方授权；请在当前 B 资料中核对目标邮箱并完成验证。",
        0,
        None,
    );
    login_wait(
        cmd,
        &root.0,
        &app.state::<Login>().cancelled,
        Duration::from_secs(600),
        |url| crate::browser::login_open(app, &target, url),
    )?;
    let result = matching_status(&auth_status(&binary, &root.0)?, email)?;
    reporter.log(
        "login",
        "ok",
        "CLI 官方登录已完成；邮箱符合输入目标，B 组织未自动核对，尚未关联档案。",
        1,
        Some(1),
    );
    Ok(result)
}
#[cfg(all(test, target_os = "macos"))]
mod process_tests {
    use super::*;
    const URL:&str="https://claude.com/cai/oauth/authorize?redirect_uri=http%3A%2F%2Flocalhost%3A49152%2Fcallback&response_type=code&client_id=fixture&state=fixture&code_challenge=fixture&code_challenge_method=S256";
    fn fixture() -> Temporary {
        let root = Temporary(fs::canonicalize("/tmp").unwrap().join(format!(
            "ccbaga-login-test-{}",
            crate::identities::new_id().unwrap()
        )));
        super::super::private::create_directory(&root.0).unwrap();
        receiver(&root.0).unwrap();
        root
    }
    fn fake(root: &Path, script: &str) -> Command {
        let mut cmd = Command::new("/bin/sh");
        cmd.args(["-c", script])
            .current_dir(root)
            .env("FIXTURE_URL", URL);
        cmd
    }
    #[test]
    fn receiver_delivers_once_without_default_browser_or_url_output() {
        let root = fixture();
        let cancel = AtomicBool::new(false);
        let mut count = 0;
        login_wait(
            fake(&root.0, "\"$BROWSER\" \"$FIXTURE_URL\"; /bin/sleep .2"),
            &root.0,
            &cancel,
            Duration::from_secs(2),
            |url| {
                assert_eq!(url, URL);
                count += 1;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(count, 1);
        assert!(!root.0.join("url").exists());
        assert!(!root.0.join("blocked").exists());
    }
    #[test]
    fn fallback_and_wrong_browser_target_cannot_report_success() {
        let root = fixture();
        let cancel = AtomicBool::new(false);
        assert!(login_wait(
            fake(&root.0, "open \"$FIXTURE_URL\"; /bin/sleep 2"),
            &root.0,
            &cancel,
            Duration::from_secs(2),
            |_| panic!("must not open")
        )
        .unwrap_err()
        .contains("默认浏览器"));
        fs::remove_file(root.0.join("blocked")).unwrap();
        assert!(login_wait(
            fake(&root.0, "\"$BROWSER\" \"$FIXTURE_URL\"; /bin/sleep 2"),
            &root.0,
            &cancel,
            Duration::from_secs(2),
            |_| Err("target changed".into())
        )
        .unwrap_err()
        .contains("target changed"));
    }
    #[test]
    fn timeout_cancel_and_failure_never_complete_or_leave_writer() {
        for mode in ["timeout", "cancel", "failure"] {
            let root = fixture();
            let cancel = AtomicBool::new(mode == "cancel");
            let script = if mode == "failure" {
                "exit 3"
            } else {
                "/bin/sleep .4; printf late > late-write"
            };
            assert!(login_wait(
                fake(&root.0, script),
                &root.0,
                &cancel,
                Duration::from_millis(150),
                |_| panic!("must not open")
            )
            .is_err());
            std::thread::sleep(Duration::from_millis(450));
            assert!(!root.0.join("late-write").exists());
        }
    }
}
