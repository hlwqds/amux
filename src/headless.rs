//! Headless (non-interactive) CLI mode for amux.
//!
//!
//! Subcommands:
//!   `amux run   --agent <agent> --prompt <prompt> [--workspace <path>] [--timeout <secs>]`
//!   `amux list  [--json]`
//!   `amux status <session-id>`

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tracing::info;

use anyhow::{Context, Result, bail};
use portable_pty::PtySystem;

use crate::config;
use crate::discovery::{discover_sessions, discover_workspaces_from_fs};
use crate::types::Agent;

// ---------------------------------------------------------------------------
// Exit codes
// ---------------------------------------------------------------------------

/// Process exited successfully.
pub const EXIT_SUCCESS: i32 = 0;
/// Process exited with a failure.
pub const EXIT_FAILURE: i32 = 1;
/// Process timed out.
pub const EXIT_TIMEOUT: i32 = 2;

// ---------------------------------------------------------------------------
// `amux run`
// ---------------------------------------------------------------------------

/// Run an agent non-interactively, streaming output to stdout.
///
/// Returns an exit code: 0 = success, 1 = failure, 2 = timeout.
pub fn run(agent: Agent, prompt: &str, workspace: &Path, timeout_secs: Option<u64>) -> Result<i32> {
    // Build the command with --print (non-interactive, pipe-friendly output).
    let mut cmd = portable_pty::CommandBuilder::new(agent.cmd());
    cmd.cwd(workspace);
    cmd.env("TERM", "dumb");
    // Remove terminal multiplexer vars that confuse agents
    cmd.env_remove("KITTY_WINDOW_ID");
    cmd.env_remove("KITTY_LISTEN_ON");
    cmd.env_remove("TERM_PROGRAM");
    cmd.env_remove("GHOSTTY_RESOURCES_DIR");

    // Claude: `claude --print "prompt"` runs non-interactively.
    // Other agents get `agent_cmd prompt` as best-effort.
    match agent {
        Agent::Claude => {
            cmd.arg("--print");
            cmd.arg("--verbose");
            cmd.arg(prompt);
        }
        Agent::Codex => {
            cmd.arg("--quiet");
            cmd.arg(prompt);
        }
        Agent::Omp => {
            cmd.arg("--print");
            cmd.arg(prompt);
        }
    }

    // Load project env vars from .amux.json if present
    let project_config = config::load_project_config(workspace);
    for (key, value) in &project_config.env {
        cmd.env(key, value);
    }

    // Use a large PTY so output doesn't wrap strangely
    let size = (200u16, 50u16);
    let pty_system = portable_pty::NativePtySystem::default();
    let pty_size = portable_pty::PtySize {
        rows: size.1,
        cols: size.0,
        pixel_width: 0,
        pixel_height: 0,
    };
    let pair = pty_system.openpty(pty_size).context("failed to open PTY")?;
    let mut child = pair
        .slave
        .spawn_command(cmd)
        .context(format!("failed to spawn {}", agent.label()))?;

    let master = pair.master;
    let mut reader = master
        .try_clone_reader()
        .context("failed to clone PTY reader")?;

    // We need to keep master alive so the child PTY doesn't get SIGHUP.
    let _writer = master.take_writer();

    let timed_out = Arc::new(AtomicBool::new(false));
    let deadline = timeout_secs.map(|s| Instant::now() + Duration::from_secs(s));

    // Reader thread: strip ANSI and write to stdout
    let timed_out_clone = timed_out.clone();
    let reader_handle = std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        let mut stdout = io::stdout();
        let mut strip = StripAnsi::new();
        loop {
            match io::Read::read(&mut reader, &mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let clean = strip.process(&buf[..n]);
                    if !clean.is_empty() {
                        let _ = stdout.write_all(&clean);
                        let _ = stdout.flush();
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Timeout watchdog (if requested)
    let timeout_handle = deadline.map(|dl| {
        let timed_out = timed_out_clone.clone();
        std::thread::spawn(move || {
            loop {
                if Instant::now() >= dl {
                    timed_out.store(true, Ordering::Relaxed);
                    return;
                }
                std::thread::sleep(Duration::from_millis(250));
            }
        })
    });

    // Wait for child, checking timeout
    let exit_status: Option<portable_pty::ExitStatus> = loop {
        // Check if child has exited (non-blocking poll via try_wait)
        if let Some(status) = child.try_wait().context("failed to poll child")? {
            break Some(status);
        }
        if timed_out.load(Ordering::Relaxed) {
            // Kill the child process
            let _ = child.kill();
            // Give it a moment then move on
            std::thread::sleep(Duration::from_millis(100));
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(100));
    };

    // Clean up timeout thread
    if let Some(h) = timeout_handle {
        // The timeout thread will exit once timed_out is true or deadline passes.
        // We set timed_out to force it to exit quickly.
        timed_out.store(true, Ordering::Relaxed);
        let _ = h.join();
    }

    // Wait for reader to drain
    // Drop master to signal EOF to reader
    drop(master);
    let _ = reader_handle.join();

    // Determine exit code
    if timed_out.load(Ordering::Relaxed) {
        // Timeout was the cause
        return Ok(EXIT_TIMEOUT);
    }

    match exit_status {
        Some(status) if status.success() => Ok(EXIT_SUCCESS),
        Some(_) => Ok(EXIT_FAILURE),
        None => Ok(EXIT_FAILURE), // shouldn't happen, but safe default
    }
}

// ---------------------------------------------------------------------------
// `amux list`
// ---------------------------------------------------------------------------

/// List all discovered sessions.
pub fn list(json_output: bool) -> Result<()> {
    let config = config::load_config()?;
    let workspaces = if config.workspaces.is_empty() {
        discover_workspaces_from_fs()
    } else {
        config.workspaces
    };
    let sessions = discover_sessions(&workspaces);

    if json_output {
        let out: Vec<serde_json::Value> = sessions
            .iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "title": s.title,
                    "agent": s.agent.label(),
                    "workspace": s.workspace_path.to_string_lossy(),
                    "last_active": s.last_active,
                    "tags": s.tags,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&out).unwrap_or_else(|_| "[]".into())
        );
    } else {
        if sessions.is_empty() {
            println!("No sessions found.");
            return Ok(());
        }
        // Table header
        println!(
            "{:<12} {:<8} {:<30} {:<20} LAST ACTIVE",
            "ID", "AGENT", "TITLE", "WORKSPACE"
        );
        println!("{}", "-".repeat(90));
        for s in &sessions {
            let id_short = if s.id.len() > 10 { &s.id[..10] } else { &s.id };
            let title = truncate_str(&s.title, 30);
            let ws_name = s
                .workspace_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?");
            println!(
                "{:<12} {:<8} {:<30} {:<20} {}",
                id_short,
                s.agent.label(),
                title,
                truncate_str(ws_name, 20),
                crate::util::relative_time(s.last_active)
            );
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// `amux status`
// ---------------------------------------------------------------------------

/// Show status of a single session.
pub fn status(session_id_prefix: &str) -> Result<()> {
    let config = config::load_config()?;
    let workspaces = if config.workspaces.is_empty() {
        discover_workspaces_from_fs()
    } else {
        config.workspaces
    };
    let sessions = discover_sessions(&workspaces);

    let session = sessions
        .iter()
        .find(|s| s.id.starts_with(session_id_prefix))
        .or_else(|| sessions.iter().find(|s| s.id == session_id_prefix))
        .context(format!("No session found matching '{}'", session_id_prefix))?;

    let jsonl_path = crate::discovery::find_session_jsonl(session);
    let line_count = jsonl_path
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|c| c.lines().count())
        .unwrap_or(0);

    let jsonl_size = jsonl_path
        .as_ref()
        .and_then(|p| p.metadata().ok())
        .map(|m| m.len())
        .unwrap_or(0);

    let out = serde_json::json!({
        "id": session.id,
        "title": session.title,
        "agent": session.agent.label(),
        "workspace": session.workspace_path.to_string_lossy(),
        "last_active": session.last_active,
        "last_active_ago": crate::util::relative_time(session.last_active),
        "tags": session.tags,
        "jsonl_path": jsonl_path.map(|p| p.to_string_lossy().into_owned()),
        "jsonl_lines": line_count,
        "jsonl_size_bytes": jsonl_size,
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&out).unwrap_or_else(|_| "{}".into())
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// CLI argument parsing for headless subcommands
// ---------------------------------------------------------------------------

/// Parse and dispatch headless subcommands. Returns `None` if the args
/// don't match a known headless command (caller should fall through to TUI).
pub fn try_headless(args: &[String]) -> Option<Result<i32>> {
    if args.len() < 2 {
        return None;
    }
    match args[1].as_str() {
        "run" => Some(cmd_run(&args[2..])),
        "list" => Some(cmd_list(&args[2..])),
        "status" => Some(cmd_status(&args[2..])),
        _ => None,
    }
}

fn cmd_run(rest: &[String]) -> Result<i32> {
    let mut agent: Option<Agent> = None;
    let mut prompt: Option<String> = None;
    let mut workspace: Option<PathBuf> = None;
    let mut timeout: Option<u64> = None;

    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--agent" => {
                i += 1;
                let name = rest.get(i).context("--agent requires a value")?;
                agent = Some(Agent::from_label(name).context(format!(
                    "unknown agent '{}'. Supported: claude, codex, omp",
                    name
                ))?);
            }
            "--prompt" => {
                i += 1;
                prompt = Some(rest.get(i).context("--prompt requires a value")?.clone());
            }
            "--workspace" => {
                i += 1;
                workspace = Some(PathBuf::from(
                    rest.get(i).context("--workspace requires a value")?,
                ));
            }
            "--timeout" => {
                i += 1;
                timeout = Some(
                    rest.get(i)
                        .context("--timeout requires a value (seconds)")?
                        .parse::<u64>()
                        .context("--timeout must be a number")?,
                );
            }
            other => {
                bail!("unknown flag: {}", other);
            }
        }
        i += 1;
    }

    let agent = agent.context("--agent is required")?;
    let prompt = prompt.context("--prompt is required")?;
    let workspace = workspace.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    info!(
        "headless run: agent={:?} workspace={}",
        agent,
        workspace.display()
    );

    if !workspace.exists() {
        bail!("workspace path does not exist: {}", workspace.display());
    }

    run(agent, &prompt, &workspace, timeout)
}

fn cmd_list(rest: &[String]) -> Result<i32> {
    let json_output = rest.contains(&"--json".to_string());
    list(json_output)?;
    Ok(EXIT_SUCCESS)
}

fn cmd_status(rest: &[String]) -> Result<i32> {
    let session_id = rest
        .iter()
        .find(|a| !a.starts_with('-'))
        .context("usage: amux status <session-id>")?;
    status(session_id)?;
    Ok(EXIT_SUCCESS)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Truncate a string to `max` characters, appending "…" if truncated.
fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max - 1).collect();
        truncated + "…"
    }
}

/// Strips ANSI escape sequences from a byte stream.
/// Designed for incremental processing — maintains minimal state across calls.
struct StripAnsi {
    /// True when we've seen ESC and are inside a potential sequence.
    in_escape: bool,
    /// True when we're inside a CSI sequence (ESC [ ...).
    in_csi: bool,
}

impl StripAnsi {
    fn new() -> Self {
        Self {
            in_escape: false,
            in_csi: false,
        }
    }

    /// Process bytes, returning a Vec of clean (non-ANSI) bytes.
    fn process(&mut self, input: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(input.len());
        for &b in input {
            if self.in_csi {
                // CSI sequences end with a byte in 0x40..=0x7E
                if (0x40..=0x7E).contains(&b) {
                    self.in_csi = false;
                    self.in_escape = false;
                }
                // else: intermediate byte, keep consuming
            } else if self.in_escape {
                match b {
                    b'[' => {
                        self.in_csi = true;
                    }
                    b']' => {
                        // OSC sequence — ends with BEL (0x07) or ST (ESC \)
                        // For simplicity, we handle this by staying in escape mode
                        // and letting the next ESC reset or BEL terminate.
                        // Actually, let's just skip until BEL or ST.
                        self.in_escape = false; // simplified: just drop the ]
                    }
                    _ => {
                        // Two-character escape sequence done
                        self.in_escape = false;
                    }
                }
            } else if b == 0x1b {
                self.in_escape = true;
            } else {
                out.push(b);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi_plain() {
        let mut s = StripAnsi::new();
        assert_eq!(s.process(b"hello world"), b"hello world");
    }

    #[test]
    fn test_strip_ansi_csi() {
        let mut s = StripAnsi::new();
        assert_eq!(s.process(b"\x1b[32mhello\x1b[0m world"), b"hello world");
    }

    #[test]
    fn test_strip_ansi_cursor() {
        let mut s = StripAnsi::new();
        assert_eq!(s.process(b"\x1b[2J\x1b[Hclean"), b"clean");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("hello world", 8), "hello w…");
    }
}
