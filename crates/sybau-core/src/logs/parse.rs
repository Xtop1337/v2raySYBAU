//! Parsing of Xray / sing-box console output into journal entries.

use chrono::Utc;

use crate::model::{LogEntry, LogLevel, LogSource, RequestRecord};

pub fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Parses one line of core output. Xray access lines become `LogSource::Access` with a request record.
pub fn parse_core_line(raw: &str) -> Option<LogEntry> {
    let line = strip_ansi(raw);
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    if let Some(request) = parse_access(line) {
        let message = format!(
            "{} {} {} [{} -> {}]",
            request.source,
            if request.accepted { "→" } else { "✕" },
            request.destination,
            request.inbound,
            request.outbound
        );
        return Some(LogEntry { at: Utc::now(), level: LogLevel::Info, source: LogSource::Access, message, request: Some(request) });
    }
    Some(LogEntry {
        at: Utc::now(),
        level: detect_level(line),
        source: LogSource::Core,
        message: strip_timestamp(line).to_string(),
        request: None,
    })
}

/// `2026/10/28 13:48:18.044250 from 127.0.0.1:54321 accepted tcp:www.google.com:443 [socks -> proxy]`
fn parse_access(line: &str) -> Option<RequestRecord> {
    let from = line.find("from ")?;
    let rest = &line[from + 5..];
    let (index, accepted) = match rest.find(" accepted ") {
        Some(i) => (i, true),
        None => (rest.find(" rejected ")?, false),
    };
    let (source, rest) = (&rest[..index], &rest[index + 10..]);
    let mut parts = rest.splitn(2, ' ');
    let target = parts.next()?.trim();
    let tail = parts.next().unwrap_or("");
    let (network, destination) = match target.split_once(':') {
        Some((net, dest)) if net == "tcp" || net == "udp" => (net.to_string(), dest.to_string()),
        _ => (String::new(), target.to_string()),
    };
    let (inbound, outbound) = tail
        .find('[')
        .and_then(|start| tail[start + 1..].find(']').map(|end| &tail[start + 1..start + 1 + end]))
        .map(|route| {
            let split = route.split_once("->").or_else(|| route.split_once(">>"));
            match split {
                Some((i, o)) => (i.trim().to_string(), o.trim().to_string()),
                None => (route.trim().to_string(), String::new()),
            }
        })
        .unwrap_or_default();
    Some(RequestRecord {
        source: source.trim().trim_start_matches("tcp:").trim_start_matches("udp:").to_string(),
        destination,
        network,
        inbound,
        outbound,
        accepted,
    })
}

fn detect_level(line: &str) -> LogLevel {
    let upper = line.to_ascii_uppercase();
    if upper.contains("[ERROR]") || upper.contains(" ERROR ") || upper.contains("FATAL") || upper.contains("FAILED TO START") {
        LogLevel::Error
    } else if upper.contains("[WARNING]") || upper.contains(" WARN ") {
        LogLevel::Warning
    } else if upper.contains("[DEBUG]") || upper.contains(" DEBUG ") || upper.contains(" TRACE ") {
        LogLevel::Debug
    } else {
        LogLevel::Info
    }
}

/// Drops the leading `2026/10/28 13:48:18.044250 ` Xray timestamp (the journal has its own).
fn strip_timestamp(line: &str) -> &str {
    let bytes = line.as_bytes();
    if bytes.len() > 20 && bytes[4] == b'/' && bytes[7] == b'/' && bytes[10] == b' ' {
        if let Some(space) = line[11..].find(' ') {
            return &line[11 + space + 1..];
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_access_lines() {
        let e =
            parse_core_line("2026/10/28 13:48:18.044250 from 127.0.0.1:54321 accepted tcp:www.google.com:443 [socks -> proxy]").unwrap();
        assert_eq!(e.source, LogSource::Access);
        let r = e.request.unwrap();
        assert_eq!(r.source, "127.0.0.1:54321");
        assert_eq!(r.destination, "www.google.com:443");
        assert_eq!(r.network, "tcp");
        assert_eq!((r.inbound.as_str(), r.outbound.as_str()), ("socks", "proxy"));
        assert!(r.accepted);

        let e = parse_core_line("2026/10/28 13:48:18.0 from tcp:127.0.0.1:1 accepted udp:8.8.8.8:53 [http >> direct] email: x").unwrap();
        let r = e.request.unwrap();
        assert_eq!(r.outbound, "direct");
        assert_eq!(r.source, "127.0.0.1:1");
    }

    #[test]
    fn parses_levels_and_ansi() {
        let e = parse_core_line("2026/10/28 13:48:18.044250 [Warning] core: something").unwrap();
        assert_eq!(e.level, LogLevel::Warning);
        assert_eq!(e.message, "[Warning] core: something");
        let e = parse_core_line("\u{1b}[31mFATAL\u{1b}[0m[0000] decode config").unwrap();
        assert_eq!(e.level, LogLevel::Error);
        assert_eq!(e.message, "FATAL[0000] decode config");
        assert!(parse_core_line("   ").is_none());
    }
}
