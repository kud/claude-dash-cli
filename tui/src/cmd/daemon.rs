use std::collections::HashMap;
use std::io::Read as _;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::{oneshot, watch, Mutex};

const HOOK_SOCKET_PATH: &str = "/tmp/claude-dash.sock";
const TUI_SOCKET_PATH: &str = "/tmp/claude-dash-tui.sock";
const PID_PATH: &str = "/tmp/claude-dash.pid";
const PERMISSION_TIMEOUT_SECS: u64 = 30;

// ── Incoming event from hook process ────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct HookEventRaw {
    #[serde(default)]
    session_id: String,
    #[serde(default)]
    transcript_path: String,
    #[serde(default)]
    cwd: String,
    #[serde(default)]
    _permission_mode: String,
    #[serde(default)]
    hook_event_name: String,
    #[serde(default)]
    tool_name: Option<String>,
    #[serde(default)]
    tool_input: Option<serde_json::Value>,
    #[serde(default)]
    tool_output: Option<String>,
    #[serde(default)]
    tool_use_id: Option<String>,
    #[serde(default)]
    success: Option<bool>,
    #[serde(default)]
    notification_type: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    pid: i64,
    #[serde(default)]
    ts: i64,
    // Assigned by daemon for PermissionRequest connections
    #[serde(default)]
    connection_id: Option<String>,
}

// ── Internal state types (serialize to camelCase for TUI wire format) ────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolHistoryEntry {
    tool_name: String,
    tool_input: serde_json::Value,
    tool_output: Option<String>,
    success: Option<bool>,
    tool_use_id: Option<String>,
    started_at: i64,
    ended_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentNode {
    agent_id: String,
    status: String,
    started_at: i64,
    stopped_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum SessionStatus {
    WaitingForInput,
    Processing,
    RunningTool,
    WaitingForApproval,
    Compacting,
    Ended,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionState {
    session_id: String,
    status: SessionStatus,
    cwd: String,
    transcript_path: String,
    pid: i64,
    started_at: i64,
    last_event_at: i64,
    current_tool: Option<String>,
    current_tool_input: Option<serde_json::Value>,
    current_tool_use_id: Option<String>,
    tool_history: Vec<ToolHistoryEntry>,
    agents: Vec<AgentNode>,
    last_notification: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PendingPermission {
    connection_id: String,
    session_id: String,
    tool_name: String,
    tool_input: serde_json::Value,
    tool_use_id: Option<String>,
    cwd: String,
    requested_at: i64,
}

#[derive(Debug, Clone)]
struct DaemonState {
    sessions: HashMap<String, SessionState>,
    pending_permissions: Vec<PendingPermission>,
}

impl DaemonState {
    fn new() -> Self {
        Self { sessions: HashMap::new(), pending_permissions: vec![] }
    }
}

// ── Wire message to TUI ───────────────────────────────────────────────────────

#[derive(Serialize)]
struct TuiStateMessage<'a> {
    #[serde(rename = "type")]
    msg_type: &'static str,
    sessions: Vec<&'a SessionState>,
    #[serde(rename = "pendingPermissions")]
    pending_permissions: &'a [PendingPermission],
}

// ── Wire message from TUI ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct TuiMessage {
    #[serde(rename = "type", default)]
    msg_type: String,
    #[serde(rename = "connectionId", default)]
    connection_id: String,
    #[serde(default)]
    decision: String,
}

type SharedState = Arc<Mutex<DaemonState>>;
type PendingMap = Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>;
type ChangeTx = Arc<watch::Sender<u64>>;

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn generate_uuid() -> String {
    let mut bytes = [0u8; 16];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut bytes);
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    )
}

// ── State machine ─────────────────────────────────────────────────────────────

fn get_or_create(state: &DaemonState, event: &HookEventRaw) -> SessionState {
    state.sessions.get(&event.session_id).cloned().unwrap_or_else(|| SessionState {
        session_id: event.session_id.clone(),
        status: SessionStatus::WaitingForInput,
        cwd: event.cwd.clone(),
        transcript_path: event.transcript_path.clone(),
        pid: event.pid,
        started_at: event.ts,
        last_event_at: event.ts,
        current_tool: None,
        current_tool_input: None,
        current_tool_use_id: None,
        tool_history: vec![],
        agents: vec![],
        last_notification: None,
    })
}

fn upsert(state: &mut DaemonState, session: SessionState) {
    state.sessions.insert(session.session_id.clone(), session);
}

fn apply_event(state: &mut DaemonState, event: &HookEventRaw) {
    let ts = if event.ts == 0 { now_ms() } else { event.ts };

    match event.hook_event_name.as_str() {
        "SessionStart" => {
            let mut s = get_or_create(state, event);
            s.cwd = event.cwd.clone();
            s.transcript_path = event.transcript_path.clone();
            s.pid = event.pid;
            s.status = SessionStatus::WaitingForInput;
            s.last_event_at = ts;
            upsert(state, s);
        }
        "SessionEnd" => {
            let mut s = get_or_create(state, event);
            s.status = SessionStatus::Ended;
            s.last_event_at = ts;
            upsert(state, s);
        }
        "UserPromptSubmit" => {
            let mut s = get_or_create(state, event);
            s.status = SessionStatus::Processing;
            s.last_event_at = ts;
            upsert(state, s);
        }
        "PreToolUse" => {
            let mut s = get_or_create(state, event);
            let entry = ToolHistoryEntry {
                tool_name: event.tool_name.clone().unwrap_or_default(),
                tool_input: event.tool_input.clone().unwrap_or_default(),
                tool_output: None,
                success: None,
                tool_use_id: event.tool_use_id.clone(),
                started_at: ts,
                ended_at: None,
            };
            s.status = SessionStatus::RunningTool;
            s.current_tool = event.tool_name.clone();
            s.current_tool_input = event.tool_input.clone();
            s.current_tool_use_id = event.tool_use_id.clone();
            s.last_event_at = ts;
            s.tool_history.push(entry);
            upsert(state, s);
        }
        "PostToolUse" => {
            let mut s = get_or_create(state, event);
            if let Some(id) = &event.tool_use_id {
                for entry in &mut s.tool_history {
                    if entry.tool_use_id.as_deref() == Some(id.as_str()) {
                        entry.tool_output = event.tool_output.clone();
                        entry.success = event.success;
                        entry.ended_at = Some(ts);
                    }
                }
            }
            s.status = SessionStatus::Processing;
            s.current_tool = None;
            s.current_tool_input = None;
            s.current_tool_use_id = None;
            s.last_event_at = ts;
            upsert(state, s);
        }
        "PermissionRequest" => {
            let mut s = get_or_create(state, event);
            s.status = SessionStatus::WaitingForApproval;
            s.last_event_at = ts;
            upsert(state, s);
            if let Some(connection_id) = &event.connection_id {
                state.pending_permissions.push(PendingPermission {
                    connection_id: connection_id.clone(),
                    session_id: event.session_id.clone(),
                    tool_name: event.tool_name.clone().unwrap_or_default(),
                    tool_input: event.tool_input.clone().unwrap_or_default(),
                    tool_use_id: event.tool_use_id.clone(),
                    cwd: event.cwd.clone(),
                    requested_at: ts,
                });
            }
        }
        "Stop" => {
            let mut s = get_or_create(state, event);
            s.status = SessionStatus::WaitingForInput;
            s.current_tool = None;
            s.current_tool_input = None;
            s.current_tool_use_id = None;
            s.last_event_at = ts;
            upsert(state, s);
        }
        "SubagentStop" => {
            if let Some(s) = state.sessions.get_mut(&event.session_id) {
                let agent_id = event.tool_use_id.clone().unwrap_or_else(|| event.session_id.clone());
                if let Some(a) = s.agents.iter_mut().find(|a| a.agent_id == agent_id) {
                    a.status = "stopped".to_string();
                    a.stopped_at = Some(ts);
                } else {
                    s.agents.push(AgentNode {
                        agent_id,
                        status: "stopped".to_string(),
                        started_at: ts,
                        stopped_at: Some(ts),
                    });
                }
                s.last_event_at = ts;
            }
        }
        "Notification" => {
            let mut s = get_or_create(state, event);
            s.status = if event.notification_type.as_deref() == Some("idle_prompt") {
                SessionStatus::WaitingForInput
            } else {
                SessionStatus::Processing
            };
            s.last_event_at = ts;
            s.last_notification = event.message.clone();
            upsert(state, s);
        }
        "PreCompact" => {
            let mut s = get_or_create(state, event);
            s.status = SessionStatus::Compacting;
            s.last_event_at = ts;
            upsert(state, s);
        }
        _ => {
            let mut s = get_or_create(state, event);
            s.last_event_at = ts;
            upsert(state, s);
        }
    }
}

fn serialize_state(state: &DaemonState, msg_type: &'static str) -> String {
    let msg = TuiStateMessage {
        msg_type,
        sessions: state.sessions.values().collect(),
        pending_permissions: &state.pending_permissions,
    };
    serde_json::to_string(&msg).unwrap_or_default() + "\n"
}

// ── Hook server ───────────────────────────────────────────────────────────────

async fn serve_hook(state: SharedState, pending: PendingMap, change_tx: ChangeTx) -> Result<()> {
    let listener = UnixListener::bind(HOOK_SOCKET_PATH)?;
    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(handle_hook_connection(
            stream,
            state.clone(),
            pending.clone(),
            change_tx.clone(),
        ));
    }
}

async fn handle_hook_connection(
    stream: tokio::net::UnixStream,
    state: SharedState,
    pending: PendingMap,
    change_tx: ChangeTx,
) {
    let (reader, mut writer) = tokio::io::split(stream);
    let mut lines = BufReader::new(reader).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        let Ok(mut event) = serde_json::from_str::<HookEventRaw>(&line) else {
            continue;
        };

        if event.hook_event_name == "PermissionRequest" {
            let connection_id = generate_uuid();
            event.connection_id = Some(connection_id.clone());

            let (tx, rx) = oneshot::channel::<String>();
            pending.lock().await.insert(connection_id.clone(), tx);

            {
                let mut s = state.lock().await;
                apply_event(&mut s, &event);
                change_tx.send_modify(|v| *v += 1);
            }

            let decision = tokio::time::timeout(
                Duration::from_secs(PERMISSION_TIMEOUT_SECS),
                rx,
            )
            .await
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or_else(|| "ask".to_string());

            // Clean up on timeout (no-op if TUI already removed it)
            {
                let mut p = pending.lock().await;
                p.remove(&connection_id);
            }
            {
                let mut s = state.lock().await;
                s.pending_permissions.retain(|p| p.connection_id != connection_id);
                change_tx.send_modify(|v| *v += 1);
            }

            let response = format!("{{\"decision\":\"{}\"}}\n", decision);
            let _ = writer.write_all(response.as_bytes()).await;
            return;
        } else {
            let mut s = state.lock().await;
            apply_event(&mut s, &event);
            change_tx.send_modify(|v| *v += 1);
            return;
        }
    }
}

// ── TUI server ────────────────────────────────────────────────────────────────

async fn serve_tui(state: SharedState, pending: PendingMap, change_tx: ChangeTx) -> Result<()> {
    let listener = UnixListener::bind(TUI_SOCKET_PATH)?;
    loop {
        let (stream, _) = listener.accept().await?;
        let change_rx = change_tx.subscribe();
        tokio::spawn(handle_tui_connection(
            stream,
            state.clone(),
            pending.clone(),
            change_rx,
        ));
    }
}

async fn handle_tui_connection(
    stream: tokio::net::UnixStream,
    state: SharedState,
    pending: PendingMap,
    mut change_rx: watch::Receiver<u64>,
) {
    let (reader, mut writer) = tokio::io::split(stream);
    let mut lines = BufReader::new(reader).lines();

    {
        let s = state.lock().await;
        let snapshot = serialize_state(&s, "StateSnapshot");
        if writer.write_all(snapshot.as_bytes()).await.is_err() {
            return;
        }
    }

    loop {
        tokio::select! {
            line_result = lines.next_line() => {
                match line_result {
                    Ok(Some(line)) => {
                        let line = line.trim().to_string();
                        if line.is_empty() { continue; }
                        if let Ok(msg) = serde_json::from_str::<TuiMessage>(&line) {
                            if msg.msg_type == "PermissionDecision" {
                                if let Some(sender) = pending.lock().await.remove(&msg.connection_id) {
                                    let _ = sender.send(msg.decision);
                                }
                            }
                        }
                    }
                    _ => return,
                }
            }
            Ok(()) = change_rx.changed() => {
                let delta = {
                    let s = state.lock().await;
                    serialize_state(&s, "StateDelta")
                };
                if writer.write_all(delta.as_bytes()).await.is_err() {
                    return;
                }
            }
        }
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub async fn run() -> Result<()> {
    let _ = std::fs::remove_file(HOOK_SOCKET_PATH);
    let _ = std::fs::remove_file(TUI_SOCKET_PATH);
    std::fs::write(PID_PATH, format!("{}", std::process::id()))?;

    let state: SharedState = Arc::new(Mutex::new(DaemonState::new()));
    let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
    let (change_tx, _initial_rx) = watch::channel(0u64);
    let change_tx = Arc::new(change_tx);

    eprintln!("[claude-dash daemon] hook={} tui={}", HOOK_SOCKET_PATH, TUI_SOCKET_PATH);

    let hook_state = state.clone();
    let hook_pending = pending.clone();
    let hook_tx = change_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = serve_hook(hook_state, hook_pending, hook_tx).await {
            eprintln!("[claude-dash daemon] hook server error: {}", e);
        }
    });

    let tui_state = state.clone();
    let tui_pending = pending.clone();
    let tui_tx = change_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = serve_tui(tui_state, tui_pending, tui_tx).await {
            eprintln!("[claude-dash daemon] tui server error: {}", e);
        }
    });

    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = sigterm.recv() => {}
    }

    let _ = std::fs::remove_file(HOOK_SOCKET_PATH);
    let _ = std::fs::remove_file(TUI_SOCKET_PATH);
    let _ = std::fs::remove_file(PID_PATH);
    Ok(())
}
