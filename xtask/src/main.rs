use std::io::{self, Write};
use std::process::Command;

use anyhow::{bail, Context, Result};
use toml_edit::DocumentMut;

fn main() -> Result<()> {
    let bump_kind = std::env::args().nth(1).unwrap_or_else(|| "patch".to_string());

    match bump_kind.as_str() {
        "patch" | "minor" | "major" => release(&bump_kind),
        other => bail!("unknown command: {other}\n\nUsage: cargo release [patch|minor|major]"),
    }
}

fn release(bump_kind: &str) -> Result<()> {
    ensure_clean_tree()?;

    let cargo_toml_path = "Cargo.toml";
    let source = std::fs::read_to_string(cargo_toml_path)?;
    let mut doc: DocumentMut = source.parse().context("failed to parse Cargo.toml")?;

    let current = doc["package"]["version"]
        .as_str()
        .context("missing package.version")?
        .to_string();

    let parts: Vec<u32> = current
        .split('.')
        .map(|p| p.parse::<u32>().context("invalid version segment"))
        .collect::<Result<_>>()?;

    let (major, minor, patch) = (parts[0], parts[1], parts[2]);

    let next = match bump_kind {
        "major" => format!("{}.0.0", major + 1),
        "minor" => format!("{}.{}.0", major, minor + 1),
        _ => format!("{}.{}.{}", major, minor, patch + 1),
    };

    println!("\n  version  {} → {}", current, next);
    println!("  tag      v{}", next);
    println!("  steps    bump → commit → tag → publish → push\n");
    confirm("Proceed?")?;

    doc["package"]["version"] = toml_edit::value(next.clone());
    std::fs::write(cargo_toml_path, doc.to_string())?;

    run("cargo", &["check", "--quiet"])?;
    run("git", &["add", "Cargo.toml", "Cargo.lock"])?;
    run("git", &["commit", "-m", &format!("chore: bump version to {next}")])?;
    run("git", &["tag", &format!("v{next}")])?;
    run("cargo", &["publish"])?;
    run("git", &["push"])?;
    run("git", &["push", "--tags"])?;

    println!("\n✓ Released v{next}");
    Ok(())
}

fn ensure_clean_tree() -> Result<()> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .output()?;
    let dirty: Vec<_> = std::str::from_utf8(&output.stdout)?
        .lines()
        .filter(|l| !l.starts_with("??"))
        .collect();
    if !dirty.is_empty() {
        bail!("working tree has uncommitted changes — commit or stash first:\n{}", dirty.join("\n"));
    }
    Ok(())
}

fn confirm(prompt: &str) -> Result<()> {
    print!("{prompt} [Y/n] ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    if input.trim().eq_ignore_ascii_case("n") {
        bail!("aborted");
    }
    Ok(())
}

fn run(cmd: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(cmd).args(args).status()?;
    if !status.success() {
        bail!("`{cmd} {}` failed", args.join(" "));
    }
    Ok(())
}
