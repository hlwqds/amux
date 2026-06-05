//! `amux attach` — attach to or create a tmux session running `amux tui`.

use std::os::unix::process::CommandExt;
use std::process::Command;

use anyhow::{bail, Context, Result};

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
    bail!("failed to exec tmux attach: {}", err);
}

/// Locate the `tmux` binary, printing a helpful message if missing.
fn which_tmux() -> Result<String> {
    let out = Command::new("tmux")
        .arg("-V")
        .output()
        .context(
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

    /// Verify `which_tmux` returns an error when the `tmux` binary is not on
    /// PATH.  We achieve this by temporarily pointing PATH at an empty temp
    /// directory so the OS cannot resolve `tmux`.
    #[test]
    fn which_tmux_fails_when_missing() {
        let dir = std::env::temp_dir().join("amux_test_no_tmux_bin");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let original_path = std::env::var("PATH").unwrap_or_default();
        // Point PATH at an empty directory — no tmux there.
        unsafe { std::env::set_var("PATH", &dir); }
        let result = which_tmux();
        // Restore before asserting so other tests aren't affected.
        unsafe { std::env::set_var("PATH", &original_path); }
        let _ = std::fs::remove_dir_all(&dir);

        assert!(result.is_err(), "expected error when tmux is missing");
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("tmux"),
            "error message should mention tmux: {msg}"
        );
    }

    /// Verify `run()` surfaces a clear error when tmux is unavailable.
    #[test]
    fn run_fails_gracefully_without_tmux() {
        let dir = std::env::temp_dir().join("amux_test_run_no_tmux");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let original_path = std::env::var("PATH").unwrap_or_default();
        unsafe { std::env::set_var("PATH", &dir); }
        let result = run();
        unsafe { std::env::set_var("PATH", &original_path); }
        let _ = std::fs::remove_dir_all(&dir);

        assert!(result.is_err(), "run() should fail when tmux is missing");
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("tmux"),
            "error should mention tmux: {msg}"
        );
    }
}
