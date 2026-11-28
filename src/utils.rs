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
    FRAMES[(tick as usize / 3) % FRAMES.len()]
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn format_duration_picks_largest_unit() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(-5_000), "0s");
        assert_eq!(format_duration(45_000), "45s");
        assert_eq!(format_duration(125_000), "2m 5s");
        assert_eq!(format_duration(3_723_000), "1h 2m");
    }

    #[test]
    fn trunc_mid_keeps_short_strings_and_elides_long_ones() {
        assert_eq!(trunc_mid("short", 10), "short");
        let out = trunc_mid("abcdefghijklmnopqrstuvwxyz", 12);
        assert_eq!(out.chars().count(), 12);
        assert_eq!(out, "abcde...wxyz");
    }

    #[test]
    fn trunc_mid_is_char_safe() {
        let out = trunc_mid("ééééééééééééééééééé", 9);
        assert_eq!(out.chars().count(), 9);
        assert!(out.contains("..."));
    }

    #[test]
    fn progress_bar_clamps_and_fills() {
        assert_eq!(progress_bar(0.0, 4), "○○○○");
        assert_eq!(progress_bar(50.0, 4), "●●○○");
        assert_eq!(progress_bar(150.0, 4), "●●●●");
        assert_eq!(progress_bar(-10.0, 4), "○○○○");
    }

    #[test]
    fn cost_and_token_formatting() {
        assert_eq!(fmt_cost(1.234), "$1.23");
        assert_eq!(fmt_tokens(999), "999 tok");
        assert_eq!(fmt_tokens(12_345), "12k tok");
        assert_eq!(fmt_tokens(2_500_000), "2.50M tok");
    }

    #[test]
    fn delta_formatting() {
        assert_eq!(fmt_delta(15.0, 10.0), "+$5.00 (+50.0%)");
        assert_eq!(fmt_delta(5.0, 10.0), "$-5.00 (-50.0%)");
        assert_eq!(fmt_delta(3.0, 0.0), "+$3.00 (+—%)");
    }

    #[test]
    fn spinner_cycles_through_frames() {
        assert_eq!(thinking_spinner(0), thinking_spinner(2));
        assert_ne!(thinking_spinner(0), thinking_spinner(3));
        assert_eq!(thinking_spinner(0), thinking_spinner(24));
    }

    #[test]
    fn tool_summary_prefers_file_path_then_command() {
        assert_eq!(tool_summary("Read", &json!({})), "Read");
        assert_eq!(
            tool_summary("Bash", &json!({"command": "ls -la"})),
            "Bash: ls -la"
        );
        assert_eq!(
            tool_summary("Edit", &json!({"file_path": "/x/y.rs", "command": "ignored"})),
            "Edit: /x/y.rs"
        );
    }
}
