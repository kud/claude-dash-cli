use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub fn format_duration(ms: i64) -> String {
    let secs = (ms / 1000).max(0);
    let mins = secs / 60;
    let hours = mins / 60;
    if hours >= 1 {
        format!("{}h {}m", hours, mins % 60)
    } else if mins >= 1 {
        format!("{}m {}s", mins, secs % 60)
    } else {
        format!("{}s", secs)
    }
}

pub fn trunc_mid(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let tail = max / 3;
    let head = max - tail - 3;
    let chars: Vec<char> = s.chars().collect();
    format!(
        "{}...{}",
        chars[..head].iter().collect::<String>(),
        chars[chars.len() - tail..].iter().collect::<String>()
    )
}

pub fn abbreviate_home(path: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    if home.is_empty() {
        return path.to_string();
    }
    if path.starts_with(&home) {
        format!("~{}", &path[home.len()..])
    } else {
        path.to_string()
    }
}

pub fn progress_bar(pct: f64, width: usize) -> String {
    let clamped = pct.clamp(0.0, 100.0);
    let filled = ((clamped / 100.0) * width as f64).round() as usize;
    let empty = width - filled;
    format!("{}{}", "●".repeat(filled), "○".repeat(empty))
}

pub fn fmt_cost(n: f64) -> String {
    format!("${:.2}", n)
}

pub fn fmt_tokens(n: i64) -> String {
    if n >= 1_000_000 {
        format!("{:.2}M tok", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{}k tok", n / 1_000)
    } else {
        format!("{} tok", n)
    }
}

pub fn fmt_delta(now: f64, prev: f64) -> String {
    let diff = now - prev;
    let sign = if diff >= 0.0 { "+" } else { "" };
    let pct = if prev > 0.0 {
        format!("{:.1}", (diff / prev) * 100.0)
    } else {
        "—".to_string()
    };
    format!("{}{} ({}{}%)", sign, fmt_cost(diff), sign, pct)
}

pub fn thinking_spinner(tick: u64) -> &'static str {
    const FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
    FRAMES[(tick as usize) % FRAMES.len()]
}

pub fn tool_summary(tool_name: &str, tool_input: &serde_json::Value) -> String {
    let file = tool_input
        .get("file_path")
        .or_else(|| tool_input.get("path"))
        .or_else(|| tool_input.get("filePath"))
        .and_then(|v| v.as_str());
    let command = tool_input.get("command").and_then(|v| v.as_str());
    let pattern = tool_input.get("pattern").and_then(|v| v.as_str());
    let description = tool_input.get("description").and_then(|v| v.as_str());
    let query = tool_input.get("query").and_then(|v| v.as_str());
    let url = tool_input.get("url").and_then(|v| v.as_str());

    let hint = file
        .or(command)
        .or(pattern)
        .or(description)
        .or(query)
        .or(url);

    match hint {
        None => tool_name.to_string(),
        Some(h) => format!("{}: {}", tool_name, trunc_mid(&abbreviate_home(h), 48)),
    }
}
