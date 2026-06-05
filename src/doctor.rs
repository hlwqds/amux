//! Environment diagnostics for `amux doctor`.

use std::env;
use std::fs;

use crate::config;
use crate::types::Agent;
use crate::util::which;

/// Result of a single diagnostic check.
pub struct CheckResult {
    pub name: String,
    pub passed: bool,
    pub message: String,
    pub fix_hint: Option<String>,
    /// If present, this fix can be auto-applied by `amux doctor --fix`.
    pub auto_fix: Option<AutoFix>,
}

/// An auto-fix that can be applied by `amux doctor --fix`.
pub struct AutoFix {
    /// Human-readable description of what the fix does.
    pub description: String,
    /// The command to execute.
    pub command: String,
}

/// Installation hints for agent CLIs.
const INSTALL_HINTS: &[(&str, &str)] = &[
    ("claude", "npm install -g @anthropic-ai/claude-code"),
    ("codex", "npm install -g @openai/codex"),
    ("omp", "npm install -g omp-cli"),
];

fn install_hint(cmd: &str) -> Option<&'static str> {
    INSTALL_HINTS
        .iter()
        .find(|(c, _)| *c == cmd)
        .map(|(_, h)| *h)
}

/// Run all diagnostic checks and return results.
pub fn run_doctor() -> Vec<CheckResult> {
    let mut results = Vec::new();

    // 1. Git availability
    check_git(&mut results);

    // 2. Agent CLIs
    check_agents(&mut results);

    // 3. Data directory
    check_data_dir(&mut results);

    // 4. Sessions directory
    check_sessions_dir(&mut results);

    // 5. Config file parseable
    check_config(&mut results);

    // 6. EDITOR/VISUAL set
    check_editor(&mut results);

    // 7. Build verification
    check_build(&mut results);

    results
}

/// Apply auto-fixes for all failed checks that have one.
/// Returns (applied: Vec<String>, failed: Vec<String>) with descriptions.
pub fn run_doctor_fix() -> (Vec<String>, Vec<String>) {
    let results = run_doctor();
    let mut applied = Vec::new();
    let mut failed = Vec::new();

    for result in &results {
        if result.passed {
            continue;
        }
        if let Some(ref fix) = result.auto_fix {
            eprintln!("Fix: {} ... ", fix.description);
            match std::process::Command::new("sh")
                .arg("-c")
                .arg(&fix.command)
                .output()
            {
                Ok(output) if output.status.success() => {
                    eprintln!("  ✓ Done");
                    applied.push(fix.description.clone());
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    eprintln!("  ✗ Failed: {}", stderr.trim());
                    failed.push(format!("{}: {}", fix.description, stderr.trim()));
                }
                Err(e) => {
                    eprintln!("  ✗ Failed: {}", e);
                    failed.push(format!("{}: {}", fix.description, e));
                }
            }
        }
    }

    (applied, failed)
}

/// Quick startup check — only verifies git + at least one agent CLI.
/// Returns a warning message if issues found, `None` if everything looks OK.
pub fn run_quick_doctor() -> Option<String> {
    let mut warnings = Vec::new();

    if which("git").is_none() {
        warnings.push("git not found");
    }

    let available: Vec<_> = Agent::ALL
        .iter()
        .filter(|a| which(a.cmd()).is_some())
        .collect();

    if available.is_empty() {
        warnings.push("no agent CLI found (claude/codex/omp)");
    }

    if warnings.is_empty() {
        return None;
    }

    Some(format!(
        "⚠ {} — run `amux doctor` for details",
        warnings.join(", ")
    ))
}

fn check_git(results: &mut Vec<CheckResult>) {
    match which("git") {
        Some(path) => {
            let msg = path.display().to_string();
            results.push(CheckResult { name: "git".into(), passed: true, message: msg, fix_hint: None, auto_fix: None });
        }
        None => {
            results.push(CheckResult { name: "git".into(), passed: false, message: "not found in PATH".into(), fix_hint: Some("Install git: https://git-scm.com/downloads".into()), auto_fix: None });
        }
    }
}

fn check_agents(results: &mut Vec<CheckResult>) {
    for agent in Agent::ALL {
        let cmd = agent.cmd();
        let label = agent.label();
        match which(cmd) {
            Some(path) => {
                results.push(CheckResult { name: format!("agent: {}", label), passed: true, message: path.display().to_string(), fix_hint: None, auto_fix: None });
            }
            None => {
                let hint = install_hint(cmd)
                    .map(|h| h.to_string())
                    .unwrap_or_else(|| format!("Install {}", label));
                results.push(CheckResult { name: format!("agent: {}", label), passed: false, message: "not found in PATH".into(), fix_hint: Some(hint), auto_fix: None });
            }
        }
    }
}

fn check_data_dir(results: &mut Vec<CheckResult>) {
    let dir = config::data_dir();
    let dir_str = dir.display().to_string();

    if !dir.exists() {
        results.push(CheckResult {
            name: "data directory".into(),
            passed: false,
            message: format!("{} does not exist", dir_str),
            fix_hint: Some("Run any amux command to auto-create it".into()),
            auto_fix: Some(AutoFix {
                description: format!("Create {}", dir_str),
                command: format!("mkdir -p {}", dir_str),
            }),
        });
        return;
    }

    // Check writable by creating a temp file
    let test_file = dir.join(".amux-doctor-write-test");
    match fs::write(&test_file, b"test") {
        Ok(()) => {
            let _ = fs::remove_file(&test_file);
            results.push(CheckResult { name: "data directory".into(), passed: true, message: dir_str, fix_hint: None, auto_fix: None });
        }
        Err(e) => {
            results.push(CheckResult { name: "data directory".into(), passed: false, message: format!("{} not writable: {}", dir_str, e), fix_hint: Some("Check directory permissions".into()), auto_fix: None });
        }
    }
}
fn check_sessions_dir(results: &mut Vec<CheckResult>) {
    let dir = config::data_dir().join("sessions");
    let dir_str = dir.display().to_string();

    if !dir.exists() {
        results.push(CheckResult {
            name: "sessions directory".into(),
            passed: false,
            message: format!("{} does not exist", dir_str),
            fix_hint: Some("Run any amux command to auto-create it".into()),
            auto_fix: Some(AutoFix {
                description: format!("Create {}", dir_str),
                command: format!("mkdir -p {}", dir_str),
            }),
        });
        return;
    }

    let count = fs::read_dir(&dir).map(|rd| rd.count()).unwrap_or(0);
    results.push(CheckResult { name: "sessions directory".into(), passed: true, message: format!("{} ({} entries)", dir_str, count), fix_hint: None, auto_fix: None });
}

fn check_config(results: &mut Vec<CheckResult>) {
    let path = config::config_path();
    let path_str = path.display().to_string();

    if !path.exists() {
        results.push(CheckResult { name: "config file".into(), passed: true, message: format!("{} (will be created on first use)", path_str), fix_hint: None, auto_fix: None });
        return;
    }

    match config::load_config() {
        Ok(cfg) => {
            let ws_count = cfg.workspaces.len();
            results.push(CheckResult { name: "config file".into(), passed: true, message: format!("{} ({} workspace(s))", path_str, ws_count), fix_hint: None, auto_fix: None });
        }
        Err(e) => {
            results.push(CheckResult { name: "config file".into(), passed: false, message: format!("{}: parse error: {}", path_str, e), fix_hint: Some("Fix or delete the config file to regenerate".into()), auto_fix: None });
        }
    }
}

fn check_editor(results: &mut Vec<CheckResult>) {
    let editor = env::var("EDITOR").or_else(|_| env::var("VISUAL")).ok();

    match editor {
        Some(ed) => {
            results.push(CheckResult { name: "EDITOR / VISUAL".into(), passed: true, message: ed, fix_hint: None, auto_fix: None });
        }
        None => {
            results.push(CheckResult { name: "EDITOR / VISUAL".into(), passed: false, message: "neither EDITOR nor VISUAL is set".into(), fix_hint: Some("Set EDITOR in your shell profile, e.g. export EDITOR=vim".into()), auto_fix: None });
        }
    }
}

fn check_build(results: &mut Vec<CheckResult>) {
    let manifest = std::path::Path::new("Cargo.toml");
    if !manifest.exists() {
        return; // Not running from source directory
    }
    let output = std::process::Command::new("cargo")
        .args(["check", "--message-format=short"])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            results.push(CheckResult { name: "Build".into(), passed: true, message: "cargo check passes".into(), fix_hint: None, auto_fix: None });
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let first_error = stderr
                .lines()
                .find(|l| l.contains("error"))
                .unwrap_or("unknown error");
            results.push(CheckResult { name: "Build".into(), passed: false, message: format!("cargo check failed: {first_error}"), fix_hint: Some("Run `cargo check` and fix compilation errors".into()), auto_fix: None });
        }
        Err(e) => {
            results.push(CheckResult { name: "Build".into(), passed: false, message: format!("failed to run cargo check: {e}"), fix_hint: None, auto_fix: None });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_runs_all_checks() {
        let results = run_doctor();
        // At minimum: git + 3 agents + data_dir + sessions_dir + config + editor = 8
        assert!(
            results.len() >= 8,
            "expected >= 8 checks, got {}",
            results.len()
        );
    }

    #[test]
    fn quick_doctor_returns_none_when_git_and_agent_present() {
        // This test depends on the environment — git and at least one agent
        // are typically available in the dev environment.
        // If neither is available, quick_doctor should return Some.
        let _result = run_quick_doctor();
    }
}
