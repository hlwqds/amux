//! Pre-flight checks run before starting a new session.
//!
//! Verifies workspace state (git status, branch, project config, compilation)
//! and produces a structured result for display in a confirmation popup.

use std::path::Path;

use crate::app::git_cmd;

/// Outcome of a single pre-flight check.
#[derive(Clone, Debug)]
pub enum CheckStatus {
    Pass(String),
    Warn(String),
    Fail(String),
}

/// Aggregated result of all pre-flight checks.
#[derive(Clone, Debug, Default)]
pub struct PreflightResult {
    pub checks: Vec<(String, CheckStatus)>,
    pub suggestions: Vec<String>,
}

impl PreflightResult {
    pub fn has_warnings(&self) -> bool {
        self.checks
            .iter()
            .any(|(_, s)| matches!(s, CheckStatus::Warn(_) | CheckStatus::Fail(_)))
    }
}

/// Run pre-flight checks against the given workspace directory.
pub fn run_preflight(workspace: &Path) -> PreflightResult {
    let mut result = PreflightResult::default();

    // 1. Git status — warn if working tree is dirty.
    match git_cmd(workspace, &["status", "--porcelain"]) {
        Ok(output) if output.is_empty() => {
            result.checks.push((
                "Git status".into(),
                CheckStatus::Pass("Working tree clean".into()),
            ));
        }
        Ok(output) => {
            let count = output.lines().count();
            result.checks.push((
                "Git status".into(),
                CheckStatus::Warn(format!("{} uncommitted file(s)", count)),
            ));
            result
                .suggestions
                .push("Commit or stash changes before starting".into());
        }
        Err(msg) => {
            result
                .checks
                .push(("Git status".into(), CheckStatus::Fail(msg)));
        }
    }

    // 2. Current branch — warn if on main/master.
    match git_cmd(workspace, &["branch", "--show-current"]) {
        Ok(branch) if branch == "main" || branch == "master" => {
            result.checks.push((
                "Branch".into(),
                CheckStatus::Warn(format!("On '{}' — consider a feature branch", branch)),
            ));
            result
                .suggestions
                .push("Create a feature branch for your work".into());
        }
        Ok(branch) if !branch.is_empty() => {
            result.checks.push((
                "Branch".into(),
                CheckStatus::Pass(format!("On '{}'", branch)),
            ));
        }
        Ok(_) => {
            // Empty output — possibly detached HEAD or no branches.
            result.checks.push((
                "Branch".into(),
                CheckStatus::Warn("Detached HEAD or no branch".into()),
            ));
        }
        Err(msg) => {
            result
                .checks
                .push(("Branch".into(), CheckStatus::Fail(msg)));
        }
    }

    // 3. .amux.json presence.
    let amux_json = workspace.join(".amux.json");
    if amux_json.exists() {
        result.checks.push((
            "Project config".into(),
            CheckStatus::Pass(".amux.json found".into()),
        ));
    } else {
        result.checks.push((
            "Project config".into(),
            CheckStatus::Pass("No .amux.json".into()),
        ));
    }

    // 4. Cargo check — skip if not a Rust project.
    let cargo_toml = workspace.join("Cargo.toml");
    if cargo_toml.exists() {
        let output = std::process::Command::new("cargo")
            .args(["check", "--message-format=short"])
            .current_dir(workspace)
            .output();

        match output {
            Ok(o) if o.status.success() => {
                result.checks.push((
                    "Compilation".into(),
                    CheckStatus::Pass("cargo check passed".into()),
                ));
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                // Extract first line of error for brevity.
                let first_err = stderr
                    .lines()
                    .find(|l| l.contains("error"))
                    .unwrap_or("compilation failed");
                result.checks.push((
                    "Compilation".into(),
                    CheckStatus::Fail(first_err.to_string()),
                ));
                result
                    .suggestions
                    .push("Fix compilation errors before starting".into());
            }
            Err(e) => {
                result.checks.push((
                    "Compilation".into(),
                    CheckStatus::Fail(format!("cargo check failed: {}", e)),
                ));
            }
        }
    }

    result
}
