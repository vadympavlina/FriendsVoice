//! Crash log and diagnostic report.
//!
//! The app has no console window, so a panic would otherwise vanish without a trace.
//! Panics are appended to `crash.log` in the settings folder, which is what a bug
//! report needs. The About page can open that folder and copy a short report.
use std::{fs, io::Write, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

const MAX_LOG_BYTES: u64 = 256 * 1024;

/// `%APPDATA%\FriendsVoice` on Windows.
pub fn log_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|dir| PathBuf::from(dir).join("FriendsVoice"))
}

/// One readable block for the log file.
pub fn format_report(version: &str, thread: &str, location: &str, message: &str, backtrace: &str, unix_seconds: u64) -> String {
    format!(
        "---- Friends Voice {version} crashed ----\nunix time: {unix_seconds}\nthread: {thread}\nat: {location}\nmessage: {message}\nbacktrace:\n{backtrace}\n\n"
    )
}

fn append(path: &PathBuf, text: &str) {
    if let Some(parent) = path.parent() { let _ = fs::create_dir_all(parent); }
    // Keep the file small: an old log is not worth megabytes on the user's disk.
    if fs::metadata(path).is_ok_and(|m| m.len() > MAX_LOG_BYTES) { let _ = fs::remove_file(path); }
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(text.as_bytes());
    }
}

/// Installs the panic hook. Call once, first thing in `main`.
pub fn install() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let message = info.payload().downcast_ref::<&str>().map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(no message)".into());
        let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        if let Some(dir) = log_dir() {
            append(&dir.join("crash.log"), &format_report(env!("CARGO_PKG_VERSION"), &thread, &location, &message, &backtrace, seconds));
        }
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_contains_everything_needed_for_a_bug_report() {
        let text = format_report("1.2.0", "main", "src/x.rs:10", "boom", "frame 1", 1_700_000_000);
        for part in ["Friends Voice 1.2.0", "1700000000", "thread: main", "src/x.rs:10", "message: boom", "frame 1"] {
            assert!(text.contains(part), "missing {part}");
        }
    }

    #[test]
    fn log_file_is_created_appended_and_kept_small() {
        let dir = std::env::temp_dir().join(format!("fv-log-test-{}", std::process::id()));
        let path = dir.join("nested").join("crash.log");
        append(&path, "first\n");
        append(&path, "second\n");
        assert_eq!(fs::read_to_string(&path).unwrap(), "first\nsecond\n");
        append(&path, &"x".repeat(MAX_LOG_BYTES as usize + 10));
        append(&path, "after\n"); // the oversized log is dropped before this write
        assert_eq!(fs::read_to_string(&path).unwrap(), "after\n");
        let _ = fs::remove_dir_all(dir);
    }
}
