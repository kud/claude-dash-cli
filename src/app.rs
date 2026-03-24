use crossterm::event::KeyCode;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::daemon::DaemonCommand;
use std::collections::{HashMap, HashSet};

use crate::types::{
    DailyUsage, DaemonMessage, MonthlyUsage, PendingPermission, RateLimits, SessionState,
    TotalUsage, UsageData,
};

#[derive(Serialize, Deserialize)]
struct UsageCache {
    today: Option<DailyUsage>,
    yesterday: Option<DailyUsage>,
    monthly: Option<MonthlyUsage>,
    total: Option<TotalUsage>,
    daily_history: Vec<DailyUsage>,
}

fn cache_path() -> Option<std::path::PathBuf> {
    std::env::var("HOME").ok().map(|h| std::path::PathBuf::from(h).join(".claude-dash-usage.json"))
}

fn load_usage_cache() -> Option<UsageCache> {
    let path = cache_path()?;
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn save_usage_cache(usage: &UsageData) {
    let Some(path) = cache_path() else { return };
    let cache = UsageCache {
        today: usage.today.clone(),
        yesterday: usage.yesterday.clone(),
        monthly: usage.monthly.clone(),
        total: usage.total.clone(),
        daily_history: usage.daily_history.clone(),
    };
    if let Ok(json) = serde_json::to_vec(&cache) {
        let _ = std::fs::write(path, json);
    }
}

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
}

#[derive(Clone, Copy, PartialEq)]
pub enum SortMode {
    Recent,
    Alphabetical,
}

impl SortMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Recent => "recent",
            Self::Alphabetical => "a-z",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Recent => Self::Alphabetical,
            Self::Alphabetical => Self::Recent,
        }
    }
}

pub struct App {
    pub sessions: Vec<SessionState>,
    pub pending_permissions: Vec<PendingPermission>,
    pub connected: bool,
    pub selected_index: usize,
    pub list_offset: usize,
    pub usage: UsageData,
    pub show_new_session: bool,
    pub new_session_input: String,
    pub new_session_launched: bool,
    pub new_session_error: Option<String>,
    pub tick_count: u64,
    pub session_allowed_tools: HashMap<String, HashSet<String>>,
    pub session_names: HashMap<String, String>,
    pub show_rename: bool,
    pub rename_input: String,
    pub sort_mode: SortMode,

    daemon_cmd_tx: mpsc::UnboundedSender<DaemonCommand>,
    usage_refresh_tx: mpsc::UnboundedSender<()>,
}

impl App {
    pub fn new(
        daemon_cmd_tx: mpsc::UnboundedSender<DaemonCommand>,
        usage_refresh_tx: mpsc::UnboundedSender<()>,
    ) -> Self {
        let mut usage = UsageData::default();
        if let Some(cache) = load_usage_cache() {
            usage.today = cache.today;
            usage.yesterday = cache.yesterday;
            usage.monthly = cache.monthly;
            usage.total = cache.total;
            usage.daily_history = cache.daily_history;
        }
        Self {
            sessions: vec![],
            pending_permissions: vec![],
            connected: false,
            selected_index: 0,
            list_offset: 0,
            usage,
            show_new_session: false,
            new_session_input: String::new(),
            new_session_launched: false,
            new_session_error: None,
            tick_count: 0,
            session_allowed_tools: HashMap::new(),
            session_names: HashMap::new(),
            show_rename: false,
            rename_input: String::new(),
            sort_mode: SortMode::Recent,
            daemon_cmd_tx,
            usage_refresh_tx,
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
                save_usage_cache(&self.usage);
            }
            AppEvent::RateLimitsLoaded(limits) => {
                self.usage.limits = Some(limits);
                self.usage.limits_loading = false;
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
        self.sort_sessions(&mut sessions);
        self.sessions = sessions;
        self.pending_permissions = perms;
        self.selected_index = self.clamped_index();
    }

    // Returns true if the app should quit.
    pub fn handle_key(&mut self, code: KeyCode, modifiers: crossterm::event::KeyModifiers) -> bool {
        if self.show_new_session {
            return self.handle_key_new_session(code, modifiers);
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
            KeyCode::Char('n') => {
                self.show_new_session = true;
                self.new_session_input.clear();
                self.new_session_launched = false;
                self.new_session_error = None;
            }
            KeyCode::Char('o') => {
                self.sort_mode = self.sort_mode.next();
                let mut sessions = std::mem::take(&mut self.sessions);
                self.sort_sessions(&mut sessions);
                self.sessions = sessions;
            }
            KeyCode::Char('r') => {
                let _ = self.usage_refresh_tx.send(());
            }
            KeyCode::Char('x') => {
                let selected_id = self.selected_session().map(|s| s.session_id.clone());
                self.sessions.retain(|s| s.status != crate::types::SessionStatus::Ended);
                // Reselect the previously selected session if it still exists.
                if let Some(id) = selected_id {
                    if let Some(pos) = self.sessions.iter().position(|s| s.session_id == id) {
                        self.selected_index = pos;
                    } else {
                        self.selected_index = self.clamped_index();
                    }
                }
            }
            KeyCode::Delete | KeyCode::Backspace => {
                if let Some(session) = self.selected_session() {
                    let id = session.session_id.clone();
                    self.sessions.retain(|s| s.session_id != id);
                    self.selected_index = self.clamped_index();
                }
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
            self.scroll_list_to_selected();
        }
    }

    fn select_next(&mut self) {
        if !self.sessions.is_empty() && self.selected_index < self.sessions.len() - 1 {
            self.selected_index += 1;
            self.scroll_list_to_selected();
        }
    }

    fn sort_sessions(&self, sessions: &mut Vec<SessionState>) {
        match self.sort_mode {
            SortMode::Recent => sessions.sort_by(|a, b| {
                a.status.sort_priority().cmp(&b.status.sort_priority())
                    .then(b.last_event_at.cmp(&a.last_event_at))
            }),
            SortMode::Alphabetical => sessions.sort_by(|a, b| {
                a.status.sort_priority().cmp(&b.status.sort_priority())
                    .then(a.cwd.cmp(&b.cwd))
            }),
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
