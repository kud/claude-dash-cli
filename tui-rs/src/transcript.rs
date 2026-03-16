use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::{mpsc, watch};

use crate::app::AppEvent;
use crate::types::{MessageRole, TranscriptMessage};

pub async fn run(
    tx: mpsc::UnboundedSender<AppEvent>,
    mut path_rx: watch::Receiver<Option<String>>,
) {
    let mut current_path: Option<String> = None;
    let mut file_offset: u64 = 0;
    let mut messages: Vec<TranscriptMessage> = Vec::new();
    let mut poll = tokio::time::interval(std::time::Duration::from_millis(500));
    poll.tick().await;

    loop {
        tokio::select! {
            _ = path_rx.changed() => {
                current_path = path_rx.borrow().clone();
                file_offset = 0;
                messages.clear();
                match read_new(&current_path, &mut file_offset).await {
                    Some(new_msgs) => messages.extend(new_msgs),
                    None => {}
                }
                let _ = tx.send(AppEvent::TranscriptLoaded(messages.clone()));
            }
            _ = poll.tick() => {
                if current_path.is_some() {
                    if let Some(new_msgs) = read_new(&current_path, &mut file_offset).await {
                        if !new_msgs.is_empty() {
                            messages.extend(new_msgs);
                            let _ = tx.send(AppEvent::TranscriptLoaded(messages.clone()));
                        }
                    }
                }
            }
        }
    }
}

async fn read_new(path: &Option<String>, offset: &mut u64) -> Option<Vec<TranscriptMessage>> {
    let path = path.as_deref()?;

    let size = tokio::fs::metadata(path).await.ok()?.len();

    if size < *offset {
        *offset = 0; // file was replaced
    }
    if size == *offset {
        return Some(vec![]); // nothing new
    }

    let mut file = tokio::fs::File::open(path).await.ok()?;
    if *offset > 0 {
        file.seek(std::io::SeekFrom::Start(*offset)).await.ok()?;
    }

    let mut buf = Vec::new();
    file.read_to_end(&mut buf).await.ok()?;

    // Only parse up to the last complete newline so we never split a partial line.
    let end = buf.iter().rposition(|&b| b == b'\n').map(|i| i + 1).unwrap_or(0);
    *offset += end as u64;

    let text = std::str::from_utf8(&buf[..end]).ok()?;
    Some(parse(text))
}

fn parse(raw: &str) -> Vec<TranscriptMessage> {
    let mut messages = Vec::new();
    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let role = match entry.get("type").and_then(|v| v.as_str()) {
            Some("user") => MessageRole::User,
            Some("assistant") => MessageRole::Assistant,
            _ => continue,
        };
        let Some(msg) = entry.get("message") else {
            continue;
        };
        let text = parse_content(msg.get("content"));
        if text.is_empty() {
            continue;
        }
        let timestamp = entry
            .get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(parse_iso_ms)
            .unwrap_or_else(crate::utils::now_ms);
        let id = entry
            .get("uuid")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| timestamp.to_string());
        messages.push(TranscriptMessage { id, role, text, timestamp });
    }
    messages
}

fn parse_content(content: Option<&serde_json::Value>) -> String {
    match content {
        Some(serde_json::Value::String(s)) => s.trim().to_string(),
        Some(serde_json::Value::Array(blocks)) => blocks
            .iter()
            .filter_map(|b| {
                if b.get("type")?.as_str()? == "text" {
                    b.get("text")?.as_str().map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string(),
        _ => String::new(),
    }
}

fn parse_iso_ms(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() < 19 {
        return None;
    }
    let year: i64 = s[..4].parse().ok()?;
    let month: i64 = s[5..7].parse().ok()?;
    let day: i64 = s[8..10].parse().ok()?;
    let hour: i64 = s[11..13].parse().ok()?;
    let min: i64 = s[14..16].parse().ok()?;
    let sec: i64 = s[17..19].parse().ok()?;
    let days = days_since_epoch(year, month, day);
    Some((days * 86400 + hour * 3600 + min * 60 + sec) * 1000)
}

fn days_since_epoch(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let m = if month <= 2 { month + 12 } else { month };
    let a = y / 100;
    let b = 2 - a + a / 4;
    let jd = (365.25 * (y + 4716) as f64) as i64
        + (30.6001 * (m + 1) as f64) as i64
        + day
        + b
        - 1524;
    jd - 2440588
}
