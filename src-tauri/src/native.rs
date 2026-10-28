// [INPUT]: macOS Objective-C / Linux Rust / Windows Rust 的本地清理、授权与剪贴板接口。
// [OUTPUT]: 辅助功能权限、剪贴板复制、带创建时间的 macOS 进程快照/结束核验、钥匙串解锁、脱敏凭据发现及用户确认的原生身份凭据暂存/恢复，不将正文交给 UI。
// [POS]: 清理与电脑设置的平台桥接边界；不提供旧批量凭据删除包装，公共 JSON 解码供独立 system 桥复用。
// [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
use serde::Deserialize;
use std::ffi::{c_char, CStr, CString};

#[cfg(target_os = "linux")]
use crate::linux::*;
#[cfg(target_os = "windows")]
use crate::windows::*;
#[cfg(target_os = "macos")]
extern "C" {
    fn rf_permissions();
    fn rf_copy(text: *const c_char);
    fn rf_free(value: *mut c_char);
    fn rf_claude_processes() -> *mut c_char;
    fn rf_signal_process(pid: i32, created: u64, executable: *const c_char, signal: i32) -> *mut c_char;
    #[cfg(test)]
    fn rf_process_identity(pid: i32) -> *mut c_char;
    fn rf_credentials_list() -> *mut c_char;
    fn rf_keychain_unlock() -> *mut c_char;
    fn rf_identity_snapshot() -> *mut c_char;
    fn rf_identity_snapshot_for(scope: i32) -> *mut c_char;
    fn rf_identity_remove(json: *const c_char) -> *mut c_char;
    fn rf_identity_restore(json: *const c_char) -> *mut c_char;
}

#[derive(Clone, Deserialize)]
pub struct Credential {
    pub service: String,
    pub reference: String,
}
pub fn credentials() -> Result<Vec<Credential>, String> {
    #[derive(Deserialize)]
    struct List {
        items: Vec<Credential>,
    }
    Ok(decode::<List>(unsafe { rf_credentials_list() })?.items)
}
pub fn unlock_keychain() -> Result<String, String> {
    #[derive(Deserialize)]
    struct ResultMessage {
        message: String,
    }
    Ok(decode::<ResultMessage>(unsafe { rf_keychain_unlock() })?.message)
}
pub(crate) fn decode<T: serde::de::DeserializeOwned>(ptr: *mut c_char) -> Result<T, String> {
    if ptr.is_null() {
        return Err("系统操作失败。".into());
    }
    let text = unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() };
    unsafe {
        rf_free(ptr);
    }
    let data: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| "系统返回了无法识别的数据。")?;
    if let Some(error) = data["error"].as_str() {
        return Err(error.into());
    }
    serde_json::from_value(data).map_err(|_| "系统返回数据不完整。".into())
}

#[cfg(target_os = "macos")]
pub fn claude_processes() -> Result<Vec<crate::cleaner::ClaudeProcess>, String> {
    decode(unsafe { rf_claude_processes() })
}
#[cfg(target_os = "macos")]
pub fn signal_process(target: &crate::cleaner::ClaudeProcess, signal: i32) -> Result<(), String> {
    let executable = CString::new(target.executable.as_bytes()).map_err(|_| "进程路径无效。")?;
    let _: serde_json::Value = decode(unsafe { rf_signal_process(target.pid, target.created, executable.as_ptr(), signal) })?;
    Ok(())
}
#[cfg(all(test, target_os = "macos"))]
pub fn process_created(pid: i32) -> Result<u64, String> {
    let value: serde_json::Value = decode(unsafe { rf_process_identity(pid) })?;
    value["process"]["created"].as_u64().ok_or("测试进程已退出。".into())
}
pub fn permissions() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        crate::linux::open_settings("location").map(|_| ())
    }
    #[cfg(target_os = "windows")]
    {
        crate::windows::open_settings("location").map(|_| ())
    }
    #[cfg(target_os = "macos")]
    {
        unsafe {
            rf_permissions();
        }
        Ok(())
    }
}
pub fn copy(text: &str) -> Result<(), String> {
    let text = CString::new(text).map_err(|_| "文本包含无法复制的字符。")?;
    unsafe {
        #[cfg(target_os = "linux")]
        rf_copy(text.as_ptr())?;
        #[cfg(target_os = "windows")]
        rf_copy(text.as_ptr())?;
        #[cfg(target_os = "macos")]
        rf_copy(text.as_ptr());
    }
    Ok(())
}

// ----- 只供身份保险库使用；没有可直接调用的 WebView 凭据命令 -----
pub(crate) fn identity_snapshot() -> Result<Vec<serde_json::Value>, String> {
    #[cfg(target_os = "macos")]
    {
        #[derive(Deserialize)]
        struct Snapshot {
            items: Vec<serde_json::Value>,
        }
        return Ok(decode::<Snapshot>(unsafe { rf_identity_snapshot() })?.items);
    }
    #[cfg(not(target_os = "macos"))]
    Ok(Vec::new())
}
pub(crate) fn identity_snapshot_client(desktop: bool) -> Result<Vec<serde_json::Value>, String> {
    #[cfg(target_os = "macos")]
    {
        #[derive(Deserialize)] struct Snapshot { items: Vec<serde_json::Value> }
        Ok(decode::<Snapshot>(unsafe { rf_identity_snapshot_for(if desktop {2} else {1}) })?.items)
    }
    #[cfg(not(target_os = "macos"))]
    { let _ = desktop; Ok(Vec::new()) }
}
pub(crate) fn identity_keys(items: &[serde_json::Value], restore: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let bytes = serde_json::to_string(items).map_err(|_| "身份凭据无法处理。")?;
        let text = CString::new(bytes).map_err(|_| "身份凭据无法处理。")?;
        let _: serde_json::Value = decode(unsafe {
            if restore {
                rf_identity_restore(text.as_ptr())
            } else {
                rf_identity_remove(text.as_ptr())
            }
        })?;
    }
    let _ = (items, restore);
    Ok(())
}
