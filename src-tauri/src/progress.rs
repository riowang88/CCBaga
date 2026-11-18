// [INPUT]: 原生操作的真实阶段、完成数量与 Tauri 单次调用通道。
// [OUTPUT]: 有序脱敏进度和内存中的有界活动记录；阶段开始/结果另写入本机有界诊断日志（两个文件各至多 256 KiB，轮换覆盖），仅由用户主动复制导出。
// [POS]: 只观察事务，供 Claude 清理与浏览器操作共享，不改变执行；不读取文件正文、凭据或配置值。诊断日志不上传，主目录写成 ~、邮箱打码、单行截断；写入失败静默忽略。
// [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use tauri::ipc::Channel;
#[derive(Clone, Serialize)]
pub struct Progress {
    stage: String,
    status: String,
    message: String,
    current: usize,
    total: Option<usize>,
    elapsed_ms: u64,
}
pub(crate) struct Reporter {
    started: Instant,
    channel: Option<Channel<Progress>>,
    events: Mutex<Vec<Progress>>,
}
const LOG_LIMIT: u64 = 256 * 1024;
const LINE_LIMIT: usize = 600;
const EXPORT_LIMIT: usize = 200 * 1024;
struct Sink {
    path: PathBuf,
    home: Option<String>,
}
static SINK: OnceLock<Mutex<Sink>> = OnceLock::new();
fn timestamp(seconds: u64) -> String {
    // Civil date from days since 1970-01-01 (proleptic Gregorian, UTC).
    let (days, rest) = ((seconds / 86_400) as i64, seconds % 86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let (d, m) = (doy - (153 * mp + 2) / 5 + 1, if mp < 10 { mp + 3 } else { mp - 9 });
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rest / 3_600, rest % 3_600 / 60, rest % 60)
}
fn scrub(message: &str, home: Option<&str>) -> String {
    let text = match home.filter(|h| h.len() > 1) {
        Some(home) => message.replace(home, "~"),
        None => message.to_owned(),
    };
    let chars: Vec<char> = text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let part = |c: char| c.is_ascii_alphanumeric() || "._%+-".contains(c);
    let mut out = String::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '@' {
            let left = out.chars().rev().take_while(|c| part(*c)).count();
            let right = chars[index + 1..].iter().take_while(|c| part(**c)).count();
            if left > 0 && right > 0 && chars[index + 1..index + 1 + right].contains(&'.') {
                out.truncate(out.len() - left);
                out.push_str("<email>");
                index += 1 + right;
                continue;
            }
        }
        out.push(chars[index]);
        index += 1;
    }
    if out.chars().count() > LINE_LIMIT {
        out = out.chars().take(LINE_LIMIT).collect::<String>() + "…";
    }
    out
}
fn older(path: &Path) -> PathBuf {
    path.with_extension("1.log")
}
fn append(sink: &Sink, stage: &str, status: &str, message: &str, seconds: u64) {
    let line = format!("{} {} {} {}\n", timestamp(seconds), stage, status, scrub(message, sink.home.as_deref()));
    // Two files at most: the full one replaces the previous older file.
    if std::fs::metadata(&sink.path).is_ok_and(|m| m.len() + line.len() as u64 > LOG_LIMIT) {
        let _ = std::fs::remove_file(older(&sink.path));
        if std::fs::rename(&sink.path, older(&sink.path)).is_err() {
            let _ = std::fs::remove_file(&sink.path);
        }
    }
    let mut options = std::fs::OpenOptions::new();
    options.append(true).create(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    if let Ok(mut file) = options.open(&sink.path) {
        let _ = std::io::Write::write_all(&mut file, line.as_bytes());
    }
}
fn collect(sink: &Sink) -> String {
    let mut text = String::new();
    for path in [older(&sink.path), sink.path.clone()] {
        if let Ok(bytes) = std::fs::read(path) {
            text.push_str(&String::from_utf8_lossy(&bytes));
        }
    }
    if text.len() > EXPORT_LIMIT {
        let mut start = text.len() - EXPORT_LIMIT;
        while !text.is_char_boundary(start) { start += 1; }
        start += text[start..].find('\n').map_or(0, |n| n + 1);
        text.drain(..start);
    }
    format!("CCBaga {} {} {}\n{}", env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH, text)
}
fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}
/// Called once at startup; without it nothing is written (tests, helper processes).
pub(crate) fn install(directory: &Path, home: Option<&Path>) {
    if std::fs::create_dir_all(directory).is_err() { return; }
    let _ = SINK.set(Mutex::new(Sink {
        path: directory.join("diagnostics.log"),
        home: home.and_then(|h| h.to_str()).map(str::to_owned),
    }));
    note("app", "start", env!("CARGO_PKG_VERSION"));
}
pub(crate) fn note(stage: &str, status: &str, message: &str) {
    if let Some(sink) = SINK.get() {
        if let Ok(sink) = sink.lock() { append(&sink, stage, status, message, now()); }
    }
}
pub(crate) fn export() -> String {
    SINK.get().and_then(|sink| sink.lock().ok().map(|sink| collect(&sink))).unwrap_or_default()
}
impl Reporter {
    pub(crate) fn channel(channel: Option<Channel<Progress>>) -> Self {
        Self {
            started: Instant::now(),
            channel,
            events: Mutex::new(Vec::new()),
        }
    }
    pub(crate) fn log(
        &self,
        stage: &str,
        status: &str,
        message: &str,
        current: usize,
        total: Option<usize>,
    ) {
        let event = Progress {
            stage: stage.into(),
            status: status.into(),
            message: message.into(),
            current,
            total,
            elapsed_ms: self.started.elapsed().as_millis() as u64,
        };
        // Per-item progress stays in memory; the file keeps stage starts and outcomes only.
        if status != "running" || current == 0 {
            let count = total.map_or(String::new(), |total| format!(" [{current}/{total}]"));
            note(stage, status, &format!("{message}{count}"));
        }
        let mut events = self.events.lock().unwrap();
        if events.len() == 2000 {
            events.remove(0);
        }
        events.push(event.clone());
        if let Some(channel) = &self.channel {
            let _ = channel.send(event);
        }
    }
    pub(crate) fn fail(&self, message: &str) {
        let stage = self
            .events
            .lock()
            .unwrap()
            .last()
            .map(|event| event.stage.clone())
            .unwrap_or_else(|| "validate".into());
        self.log(&stage, "error", message, 0, None);
    }
    #[cfg(test)]
    pub(crate) fn events(&self) -> Vec<Progress> {
        self.events.lock().unwrap().clone()
    }
}
#[cfg(test)]
impl Progress {
    pub(crate) fn stage(&self) -> &str {
        &self.stage
    }
    pub(crate) fn status(&self) -> &str {
        &self.status
    }
    pub(crate) fn current(&self) -> usize {
        self.current
    }
    pub(crate) fn message(&self) -> &str {
        &self.message
    }
}

#[cfg(test)]
mod diagnostics_tests {
    use super::*;
    #[test]
    fn diagnostics_stay_bounded_and_scrubbed() {
        let folder = std::env::temp_dir().join(format!("baga-diagnostics-{}-{}", std::process::id(), now()));
        std::fs::create_dir_all(&folder).unwrap();
        let sink = Sink { path: folder.join("diagnostics.log"), home: Some("/Users/someone".into()) };
        append(&sink, "restore", "error", "路径 /Users/someone/.claude/x 属于 a.b+c@example.co.uk；\n下一行 @ 不是邮箱", 86_400 * 365 + 3_661);
        let text = std::fs::read_to_string(&sink.path).unwrap();
        assert_eq!(text, "1971-01-01T01:01:01Z restore error 路径 ~/.claude/x 属于 <email>； 下一行 @ 不是邮箱\n");
        let long = "长".repeat(5_000);
        for _ in 0..2_000 { append(&sink, "stage", "ok", &long, 0); }
        let (new, old) = (std::fs::metadata(&sink.path).unwrap().len(), std::fs::metadata(older(&sink.path)).unwrap().len());
        assert!(new <= LOG_LIMIT && old <= LOG_LIMIT, "{new} {old}");
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 2);
        let exported = collect(&sink);
        assert!(exported.len() <= EXPORT_LIMIT + 200 && exported.starts_with("CCBaga "));
        assert!(exported.lines().skip(1).all(|line| line.starts_with("1970-01-01T00:00:00Z stage ok ")));
        assert_eq!(timestamp(1_794_960_000), "2026-11-18T00:00:00Z");
        std::fs::remove_dir_all(folder).unwrap();
    }
}
