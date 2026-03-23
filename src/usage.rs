use std::time::Duration;
use tokio::sync::mpsc;

use crate::app::AppEvent;
use crate::types::{
    DailyUsage, ModelBreakdown, MonthlyUsage, RateLimitEntry, RateLimits, TotalUsage,
};

pub async fn run(tx: mpsc::UnboundedSender<AppEvent>, mut refresh_rx: mpsc::UnboundedReceiver<()>) {
    let _ = tx.send(AppEvent::UsageLoading);
    fetch_usage(&tx).await;
    fetch_limits(&tx).await;

    let mut usage_tick = tokio::time::interval(Duration::from_secs(60));
    let mut limits_tick = tokio::time::interval(Duration::from_secs(30));
    usage_tick.tick().await;
    limits_tick.tick().await;

    loop {
        tokio::select! {
            _ = usage_tick.tick() => {
                fetch_usage(&tx).await;
            }
            _ = limits_tick.tick() => {
                fetch_limits(&tx).await;
            }
            Some(_) = refresh_rx.recv() => {
                let _ = tx.send(AppEvent::UsageLoading);
                fetch_usage(&tx).await;
                fetch_limits(&tx).await;
            }
        }
    }
}

async fn ccusage(args: &[&str]) -> Option<serde_json::Value> {
    let output = tokio::process::Command::new("npx")
        .args(["--yes", "ccusage@latest"])
        .args(args)
        .output()
        .await
        .ok()?;
    let s = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(s.trim()).ok()
}

fn yesterday_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let day_secs = secs - 86400;
    let days = day_secs / 86400;
    let (y, m, d) = days_to_ymd(days as i64 + 719468);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

fn days_to_ymd(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

async fn fetch_usage(tx: &mpsc::UnboundedSender<AppEvent>) {
    let (raw_all, raw_monthly) = tokio::join!(
        ccusage(&["--json"]),
        ccusage(&["monthly", "--json"]),
    );

    let daily_arr: Vec<DailyUsage> = raw_all
        .as_ref()
        .and_then(|v| v.get("daily"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(parse_daily_entry).collect())
        .unwrap_or_default();

    let today = daily_arr.last().cloned();
    let yesterday_iso = yesterday_iso();
    let yesterday = daily_arr
        .iter()
        .rev()
        .find(|d| d.date == yesterday_iso)
        .cloned();

    let total = raw_all.as_ref().and_then(|v| normalize_total(v));
    let monthly = raw_monthly.as_ref().and_then(|v| normalize_monthly(v));

    let _ = tx.send(AppEvent::UsageLoaded {
        today,
        yesterday,
        monthly,
        total,
        daily_history: daily_arr,
    });
}

async fn fetch_limits(tx: &mpsc::UnboundedSender<AppEvent>) {
    if let Some(limits) = get_rate_limits().await {
        let _ = tx.send(AppEvent::RateLimitsLoaded(limits));
    }
}

async fn get_claude_token() -> Option<String> {
    if let Ok(t) = std::env::var("ANTHROPIC_API_KEY") {
        if !t.is_empty() {
            return Some(t);
        }
    }
    if let Ok(t) = std::env::var("CLAUDE_API_KEY") {
        if !t.is_empty() {
            return Some(t);
        }
    }
    let out = tokio::process::Command::new("security")
        .args(["find-generic-password", "-s", "Claude", "-w"])
        .output()
        .await
        .ok()?;
    if out.status.success() {
        let t = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    None
}

async fn get_rate_limits() -> Option<RateLimits> {
    let token = get_claude_token().await?;
    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.anthropic.com/api/oauth/usage")
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await
        .ok()?;
    let data: serde_json::Value = resp.json().await.ok()?;

    let normalize_entry = |raw: Option<&serde_json::Value>| -> Option<RateLimitEntry> {
        let r = raw?;
        let resets = r
            .get("resets_at")
            .or_else(|| r.get("resetsAt"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        Some(RateLimitEntry {
            utilization: r
                .get("utilization")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            resets_at: if resets.is_empty() {
                "—".to_string()
            } else {
                format_resets_at(resets)
            },
        })
    };

    let five_hour = normalize_entry(data.get("five_hour"))
        .or_else(|| normalize_entry(data.get("fiveHour")))
        .or_else(|| {
            normalize_entry(
                data.get("limits")
                    .and_then(|l| l.get("five_hour")),
            )
        })?;
    let seven_day = normalize_entry(data.get("seven_day"))
        .or_else(|| normalize_entry(data.get("sevenDay")))?;
    let seven_day_sonnet = normalize_entry(data.get("seven_day_sonnet"))
        .or_else(|| normalize_entry(data.get("sevenDaySonnet")));

    Some(RateLimits {
        five_hour,
        seven_day,
        seven_day_sonnet,
    })
}

fn format_resets_at(iso: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let ts_ms = parse_iso_ms(iso).unwrap_or(now_ms);
    let diff_ms = ts_ms - now_ms;
    if diff_ms <= 0 {
        return "now".to_string();
    }
    let total_mins = (diff_ms / 60_000) as i64;
    let days = total_mins / (60 * 24);
    let hours = (total_mins % (60 * 24)) / 60;
    let mins = total_mins % 60;
    if days > 0 {
        format!("{}d {}h", days, hours)
    } else if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

fn parse_iso_ms(s: &str) -> Option<i64> {
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

fn parse_daily_entry(v: &serde_json::Value) -> Option<DailyUsage> {
    Some(DailyUsage {
        cost: v.get("totalCost").and_then(|v| v.as_f64())?,
        total_tokens: v.get("totalTokens").and_then(|v| v.as_i64()).unwrap_or(0),
        input_tokens: v.get("inputTokens").and_then(|v| v.as_i64()).unwrap_or(0),
        output_tokens: v.get("outputTokens").and_then(|v| v.as_i64()).unwrap_or(0),
        cache_creation_tokens: v.get("cacheCreationTokens").and_then(|v| v.as_i64()).unwrap_or(0),
        cache_read_tokens: v.get("cacheReadTokens").and_then(|v| v.as_i64()).unwrap_or(0),
        date: v.get("date").and_then(|v| v.as_str()).unwrap_or("").to_string(),
    })
}

fn normalize_monthly(raw: &serde_json::Value) -> Option<MonthlyUsage> {
    let entry = raw
        .get("monthly")
        .and_then(|v| v.as_array())
        .and_then(|a| a.last())?;
    Some(MonthlyUsage {
        month: entry.get("month").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        total_cost: entry.get("totalCost").and_then(|v| v.as_f64()).unwrap_or(0.0),
        total_tokens: entry.get("totalTokens").and_then(|v| v.as_i64()).unwrap_or(0),
        input_tokens: entry.get("inputTokens").and_then(|v| v.as_i64()).unwrap_or(0),
        output_tokens: entry.get("outputTokens").and_then(|v| v.as_i64()).unwrap_or(0),
        cache_creation_tokens: entry.get("cacheCreationTokens").and_then(|v| v.as_i64()).unwrap_or(0),
        cache_read_tokens: entry.get("cacheReadTokens").and_then(|v| v.as_i64()).unwrap_or(0),
        model_breakdowns: parse_model_breakdowns(entry.get("modelBreakdowns")),
    })
}

fn normalize_total(raw: &serde_json::Value) -> Option<TotalUsage> {
    let t = raw.get("totals")?;
    Some(TotalUsage {
        total_cost: t.get("totalCost").and_then(|v| v.as_f64()).unwrap_or(0.0),
        total_tokens: t.get("totalTokens").and_then(|v| v.as_i64()).unwrap_or(0),
        input_tokens: t.get("inputTokens").and_then(|v| v.as_i64()).unwrap_or(0),
        output_tokens: t.get("outputTokens").and_then(|v| v.as_i64()).unwrap_or(0),
        cache_creation_tokens: t.get("cacheCreationTokens").and_then(|v| v.as_i64()).unwrap_or(0),
        cache_read_tokens: t.get("cacheReadTokens").and_then(|v| v.as_i64()).unwrap_or(0),
        sessions: raw.get("daily").and_then(|v| v.as_array()).map(|a| a.len() as i64).unwrap_or(0),
        model_breakdowns: vec![],
    })
}

fn parse_model_breakdowns(v: Option<&serde_json::Value>) -> Vec<ModelBreakdown> {
    v.and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    Some(ModelBreakdown {
                        model_name: m
                            .get("modelName")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        total_tokens: m
                            .get("totalTokens")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0),
                        total_cost: m.get("totalCost").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}
