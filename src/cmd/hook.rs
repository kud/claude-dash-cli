use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

const SOCKET_PATH: &str = "/tmp/claude-dash.sock";

#[derive(Deserialize)]
struct RawInput {
    session_id: Option<String>,
    #[serde(default)]
    transcript_path: String,
    #[serde(default)]
    cwd: String,
    #[serde(default)]
    permission_mode: String,
    hook_event_name: Option<String>,
    tool_name: Option<String>,
    tool_input: Option<serde_json::Value>,
    tool_output: Option<String>,
    tool_use_id: Option<String>,
    success: Option<bool>,
    notification_type: Option<String>,
    message: Option<String>,
}

#[derive(Serialize)]
struct HookEvent {
    session_id: String,
    transcript_path: String,
    cwd: String,
    permission_mode: String,
    hook_event_name: String,
    pid: u32,
    ts: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    iterm_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_use_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    success: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notification_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Deserialize)]
struct PermissionDecisionMsg {
    decision: String,
}

#[derive(Serialize)]
struct HookOutput {
    #[serde(rename = "hookSpecificOutput")]
    hook_specific_output: HookSpecificOutput,
}

#[derive(Serialize)]
struct HookSpecificOutput {
    #[serde(rename = "hookEventName")]
    hook_event_name: &'static str,
    decision: DecisionBehavior,
}

#[derive(Serialize)]
struct DecisionBehavior {
    behavior: String,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub async fn run() -> Result<()> {
    let mut input = String::new();
    tokio::io::stdin().read_to_string(&mut input).await?;

    let input = input.trim();
    if input.is_empty() {
        return Ok(());
    }

    let Ok(raw) = serde_json::from_str::<RawInput>(input) else {
        return Ok(());
    };

    let Some(session_id) = raw.session_id else { return Ok(()) };
    let Some(hook_event_name) = raw.hook_event_name else { return Ok(()) };

    let ppid = unsafe { libc::getppid() } as u32;
    let iterm_session_id = std::env::var("ITERM_SESSION_ID")
        .ok()
        .and_then(|s| s.split(':').nth(1).map(|u| u.to_string()));
    let cwd = if raw.cwd.is_empty() {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default()
    } else {
        raw.cwd
    };

    let event = HookEvent {
        session_id,
        transcript_path: raw.transcript_path,
        cwd,
        permission_mode: raw.permission_mode,
        hook_event_name: hook_event_name.clone(),
        pid: ppid,
        ts: now_ms(),
        iterm_session_id,
        tool_name: raw.tool_name,
        tool_input: raw.tool_input,
        tool_output: raw.tool_output,
        tool_use_id: raw.tool_use_id,
        success: raw.success,
        notification_type: raw.notification_type,
        message: raw.message,
    };

    if hook_event_name == "PermissionRequest" {
        let decision = send_permission_request(&event).await;
        let behavior = match decision.as_str() {
            "allow" => "allow",
            "deny" => "deny",
            _ => return Ok(()), // daemon not running or timed out — let Claude ask the user
        };
        let output = HookOutput {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: "PermissionRequest",
                decision: DecisionBehavior { behavior: behavior.to_string() },
            },
        };
        println!("{}", serde_json::to_string(&output)?);
    } else {
        send_fire_and_forget(&event).await;
    }

    Ok(())
}

async fn send_fire_and_forget(event: &HookEvent) {
    if let Ok(mut stream) = UnixStream::connect(SOCKET_PATH).await {
        let line = serde_json::to_string(event).unwrap_or_default() + "\n";
        let _ = stream.write_all(line.as_bytes()).await;
    }
}

async fn send_permission_request(event: &HookEvent) -> String {
    let Ok(stream) = UnixStream::connect(SOCKET_PATH).await else {
        return "ask".to_string();
    };

    let (reader, mut writer) = tokio::io::split(stream);
    let line = serde_json::to_string(event).unwrap_or_default() + "\n";
    if writer.write_all(line.as_bytes()).await.is_err() {
        return "ask".to_string();
    }

    let mut lines = BufReader::new(reader).lines();
    match tokio::time::timeout(Duration::from_secs(35), lines.next_line()).await {
        Ok(Ok(Some(line))) => serde_json::from_str::<PermissionDecisionMsg>(&line)
            .map(|d| d.decision)
            .unwrap_or_else(|_| "ask".to_string()),
        _ => "ask".to_string(),
    }
}
