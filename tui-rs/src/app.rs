use crossterm::event::KeyCode;
use tokio::sync::{mpsc, watch};

use crate::daemon::DaemonCommand;
use std::collections::{HashMap, HashSet};

use crate::types::{
    DailyUsage, DaemonMessage, MonthlyUsage, PendingPermission, RateLimits, SessionState,
    TotalUsage, TranscriptMessage, UsageData,
};

pub enum AppEvent {
    DaemonConnected,
    DaemonDisconnected,
    DaemonMessage(DaemonMessage),
    UsageLoading,
    UsageLoaded {
        today: Option<DailyUsage>,
        yesterday: Option<DailyUsage>,
        monthly: Option<MonthlyUsage>,
        total: Option<TotalUsage>,
        daily_history: Vec<DailyUsage>,
    },
    RateLimitsLoaded(RateLimits),
    TranscriptLoaded(Vec<TranscriptMessage>),
}

pub struct App {
    pub sessions: Vec<SessionState>,
    pub pending_permissions: Vec<PendingPermission>,
    pub connected: bool,
    pub selected_index: usize,
    pub list_offset: usize,
    pub detail_scroll: usize,
    pub usage: UsageData,
    pub transcript: Vec<TranscriptMessage>,
    pub show_new_session: bool,
    pub new_session_input: String,
    pub new_session_launched: bool,
    pub new_session_error: Option<String>,
    pub show_input: bool,
    pub input_text: String,
    pub tick_count: u64,
    pub session_allowed_tools: HashMap<String, HashSet<String>>,
    pub session_names: HashMap<String, String>,
    pub show_rename: bool,
    pub rename_input: String,

    daemon_cmd_tx: mpsc::UnboundedSender<DaemonCommand>,
    usage_refresh_tx: mpsc::UnboundedSender<()>,
    transcript_path_tx: watch::Sender<Option<String>>,
    last_transcript_session: Option<String>,
}

impl App {
    pub fn new(
        daemon_cmd_tx: mpsc::UnboundedSender<DaemonCommand>,
        usage_refresh_tx: mpsc::UnboundedSender<()>,
        transcript_path_tx: watch::Sender<Option<String>>,
    ) -> Self {
        Self {
            sessions: vec![],
            pending_permissions: vec![],
            connected: false,
            selected_index: 0,
            list_offset: 0,
            detail_scroll: 0,
            usage: UsageData::default(),
            transcript: vec![],
            show_new_session: false,
            new_session_input: String::new(),
            new_session_launched: false,
            new_session_error: None,
            show_input: false,
            input_text: String::new(),
            tick_count: 0,
            session_allowed_tools: HashMap::new(),
            session_names: HashMap::new(),
            show_rename: false,
            rename_input: String::new(),
            daemon_cmd_tx,
            usage_refresh_tx,
            transcript_path_tx,
            last_transcript_session: None,
        }
    }

    pub fn selected_session(&self) -> Option<&SessionState> {
        self.sessions.get(self.clamped_index())
    }

    pub fn session_display_name<'a>(&'a self, session_id: &'a str) -> &'a str {
        self.session_names
            .get(session_id)
            .map(|s| s.as_str())
            .unwrap_or_else(|| &session_id[..8.min(session_id.len())])
    }

    pub fn selected_pending_permission(&self) -> Option<&PendingPermission> {
        let id = self.selected_session().map(|s| &s.session_id)?;
        self.pending_permissions.iter().find(|p| &p.session_id == id)
    }

    pub fn clamped_index(&self) -> usize {
        if self.sessions.is_empty() {
            0
        } else {
            self.selected_index.min(self.sessions.len() - 1)
        }
    }

    pub fn active_count(&self) -> usize {
        self.sessions.iter().filter(|s| s.status.is_active()).count()
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::DaemonConnected => self.connected = true,
            AppEvent::DaemonDisconnected => self.connected = false,
            AppEvent::DaemonMessage(msg) => self.apply_daemon_message(msg),
            AppEvent::UsageLoading => {
                self.usage.loading = true;
                self.usage.error = None;
            }
            AppEvent::UsageLoaded { today, yesterday, monthly, total, daily_history } => {
                self.usage.loading = false;
                self.usage.today = today;
                self.usage.yesterday = yesterday;
                self.usage.monthly = monthly;
                self.usage.total = total;
                self.usage.daily_history = daily_history;
                self.usage.error = None;
                self.usage.last_fetched = Some(std::time::Instant::now());
            }
            AppEvent::RateLimitsLoaded(limits) => {
                self.usage.limits = Some(limits);
                self.usage.limits_loading = false;
            }
            AppEvent::TranscriptLoaded(messages) => {
                self.transcript = messages;
            }
        }
    }

    fn apply_daemon_message(&mut self, msg: DaemonMessage) {
        let (mut sessions, perms) = match msg {
            DaemonMessage::StateSnapshot { sessions, pending_permissions } => {
                (sessions, pending_permissions)
            }
            DaemonMessage::StateDelta { sessions, pending_permissions } => {
                (sessions, pending_permissions)
            }
        };
        for session in &mut sessions {
            if perms.iter().any(|p| p.session_id == session.session_id) {
                session.status = crate::types::SessionStatus::WaitingForApproval;
            }
        }
        // Auto-allow any permission matching the session's approved tool list.
        let (auto_allow, perms): (Vec<_>, Vec<_>) = perms.into_iter().partition(|p| {
            self.session_allowed_tools
                .get(&p.session_id)
                .map(|tools| tools.contains(&p.tool_name))
                .unwrap_or(false)
        });
        for perm in auto_allow {
            self.send_decision(&perm.connection_id, "allow");
        }

        for session in &mut sessions {
            if perms.iter().any(|p| p.session_id == session.session_id) {
                session.status = crate::types::SessionStatus::WaitingForApproval;
            }
        }
        sessions.sort_by_key(|s| s.status.sort_priority());
        self.sessions = sessions;
        self.pending_permissions = perms;
        self.selected_index = self.clamped_index();
        self.sync_transcript_path();
    }

    fn sync_transcript_path(&mut self) {
        let path = self.selected_session().map(|s| s.transcript_path.clone());
        let session_id = self.selected_session().map(|s| s.session_id.clone());

        if session_id != self.last_transcript_session {
            self.last_transcript_session = session_id;
            self.transcript.clear();
            self.detail_scroll = 0;
            let _ = self.transcript_path_tx.send(path);
        }
    }

    // Returns true if the app should quit.
    pub fn handle_key(&mut self, code: KeyCode, modifiers: crossterm::event::KeyModifiers) -> bool {
        if self.show_new_session {
            return self.handle_key_new_session(code, modifiers);
        }
        if self.show_input {
            return self.handle_key_input(code);
        }
        if self.show_rename {
            return self.handle_key_rename(code);
        }

        match code {
            KeyCode::Char('q') => return true,
            KeyCode::Char('Q') => {
                self.quit_and_kill();
                return true;
            }
            KeyCode::Char('c') if modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                return true;
            }
            KeyCode::Up | KeyCode::Char('k') => self.select_prev(),
            KeyCode::Down | KeyCode::Char('j') => self.select_next(),
            KeyCode::Char('a') => {
                if let Some(perm) = self.selected_pending_permission().cloned() {
                    self.send_decision(&perm.connection_id, "allow");
                }
            }
            KeyCode::Char('s') => {
                if let Some(perm) = self.selected_pending_permission().cloned() {
                    self.session_allowed_tools
                        .entry(perm.session_id.clone())
                        .or_default()
                        .insert(perm.tool_name.clone());
                    self.send_decision(&perm.connection_id, "allow");
                }
            }
            KeyCode::Char('d') => {
                if let Some(perm) = self.selected_pending_permission().cloned() {
                    self.send_decision(&perm.connection_id, "deny");
                }
            }
            KeyCode::Char('e') => {
                if let Some(session) = self.selected_session() {
                    self.rename_input = self.session_names
                        .get(&session.session_id)
                        .cloned()
                        .unwrap_or_default();
                    self.show_rename = true;
                }
            }
            KeyCode::Char('i') => {
                if self.selected_session().is_some() {
                    self.show_input = true;
                    self.input_text.clear();
                }
            }
            KeyCode::Enter => {
                let is_waiting = self.selected_session()
                    .map(|s| s.status == crate::types::SessionStatus::WaitingForInput)
                    .unwrap_or(false);
                if is_waiting {
                    self.show_input = true;
                    self.input_text.clear();
                }
            }
            KeyCode::Char('n') => {
                self.show_new_session = true;
                self.new_session_input.clear();
                self.new_session_launched = false;
                self.new_session_error = None;
            }
            KeyCode::Char('r') => {
                let _ = self.usage_refresh_tx.send(());
            }
            KeyCode::PageUp => {
                self.detail_scroll = self.detail_scroll.saturating_add(20);
            }
            KeyCode::PageDown => {
                self.detail_scroll = self.detail_scroll.saturating_sub(20);
            }
            _ => {}
        }
        false
    }

    fn handle_key_new_session(
        &mut self,
        code: KeyCode,
        _modifiers: crossterm::event::KeyModifiers,
    ) -> bool {
        match code {
            KeyCode::Esc => {
                self.show_new_session = false;
            }
            KeyCode::Enter => {
                let dir = self.new_session_input.trim().to_string();
                let dir = if let Ok(home) = std::env::var("HOME") {
                    dir.replacen("~", &home, 1)
                } else {
                    dir
                };
                if !dir.is_empty() {
                    match launch_session(&dir) {
                        Ok(_) => {
                            self.new_session_launched = true;
                            // auto-close after a tick
                        }
                        Err(e) => {
                            self.new_session_error = Some(e);
                        }
                    }
                }
            }
            KeyCode::Backspace => {
                self.new_session_input.pop();
            }
            KeyCode::Char(c) => {
                self.new_session_input.push(c);
            }
            _ => {}
        }
        false
    }

    fn handle_key_rename(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Esc => {
                self.show_rename = false;
            }
            KeyCode::Enter => {
                if let Some(session) = self.selected_session() {
                    let id = session.session_id.clone();
                    let name = self.rename_input.trim().to_string();
                    if name.is_empty() {
                        self.session_names.remove(&id);
                    } else {
                        self.session_names.insert(id, name);
                    }
                }
                self.show_rename = false;
            }
            KeyCode::Backspace => { self.rename_input.pop(); }
            KeyCode::Char(c) => { self.rename_input.push(c); }
            _ => {}
        }
        false
    }

    fn handle_key_input(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Esc => {
                self.show_input = false;
            }
            KeyCode::Enter => {
                let text = self.input_text.trim().to_string();
                if !text.is_empty() {
                    if let Some(session) = self.selected_session() {
                        let session_id = session.session_id.clone();
                        let _ = self.daemon_cmd_tx.send(DaemonCommand::SendMessage { session_id, text });
                    }
                }
                self.show_input = false;
            }
            KeyCode::Backspace => {
                self.input_text.pop();
            }
            KeyCode::Char(c) => {
                self.input_text.push(c);
            }
            _ => {}
        }
        false
    }

    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
        if self.new_session_launched {
            self.show_new_session = false;
            self.new_session_launched = false;
        }
    }

    pub fn select_prev_pub(&mut self) { self.select_prev(); }
    pub fn select_next_pub(&mut self) { self.select_next(); }

    fn select_prev(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.detail_scroll = 0;
            self.sync_transcript_path();
            self.scroll_list_to_selected();
        }
    }

    fn select_next(&mut self) {
        if !self.sessions.is_empty() && self.selected_index < self.sessions.len() - 1 {
            self.selected_index += 1;
            self.detail_scroll = 0;
            self.sync_transcript_path();
            self.scroll_list_to_selected();
        }
    }

    fn scroll_list_to_selected(&mut self) {
        // Keep the list offset in sync — called after changing selection.
        // The visible window size isn't known here; the UI will clamp it during render.
        if self.selected_index < self.list_offset {
            self.list_offset = self.selected_index;
        }
    }

    fn send_decision(&self, connection_id: &str, decision: &str) {
        let _ = self.daemon_cmd_tx.send(DaemonCommand::SendDecision {
            connection_id: connection_id.to_string(),
            decision: decision.to_string(),
        });
    }

    fn quit_and_kill(&self) {
        if let Ok(raw) = std::fs::read_to_string("/tmp/claude-dash.pid") {
            if let Ok(pid) = raw.trim().parse::<i32>() {
                unsafe {
                    libc::kill(pid, libc::SIGTERM);
                }
            }
        }
    }

    pub fn recent_cwds(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        let mut sorted = self.sessions.clone();
        sorted.sort_by(|a, b| b.last_event_at.cmp(&a.last_event_at));
        for s in sorted {
            if seen.insert(s.cwd.clone()) {
                result.push(s.cwd);
            }
        }
        result
    }
}

pub fn hooks_installed() -> bool {
    check_hooks_installed()
}

fn check_hooks_installed() -> bool {
    let home = std::env::var("HOME").unwrap_or_default();
    let path = std::path::Path::new(&home).join(".claude").join("settings.json");
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
        return false;
    };
    // Check that at least PreToolUse has a hook command referencing claude-dash
    json.get("hooks")
        .and_then(|h| h.get("PreToolUse"))
        .and_then(|v| v.as_array())
        .map(|groups| {
            groups.iter().any(|group| {
                group
                    .get("hooks")
                    .and_then(|h| h.as_array())
                    .map(|hooks| {
                        hooks.iter().any(|h| {
                            h.get("command")
                                .and_then(|c| c.as_str())
                                .map(|c| c.contains("claude-dash"))
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn launch_session(dir: &str) -> Result<(), String> {
    if std::env::var("TMUX").is_ok() {
        let name = dir.split('/').last().unwrap_or("claude");
        let name = &name[..name.len().min(20)];
        std::process::Command::new("tmux")
            .args(["new-window", "-n", name, "-c", dir, "claude"])
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    let in_iterm = std::env::var("TERM_PROGRAM").map(|v| v == "iTerm.app").unwrap_or(false)
        || std::env::var("ITERM_SESSION_ID").is_ok();

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let script = if in_iterm {
        format!(
            r#"tell application "iTerm"
  tell current window
    create tab with default profile command "{shell} -l -c 'cd {dir} && exec claude'"
  end tell
end tell"#
        )
    } else {
        format!(r#"tell application "Terminal" to do script "cd {dir} && claude""#)
    };

    std::process::Command::new("osascript")
        .args(["-e", &script])
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
