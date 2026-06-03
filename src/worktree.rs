//! Git worktree isolation for conflicting sessions.
//!
//! When multiple agents modify the same files in a workspace, we create
//! git worktrees so each agent gets its own checkout. On completion,
//! the user can merge or discard the worktree changes.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Check whether `path` is inside a git repository.
pub fn is_git_repo(path: &Path) -> bool {
    std::process::Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check whether the `git` command is available on `$PATH`.
pub fn git_available() -> bool {
    std::process::Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Create a detached git worktree for a session, returning the worktree path.
///
/// The worktree is created as `<repo_path>/.amux-worktree-<branch_name>` with a
/// new orphan branch named `amux/<branch_name>`. The caller can then restart the
/// PTY in this new directory.
pub fn create_worktree(repo_path: &Path, branch_name: &str) -> Result<PathBuf> {
    let worktree_path = repo_path.join(format!(".amux-worktree-{}", branch_name));

    // Don't recreate if it already exists
    if worktree_path.exists() {
        return Ok(worktree_path);
    }

    // Create worktree at HEAD with a new branch
    let output = std::process::Command::new("git")
        .args([
            "worktree",
            "add",
            "--detach",
            worktree_path.to_str().unwrap_or("."),
            "HEAD",
        ])
        .current_dir(repo_path)
        .output()
        .context("failed to execute git worktree add")?;

    if !output.status.success() {
        anyhow::bail!(
            "git worktree add failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Create an orphan branch in the worktree so we can identify it
    let branch_output = std::process::Command::new("git")
        .args(["checkout", "-b", &format!("amux/{}", branch_name)])
        .current_dir(&worktree_path)
        .output()
        .context("failed to create worktree branch")?;

    if !branch_output.status.success() {
        // Branch creation failed — clean up the worktree
        let _ = std::process::Command::new("git")
            .args(["worktree", "remove", "--force"])
            .arg(worktree_path.to_str().unwrap_or("."))
            .current_dir(repo_path)
            .output();
        anyhow::bail!(
            "git checkout -b failed in worktree: {}",
            String::from_utf8_lossy(&branch_output.stderr)
        );
    }

    Ok(worktree_path)
}

/// Remove a worktree and its associated branch.
pub fn remove_worktree(repo_path: &Path, branch_name: &str) -> Result<()> {
    let worktree_path = repo_path.join(format!(".amux-worktree-{}", branch_name));

    if worktree_path.exists() {
        let output = std::process::Command::new("git")
            .args([
                "worktree",
                "remove",
                "--force",
                worktree_path.to_str().unwrap_or("."),
            ])
            .current_dir(repo_path)
            .output()
            .context("failed to execute git worktree remove")?;

        if !output.status.success() {
            // Best-effort: don't fail if worktree was already cleaned up
            eprintln!(
                "warning: git worktree remove failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    // Delete the branch regardless
    let _ = std::process::Command::new("git")
        .args(["branch", "-D", &format!("amux/{}", branch_name)])
        .current_dir(repo_path)
        .output();

    Ok(())
}

/// Merge a worktree branch back into the current HEAD of `repo_path` using
/// `git merge --squash`. The caller should stage and commit after this.
pub fn merge_worktree(repo_path: &Path, branch_name: &str) -> Result<()> {
    let branch_ref = format!("amux/{}", branch_name);

    let output = std::process::Command::new("git")
        .args(["merge", "--squash", &branch_ref])
        .current_dir(repo_path)
        .output()
        .context("failed to execute git merge --squash")?;

    if !output.status.success() {
        anyhow::bail!(
            "git merge --squash failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

/// Generate a unique branch name from a session title and pty index.
pub fn branch_name(title: &str, pty_idx: usize, counter: u64) -> String {
    // Sanitize title to be a valid git branch name component
    let sanitized: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    let short = if sanitized.len() > 24 {
        &sanitized[..24]
    } else if sanitized.is_empty() {
        "session"
    } else {
        &sanitized
    };

    format!("{}-{}-{}", short, pty_idx, counter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_name_sanitizes_special_chars() {
        let name = branch_name("my session/test (v2)", 0, 1);
        assert_eq!(name, "my-session-test--v2-0-1");
    }

    #[test]
    fn branch_name_truncates_long_title() {
        let long_title = "a".repeat(100);
        let name = branch_name(&long_title, 3, 42);
        assert!(name.starts_with('a'));
        assert!(name.len() < 50);
        assert!(name.ends_with("-3-42"));
    }

    #[test]
    fn branch_name_handles_empty_title() {
        let name = branch_name("", 0, 1);
        assert_eq!(name, "session-0-1");
    }
}
