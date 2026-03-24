use anyhow::Result;

const SETTINGS_RELATIVE: &str = ".claude/settings.json";
const HOOK_EVENTS: &[&str] = &[
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "Stop",
    "Notification",
    "PermissionRequest",
    "PreCompact",
    "SubagentStop",
];

pub fn run() -> Result<()> {
    let home = std::env::var("HOME").unwrap_or_default();
    let settings_path = std::path::Path::new(&home).join(SETTINGS_RELATIVE);

    let exe = std::env::current_exe()?;
    let hook_command = format!("{} hook", exe.display());

    let mut settings: serde_json::Value = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    let mut registered = 0u32;
    let mut skipped = 0u32;

    for &event in HOOK_EVENTS {
        let already = settings
            .get("hooks")
            .and_then(|h| h.get(event))
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
            .unwrap_or(false);

        if already {
            println!("  ✓ {} — already registered", event);
            skipped += 1;
            continue;
        }

        let entry = serde_json::json!({
            "hooks": [{ "type": "command", "command": hook_command }]
        });

        settings
            .as_object_mut()
            .unwrap()
            .entry("hooks")
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
            .unwrap()
            .entry(event)
            .or_insert_with(|| serde_json::json!([]))
            .as_array_mut()
            .unwrap()
            .push(entry);

        println!("  + {} — registered", event);
        registered += 1;
    }

    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(&settings)? + "\n";
    std::fs::write(&settings_path, content)?;

    println!("\nclaude-dash: {} hook(s) registered, {} already present", registered, skipped);
    println!("hook command: {}", hook_command);
    println!("settings:    {}", settings_path.display());

    if registered > 0 {
        println!("\nRestart any running Claude Code sessions to pick up the new hooks.");
    }

    Ok(())
}
