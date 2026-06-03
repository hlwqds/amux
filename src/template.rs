use std::path::Path;

/// Expand template variables in a prompt string.
///
/// Supported variables:
/// - `{git_diff}` — `git diff --stat` output
/// - `{git_branch}` — current branch name
/// - `{files_changed}` — comma-separated list of changed files
/// - `{workspace_path}` — workspace directory path
/// - `{project_type}` — detected project type label (Rust/Node/Python/Go/Make/Unknown)
pub fn expand_template_vars(prompt: &str, workspace: &Path) -> String {
    let mut result = prompt.to_string();

    if result.contains("{git_diff}") {
        let diff = std::process::Command::new("git")
            .args(["diff", "--stat"])
            .current_dir(workspace)
            .output()
            .ok()
            .and_then(|o| {
                o.status
                    .success()
                    .then(|| String::from_utf8_lossy(&o.stdout).to_string())
            })
            .unwrap_or_else(|| "(no git diff available)".to_string());
        result = result.replace("{git_diff}", &diff);
    }

    if result.contains("{git_branch}") {
        let branch = std::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(workspace)
            .output()
            .ok()
            .and_then(|o| {
                o.status
                    .success()
                    .then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
            })
            .unwrap_or_else(|| "(unknown)".to_string());
        result = result.replace("{git_branch}", &branch);
    }

    if result.contains("{files_changed}") {
        let files = std::process::Command::new("git")
            .args(["diff", "--name-only"])
            .current_dir(workspace)
            .output()
            .ok()
            .and_then(|o| {
                o.status.success().then(|| {
                    String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .collect::<Vec<_>>()
                        .join(", ")
                })
            })
            .unwrap_or_else(|| "(no changes)".to_string());
        result = result.replace("{files_changed}", &files);
    }

    if result.contains("{workspace_path}") {
        result = result.replace("{workspace_path}", &workspace.to_string_lossy());
    }

    if result.contains("{project_type}") {
        let pt = crate::discovery::ProjectType::detect(workspace);
        let label = match pt {
            crate::discovery::ProjectType::Rust => "Rust",
            crate::discovery::ProjectType::Node => "Node",
            crate::discovery::ProjectType::Python => "Python",
            crate::discovery::ProjectType::Go => "Go",
            crate::discovery::ProjectType::Make => "Make",
            crate::discovery::ProjectType::Unknown => "Unknown",
        };
        result = result.replace("{project_type}", label);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn no_vars_unchanged() {
        let p = Path::new("/tmp");
        assert_eq!(expand_template_vars("hello world", p), "hello world");
    }

    #[test]
    fn workspace_path_expanded() {
        let p = Path::new("/home/user/project");
        let result = expand_template_vars("path={workspace_path}", p);
        assert_eq!(result, "path=/home/user/project");
    }

    #[test]
    fn project_type_expanded() {
        // This test runs from the repo root which has Cargo.toml → Rust
        let p = Path::new(".");
        let result = expand_template_vars("type={project_type}", p);
        assert!(result.contains("Rust"), "expected Rust, got: {result}");
    }

    #[test]
    fn git_branch_expanded() {
        let p = Path::new(".");
        let result = expand_template_vars("branch={git_branch}", p);
        // Should not still contain the literal
        assert!(
            !result.contains("{git_branch}"),
            "variable was not expanded: {result}"
        );
    }

    #[test]
    fn multiple_vars_expanded() {
        let p = Path::new(".");
        let result = expand_template_vars(
            "Review {git_diff} on {git_branch} ({project_type}) at {workspace_path}",
            p,
        );
        assert!(!result.contains("{git_diff}"), "git_diff not expanded");
        assert!(!result.contains("{git_branch}"), "git_branch not expanded");
        assert!(
            !result.contains("{project_type}"),
            "project_type not expanded"
        );
        assert!(
            !result.contains("{workspace_path}"),
            "workspace_path not expanded"
        );
    }

    #[test]
    fn nonexistent_dir_git_vars_graceful() {
        let p = Path::new("/nonexistent/path/that/does/not/exist");
        let result = expand_template_vars(
            "diff={git_diff} branch={git_branch} files={files_changed}",
            p,
        );
        assert!(result.contains("(no git diff available)"), "got: {result}");
        assert!(result.contains("(unknown)"), "got: {result}");
        assert!(result.contains("(no changes)"), "got: {result}");
    }

    #[test]
    fn workspace_path_with_unicode() {
        let p = PathBuf::from("/home/用户/项目");
        let result = expand_template_vars("{workspace_path}", &p);
        assert_eq!(result, "/home/用户/项目");
    }
}
