// [INPUT]: Windows Chromium 引擎、资源目录与共用资料/随机令牌会话。
// [OUTPUT]: 本地引擎发现与启动、用户主动设置默认浏览器。
// [POS]: browser 编排的 Windows 平台实现；只接收 URL/文件入口，不执行用户提供的 shell。
// [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
use std::os::windows::process::CommandExt;
fn register(_: &Path) -> Result<(), String> {
    Ok(())
}
fn register_default() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|_| "无法定位 CCBaga-A。")?;
    let script = r#"
$ErrorActionPreference='Stop'
$base='HKCU:\Software\Clients\StartMenuInternet\CCBaga-B'
$cap=$base+'\Capabilities'
$class='HKCU:\Software\Classes\CCBaga-B.URL'
$html='HKCU:\Software\Classes\CCBaga-B.HTML'
$icon='"'+$env:CCBAGA_ICON+'",0'
$command='"'+$env:CCBAGA_EXE+'" --browser "%1"'
foreach($key in @($base,$cap,($cap+'\URLAssociations'),$class,($class+'\shell\open\command'),($class+'\DefaultIcon'),($base+'\shell\open\command'),($cap+'\FileAssociations'),($html+'\shell\open\command'),($html+'\DefaultIcon'),($base+'\DefaultIcon'))) { New-Item -Path $key -Force | Out-Null }
Set-Item -LiteralPath $base -Value 'CCBaga-B'
Set-ItemProperty -LiteralPath $cap -Name ApplicationName -Value 'CCBaga-B'
Set-ItemProperty -LiteralPath $cap -Name ApplicationDescription -Value 'CCBaga companion browser'
Set-ItemProperty -LiteralPath $cap -Name ApplicationIcon -Value $icon
foreach($extension in @('.htm','.html')) { Set-ItemProperty -LiteralPath ($cap+'\FileAssociations') -Name $extension -Value 'CCBaga-B.HTML' }
Set-Item -LiteralPath $html -Value 'CCBaga-B HTML'
Set-Item -LiteralPath ($html+'\shell\open\command') -Value $command
Set-Item -LiteralPath ($html+'\DefaultIcon') -Value $icon
Set-Item -LiteralPath ($base+'\DefaultIcon') -Value $icon
foreach($protocol in @('http','https')) { Set-ItemProperty -LiteralPath ($cap+'\URLAssociations') -Name $protocol -Value 'CCBaga-B.URL' }
Set-Item -LiteralPath $class -Value 'CCBaga-B URL'
Set-ItemProperty -LiteralPath $class -Name 'URL Protocol' -Value ''
Set-Item -LiteralPath ($class+'\shell\open\command') -Value $command
Set-Item -LiteralPath ($base+'\shell\open\command') -Value $command
Set-Item -LiteralPath ($class+'\DefaultIcon') -Value $icon
New-Item -Path 'HKCU:\Software\RegisteredApplications' -Force | Out-Null
Set-ItemProperty -LiteralPath 'HKCU:\Software\RegisteredApplications' -Name 'CCBaga-B' -Value 'Software\Clients\StartMenuInternet\CCBaga-B\Capabilities'
"#;
    let status = Command::new("powershell")
        .creation_flags(0x08000000)
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("CCBAGA_EXE", &exe)
        .env(
            "CCBAGA_ICON",
            exe.parent()
                .unwrap()
                .join("browser-runtime/icons/browser.ico"),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| "无法注册 Windows 浏览器入口。")?;
    if !status.success() {
        return Err("无法注册 Windows 浏览器入口。".into());
    }
    Ok(())
}

fn binary(engine: &Path) -> PathBuf {
    engine.join("chrome.exe")
}

fn packaged_engine(resources: &Path) -> PathBuf {
    let legacy = resources.join("browser-runtime/WindowsBrowser");
    if binary(&legacy).is_file() {
        return legacy;
    }
    resources.join("WindowsBrowser")
}

fn engine(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let resources = app.path().resource_dir().map_err(|_| "资源目录不可用。")?;
    let packaged = packaged_engine(&resources);
    if binary(&packaged).is_file() || !cfg!(debug_assertions) {
        return Ok(packaged);
    }
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../browser-runtime/WindowsBrowser"))
}

fn prepare_engine(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let engine = engine(app)?;
    if !binary(&engine).is_file() {
        return Err(
            "Windows 指纹浏览器引擎缺失。请先运行 python scripts/fetch-browser.py 下载并准备引擎。"
                .into(),
        );
    }
    Ok(engine)
}

fn version(engine: &Path) -> String {
    fs::read_to_string(engine.join("version.txt"))
        .unwrap_or_default()
        .trim()
        .into()
}

pub fn open_urls(app: tauri::AppHandle, args: Vec<String>) {
    let urls = args
        .into_iter()
        .skip(1)
        .filter(|s| {
            s.starts_with("https://")
                || s.starts_with("http://")
                || s.starts_with("file://")
                || Path::new(s).is_absolute()
        })
        .collect::<Vec<_>>();
    std::thread::spawn(move || {
        if let Err(error) = start_windows_companion(&app, &urls) {
            eprintln!("CCBaga-B: {error}");
        }
    });
}

fn start_windows_companion(app: &tauri::AppHandle, urls: &[String]) -> Result<(), String> {
    crate::startup_trace("companion entry");
    let state = app.state::<Browser>();
    let _guard = state.gate.lock().unwrap();
    let engine = prepare_engine(app)?;
    crate::startup_trace("engine ready");
    let root = root(app)?;
    let running = workspaces::running_profiles(&root)?;
    let target = workspaces::prepare_active_profile(&root, &running)?;
    let settings = target.scope.clone();
    let profile = target.profile.clone();
    let slot = target.slot.clone();
    profile::check_path(&root, &profile)?;
    let config = config::load(&settings)?;
    crate::startup_trace("config ready");
    let machine = config::machine()?;
    crate::startup_trace("machine ready");
    config.validate(&machine)?;
    let running = profile::running(&profile)?;
    if !running {
        profile::preferences(&profile, &config, &machine)?;
    }
    write_launch_args(&root, &config, &machine, &profile, &version(&engine))?;
    workspaces::unblock(&root)?;
    crate::startup_trace("preferences ready");
    let reuse_running = !urls.is_empty() && running && state.session.lock().unwrap().is_some();
    let mut cmd_args = if reuse_running {
        let mut a = config::arguments(&config, &machine, &profile, "", &version(&engine));
        a.pop();
        a
    } else {
        let page = open_session(app, &config, &machine, version(&engine))?;
        crate::startup_trace("session ready");
        config::arguments(&config, &machine, &profile, &page, &version(&engine))
    };
    cmd_args.extend_from_slice(urls);
    let mut child = Command::new(binary(&engine))
        .args(cmd_args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "无法启动 Windows Chromium 浏览器。请检查引擎文件与依赖。".to_string())?;
    crate::startup_trace("companion spawned");
    let started = launch::confirm_started(&root, &slot, &profile, || {
        Ok(child.try_wait().map_err(|_| "无法读取浏览器状态。".to_string())?.is_some())
    });
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    started
}

fn default_read() -> Result<DefaultBrowser, String> {
    use windows_sys::Win32::System::Registry::HKEY_CURRENT_USER;
    let read = |protocol| {
        crate::windows::registry_string(HKEY_CURRENT_USER, &format!(r"Software\Microsoft\Windows\Shell\Associations\UrlAssociations\{protocol}\UserChoice"), "ProgId").unwrap_or_default()
    };
    let http = read("http");
    let https = read("https");
    let is_default =
        http.eq_ignore_ascii_case("CCBaga-B.URL") && https.eq_ignore_ascii_case("CCBaga-B.URL");
    let exe = std::env::current_exe().map_err(|_| "无法定位 CCBaga-A。")?;
    let installed = exe
        .parent()
        .is_some_and(|folder| binary(&packaged_engine(folder)).is_file());
    Ok(DefaultBrowser {
        is_default,
        http,
        https,
        installed,
        app: std::env::current_exe()
            .map_err(|_| "无法定位 CCBaga-A。")?
            .display()
            .to_string(),
    })
}

#[tauri::command]
pub async fn browser_set_default(app: tauri::AppHandle) -> Result<DefaultBrowser, String> {
    run(move || {
        prepare_engine(&app)?;
        register_default()?;
        crate::windows::open_external_url("ms-settings:defaultapps?registeredAppUser=CCBaga-B")?;
        default_read()
    })
    .await
}

#[cfg(test)]
mod windows_browser_tests {
    use super::*;
    #[test]
    fn ci_registration_preserves_default_associations() {
        if std::env::var("GITHUB_ACTIONS").as_deref() != Ok("true") {
            return;
        }
        use windows_sys::Win32::System::Registry::HKEY_CURRENT_USER;
        let key = r"Software\Clients\StartMenuInternet\CCBaga-B\Capabilities";
        assert!(
            crate::windows::registry_string(HKEY_CURRENT_USER, key, "ApplicationName").is_none()
        );
        let before = default_read().unwrap();
        register_default().unwrap();
        let read =
            |path: &str, name: &str| crate::windows::registry_string(HKEY_CURRENT_USER, path, name);
        assert_eq!(read(key, "ApplicationName").as_deref(), Some("CCBaga-B"));
        for protocol in ["http", "https"] {
            assert_eq!(
                read(&format!(r"{key}\URLAssociations"), protocol).as_deref(),
                Some("CCBaga-B.URL")
            );
        }
        let command = read(r"Software\Classes\CCBaga-B.URL\shell\open\command", "").unwrap();
        assert_eq!(
            command,
            format!(
                "\"{}\" --browser \"%1\"",
                std::env::current_exe().unwrap().display()
            )
        );
        let after = default_read().unwrap();
        assert_eq!(before.http, after.http);
        assert_eq!(before.https, after.https);
        let status = Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", r"Remove-Item -LiteralPath 'HKCU:\Software\Clients\StartMenuInternet\CCBaga-B','HKCU:\Software\Classes\CCBaga-B.URL','HKCU:\Software\Classes\CCBaga-B.HTML' -Recurse -Force; Remove-ItemProperty -LiteralPath 'HKCU:\Software\RegisteredApplications' -Name 'CCBaga-B'"]).status().unwrap();
        assert!(status.success());
    }
}
