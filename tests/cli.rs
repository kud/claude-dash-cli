use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_claude-dash"))
}

#[test]
fn help_lists_subcommands() {
    let out = bin().arg("--help").output().expect("run claude-dash --help");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    for sub in ["daemon", "hook", "install"] {
        assert!(stdout.contains(sub), "missing `{sub}` in --help:\n{stdout}");
    }
}

#[test]
fn unknown_subcommand_fails() {
    let out = bin().arg("definitely-not-a-command").output().expect("run");
    assert!(!out.status.success());
}
