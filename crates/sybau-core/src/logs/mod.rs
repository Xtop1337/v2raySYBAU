//! In-memory journal with daily log files, parsing of core output and export.

mod parse;

use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{Local, Utc};

use crate::model::{LogEntry, LogLevel, LogSource};
pub use parse::{parse_core_line, strip_ansi};

pub const DEFAULT_CAPACITY: usize = 5000;
const KEEP_DAYS: i64 = 7;

/// Ring buffer of recent entries plus append-only daily files in `dir`.
pub struct Journal {
    entries: Mutex<VecDeque<LogEntry>>,
    capacity: usize,
    dir: Option<PathBuf>,
}

impl Journal {
    pub fn new(dir: Option<PathBuf>, capacity: usize) -> Self {
        if let Some(dir) = &dir {
            let _ = fs::create_dir_all(dir);
            cleanup_old(dir);
        }
        Journal { entries: Mutex::new(VecDeque::with_capacity(capacity)), capacity, dir }
    }

    pub fn push(&self, entry: LogEntry) {
        if let Some(dir) = &self.dir {
            let file = dir.join(format!("sybau-{}.log", Local::now().format("%Y-%m-%d")));
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(file) {
                let _ = writeln!(f, "{}", format_line(&entry));
            }
        }
        let mut entries = self.entries.lock().unwrap();
        if entries.len() == self.capacity {
            entries.pop_front();
        }
        entries.push_back(entry);
    }

    pub fn app(&self, level: LogLevel, message: impl Into<String>) -> LogEntry {
        let entry = LogEntry { at: Utc::now(), level, source: LogSource::App, message: message.into(), request: None };
        self.push(entry.clone());
        entry
    }

    pub fn snapshot(&self, limit: usize) -> Vec<LogEntry> {
        let entries = self.entries.lock().unwrap();
        entries.iter().skip(entries.len().saturating_sub(limit)).cloned().collect()
    }

    pub fn clear(&self) {
        self.entries.lock().unwrap().clear();
    }

    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }
}

fn cleanup_old(dir: &Path) {
    let Ok(read) = fs::read_dir(dir) else { return };
    let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs((KEEP_DAYS * 86400) as u64);
    for entry in read.flatten() {
        if let Ok(modified) = entry.metadata().and_then(|m| m.modified()) {
            if modified < cutoff {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

fn level_name(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Debug => "DEBUG",
        LogLevel::Info => "INFO",
        LogLevel::Warning => "WARN",
        LogLevel::Error => "ERROR",
    }
}

fn source_name(source: LogSource) -> &'static str {
    match source {
        LogSource::App => "app",
        LogSource::Core => "core",
        LogSource::Access => "access",
    }
}

pub fn format_line(entry: &LogEntry) -> String {
    format!(
        "{} {:5} [{}] {}",
        entry.at.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S%.3f"),
        level_name(entry.level),
        source_name(entry.source),
        entry.message
    )
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// CSV with one row per entry; request columns are filled for access-log entries.
pub fn export_csv(entries: &[LogEntry]) -> String {
    let mut out = String::from("time,level,source,message,from,destination,network,inbound,outbound,accepted\n");
    for e in entries {
        let r = e.request.as_ref();
        let row = [
            e.at.to_rfc3339(),
            level_name(e.level).to_string(),
            source_name(e.source).to_string(),
            e.message.clone(),
            r.map(|r| r.source.clone()).unwrap_or_default(),
            r.map(|r| r.destination.clone()).unwrap_or_default(),
            r.map(|r| r.network.clone()).unwrap_or_default(),
            r.map(|r| r.inbound.clone()).unwrap_or_default(),
            r.map(|r| r.outbound.clone()).unwrap_or_default(),
            r.map(|r| r.accepted.to_string()).unwrap_or_default(),
        ];
        out.push_str(&row.iter().map(|f| csv_field(f)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    out
}

pub fn export_json(entries: &[LogEntry]) -> String {
    serde_json::to_string_pretty(entries).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_and_files() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::new(Some(dir.path().to_path_buf()), 3);
        for i in 0..5 {
            journal.app(LogLevel::Info, format!("m{i}"));
        }
        let snap = journal.snapshot(10);
        assert_eq!(snap.iter().map(|e| e.message.as_str()).collect::<Vec<_>>(), ["m2", "m3", "m4"]);
        let file = fs::read_dir(dir.path()).unwrap().next().unwrap().unwrap().path();
        assert_eq!(fs::read_to_string(file).unwrap().lines().count(), 5);
    }

    #[test]
    fn csv_escapes() {
        let journal = Journal::new(None, 10);
        let entry = journal.app(LogLevel::Warning, "a,\"b\"");
        assert!(export_csv(&[entry]).contains("\"a,\"\"b\"\"\""));
    }
}
