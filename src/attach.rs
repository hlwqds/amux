//! `amux attach` — attach to or create a tmux session running `amux tui`.

use std::os::unix::process::CommandExt;
use std::process::Command;

use anyhow::{Context, Result, bail};

const TMUX_SOCKET: &str = "amux";
const SESSION_NAME: &str = "amux";

/// Run `amux attach`.
///
/// 1. Verify tmux is installed.
/// 2. If a session named "amux" already exists on socket `-L amux`, attach to it.
/// 3. Otherwise create a detached session running `amux tui` and attach.
pub fn run() -> Result<()> {
    // 1. tmux must be available.
    let tmux = which_tmux()?;

    // 2. Check whether a session named "amux" already exists.
    let session_exists = Command::new(&tmux)
        .args(["-L", TMUX_SOCKET, "has-session", "-t", SESSION_NAME])
        .output()
        .context("failed to run `tmux has-session`")?
        .status
        .success();

    if !session_exists {
        // 3. Create a new detached session: tmux -L amux new-session -d 'amux tui'
        let status = Command::new(&tmux)
            .args([
                "-L",
                TMUX_SOCKET,
                "new-session",
                "-d",
                "-s",
                SESSION_NAME,
                "amux tui",
            ])
            .status()
            .context("failed to run `tmux new-session`")?;
        if !status.success() {
            bail!("`tmux new-session` failed (exit {:?})", status.code());
        }
    }

    // 4. Attach to the session (replaces current process).
    let err = Command::new(&tmux)
        .args(["-L", TMUX_SOCKET, "attach-session", "-t", SESSION_NAME])
        .exec();

    // exec only returns on error.
    bail!("failed to exec tmux attach: {err}");
}

/// Locate the `tmux` binary, printing a helpful message if missing.
fn which_tmux() -> Result<String> {
    let out = Command::new("tmux").arg("-V").output().context(
        "`tmux` not found. Install it with your package manager \
             (e.g. apt install tmux / brew install tmux).",
    )?;

    if !out.status.success() {
        bail!("`tmux -V` returned an error — is tmux installed correctly?");
    }

    Ok("tmux".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// If the test host has tmux installed, which_tmux should succeed.
    /// If not installed, the test is a no-op (can't test negative path
    /// without mutating PATH, which is UB in parallel tests).
    #[test]
    fn which_tmux_returns_ok_when_installed() {
        if crate::util::which("tmux").is_some() {
            assert!(which_tmux().is_ok());
        }
    }

    /// Verify `run()` surfaces a clear error when tmux is unavailable.
    /// Skip when tmux IS installed — we can only test the negative path
    /// on systems without tmux.
    #[test]
    fn run_fails_without_tmux() {
        if crate::util::which("tmux").is_some() {
            return;
        }
        let result = run();
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("tmux"),
            "error message should mention tmux: {msg}"
        );
    }
}
