// [INPUT]: libexec 隔离的 Linux Chromium、XDG 桌面入口、固定 coreutils nice 原生执行器与共用资料/随机令牌会话；测试使用真实 GIO。
// [OUTPUT]: 本地引擎启动与主动设置默认浏览器；零优先级调整绕过 GLib 在展开 %% 前的程序路径预检，不解释 shell 或环境赋值。
// [POS]: browser 编排的 Linux 平台实现；只接收 URL/文件入口，不执行用户提供的 shell。
// [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
fn register(_: &Path) -> Result<(), String> {
    Ok(())
}
fn binary(engine: &Path) -> PathBuf {
    engine.join("chrome")
}
fn packaged_engine(resources: &Path) -> PathBuf {
    let legacy = resources.join("browser-runtime/LinuxBrowser");
    if binary(&legacy).is_file() {
        return legacy;
    }
    resources.join("../../libexec/CCBaga-B")
}
fn engine(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let resources = app.path().resource_dir().map_err(|_| "资源目录不可用。")?;
    let packaged = packaged_engine(&resources);
    if binary(&packaged).is_file() || !cfg!(debug_assertions) {
        return Ok(packaged);
    }
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../browser-runtime/LinuxBrowser"))
}
#[test]
fn packaged_browser_paths_preserve_both_layouts() {
    let root = tests::sample();
    let resources = root.join("usr/lib/CCBaga-A");
    fs::create_dir_all(&resources).unwrap();
    let isolated = root.join("usr/libexec/CCBaga-B");
    fs::create_dir_all(&isolated).unwrap();
    fs::write(binary(&isolated), "isolated engine").unwrap();
    assert_eq!(
        packaged_engine(&resources).canonicalize().unwrap(),
        isolated
    );
    let legacy = resources.join("browser-runtime/LinuxBrowser");
    fs::create_dir_all(&legacy).unwrap();
    fs::write(binary(&legacy), "source/Arch engine").unwrap();
    assert_eq!(packaged_engine(&resources), legacy);
    fs::remove_dir_all(root).unwrap();
}
fn prepare_engine(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let engine = engine(app)?;
    if !binary(&engine).is_file() {
        return Err(
            "Linux browser engine missing. Run python3 scripts/fetch-browser.py before building."
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
        if let Err(error) = start_linux_companion(&app, &urls) {
            eprintln!("CCBaga-B: {error}");
        }
    });
}
fn start_linux_companion(app: &tauri::AppHandle, urls: &[String]) -> Result<(), String> {
    let state = app.state::<Browser>();
    let _guard = state.gate.lock().unwrap();
    let engine = prepare_engine(app)?;
    let root = root(app)?;
    let running = workspaces::running_profiles(&root)?;
    let target = workspaces::prepare_active_profile(&root, &running)?;
    let settings = target.scope.clone();
    let profile = target.profile.clone();
    let slot = target.slot.clone();
    profile::check_path(&root, &profile)?;
    let config = config::load(&settings)?;
    let machine = config::machine()?;
    config.validate(&machine)?;
    let running = profile::running(&profile)?;
    if !running {
        profile::preferences(&profile, &config, &machine)?;
    }
    write_launch_args(&root, &config, &machine, &profile, &version(&engine))?;
    workspaces::unblock(&root)?;
    let reuse_running = !urls.is_empty() && running && state.session.lock().unwrap().is_some();
    let mut cmd_args = if reuse_running {
        let mut a = config::arguments(&config, &machine, &profile, "", &version(&engine));
        a.pop();
        a
    } else {
        let page = open_session(app, &config, &machine, version(&engine))?;
        config::arguments(&config, &machine, &profile, &page, &version(&engine))
    };
    cmd_args.extend_from_slice(urls);
    let mut child = Command::new(binary(&engine))
        .args(cmd_args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| {
            "Could not launch Linux Chromium. Check engine permissions and runtime libraries."
                .to_string()
        })?;
    let started = launch::confirm_started(&root, &slot, &profile, || {
        Ok(child.try_wait().map_err(|_| "无法读取浏览器状态。".to_string())?.is_some())
    });
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    started
}
const LINUX_DESKTOP_ID: &str = "ccbaga-browser.desktop";
fn default_read() -> Result<DefaultBrowser, String> {
    let query = |scheme| -> Result<String, String> {
        let out = Command::new("xdg-mime")
            .args(["query", "default", scheme])
            .output()
            .map_err(|_| "Install xdg-utils to manage the default browser.")?;
        if !out.status.success() {
            return Err("Cannot query the desktop's default browser.".into());
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().into())
    };
    let http = query("x-scheme-handler/http")?;
    let https = query("x-scheme-handler/https")?;
    Ok(DefaultBrowser {
        is_default: http == LINUX_DESKTOP_ID && https == LINUX_DESKTOP_ID,
        http,
        https,
        installed: true,
        app: std::env::current_exe()
            .map_err(|_| "Cannot locate CCBaga-A.")?
            .display()
            .to_string(),
    })
}
// Desktop Entry 字符串先解码，再应用 Exec 双引号规则；双引号前因此写两个反斜杠。
fn desktop_exec_escape(path: &str) -> Result<String, String> {
    if path.contains(['\n', '\r']) {
        return Err("Invalid application path.".into());
    }
    Ok(path
        .replace('\\', "\\\\\\\\")
        .replace('"', "\\\\\"")
        .replace('`', "\\\\`")
        .replace('$', "\\\\$")
        .replace('%', "%%"))
}
fn desktop_exec_command(path: &str) -> Result<String, String> {
    // GLib 先查首参数再展开 %%；固定原生执行器兼容含 %/= 的路径，-n 0 保持继承优先级。
    Ok(format!("/usr/bin/nice -n 0 -- \"{}\"", desktop_exec_escape(path)?))
}
#[test]
fn desktop_exec_escapes_quotes_with_one_exec_layer_escape() {
    assert_eq!(desktop_exec_escape(r#"/opt/say "hi"/app"#).unwrap(), r#"/opt/say \\"hi\\"/app"#);
    assert_eq!(desktop_exec_escape(r"/opt/a\b/app").unwrap(), r"/opt/a\\\\b/app");
    assert_eq!(desktop_exec_escape("/opt/`x`/$HOME/100%").unwrap(), r"/opt/\\`x\\`/\\$HOME/100%%");
    assert!(desktop_exec_escape("/opt/a\nb").is_err());
    assert!(desktop_exec_escape("/opt/a\rb").is_err());
    for original in [r#"/opt/say "hi"/app"#, r"/opt/back\slash/app", "/opt/`tick`/$HOME/中文 100% %U/app"] {
        let encoded = desktop_exec_escape(original).unwrap().replace("%%", "%");
        let string_decoded = encoded.replace("\\\\", "\0");
        assert!(!string_decoded.contains('\\'));
        let exec_decoded = string_decoded.replace('\0', "\\");
        let mut characters = exec_decoded.chars();
        let mut decoded = String::new();
        while let Some(character) = characters.next() {
            if character == '\\' {
                decoded.push(characters.next().unwrap());
            } else {
                assert!(!matches!(character, '"' | '`' | '$'));
                decoded.push(character);
            }
        }
        assert_eq!(decoded, original);
    }
}
#[test]
fn desktop_exec_round_trips_through_real_gio() {
    use std::os::unix::fs::PermissionsExt;
    let root = tests::sample();
    for (index, name) in [
        "say \"hi\"\\`tick`$HOME $(touch should-not-exist) 100% %U 中文",
        "CCBAGA_ASSIGNMENT=value % 中文",
        "--leading-option %U",
    ]
    .iter()
    .enumerate()
    {
        let executable = root.join(name).join("app");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, "#!/bin/sh\n/usr/bin/nice > \"$CCBAGA_TEST_NICE\"\nprintf '%s\\0' \"$0\" \"$@\" > \"$CCBAGA_TEST_ARGV\"\n").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let desktop = root.join("fixture.desktop");
        let command = desktop_exec_command(executable.to_str().unwrap()).unwrap();
        fs::write(&desktop, format!("[Desktop Entry]\nType=Application\nName=CCBaga fixture\nExec={command} --browser %U\nTerminal=false\n")).unwrap();
        let urls = [
            "https://example.invalid/first?q=two%20words&mark=%25",
            "https://example.invalid/second?q=%22quoted%22",
        ];
        let receipt = root.join(format!("argv-{index}"));
        let niceness = root.join(format!("nice-{index}"));
        // 只运行临时无网络脚本；清空继承环境并隔离 HOME/XDG，不触碰实际默认程序。
        let output = Command::new("/usr/bin/nice")
            .args(["-n", "7", "--", "/usr/bin/gio", "launch"])
            .arg(&desktop)
            .args(urls)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &root)
            .env("XDG_DATA_HOME", root.join("data"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_CACHE_HOME", root.join("cache"))
            .env("GIO_USE_VFS", "local")
            .env("GSETTINGS_BACKEND", "memory")
            .env("CCBAGA_TEST_ARGV", &receipt)
            .env("CCBAGA_TEST_NICE", &niceness)
            .current_dir(&root)
            .stdin(Stdio::null())
            .output()
            .expect("Linux regression requires the GIO tool supplied with GLib");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut arguments = vec![executable.to_str().unwrap(), "--browser"];
        arguments.extend(urls);
        let expected = arguments
            .iter()
            .flat_map(|argument| argument.as_bytes().iter().copied().chain([0]))
            .collect::<Vec<_>>();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if fs::read(&receipt).ok().as_ref() == Some(&expected) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let actual = fs::read(&receipt).expect("GIO did not launch the temporary fixture");
        assert_eq!(actual, expected);
        assert_eq!(fs::read_to_string(niceness).unwrap().trim(), "7");
        assert!(!root.join("should-not-exist").exists());
    }
    fs::remove_dir_all(root).unwrap();
}
#[tauri::command]
pub async fn browser_set_default(app: tauri::AppHandle) -> Result<DefaultBrowser, String> {
    run(move || {
        prepare_engine(&app)?;
        let data = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/share"))).ok_or("Cannot locate XDG data directory.")?;
        let desktop = data.join("applications").join(LINUX_DESKTOP_ID);
        profile::check_path(&data, &desktop)?;
        fs::create_dir_all(desktop.parent().unwrap()).map_err(|_| "Cannot create applications directory.")?;
        // AppImage extraction paths are temporary: register the persistent AppImage when available.
        let executable = std::env::var_os("APPIMAGE").map(PathBuf::from).unwrap_or(std::env::current_exe().map_err(|_| "Cannot locate CCBaga-A.")?);
        let command = desktop_exec_command(&executable.to_string_lossy())?;
        fs::write(&desktop, format!("[Desktop Entry]\nType=Application\nName=CCBaga-B\nExec={command} --browser %U\nIcon=ccbaga-browser\nTerminal=false\nCategories=Network;WebBrowser;\nMimeType=x-scheme-handler/http;x-scheme-handler/https;text/html;\n")).map_err(|_| "Cannot register CCBaga-B desktop entry.")?;
        for scheme in ["x-scheme-handler/http", "x-scheme-handler/https"] {
            let status = Command::new("xdg-mime").args(["default", LINUX_DESKTOP_ID, scheme]).status().map_err(|_| "Install xdg-utils to manage default applications.")?;
            if !status.success() { return Err("Default browser update failed; check desktop settings.".into()); }
        }
        let state = default_read()?;
        if !state.is_default { return Err("Desktop did not confirm CCBaga-B as the default browser.".into()); }
        Ok(state)
    }).await
}
