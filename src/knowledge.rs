use std::path::{Path, PathBuf};

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Accumulated insights from completed sessions for a workspace.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct WorkspaceKnowledge {
    pub architecture: String,
    pub key_files: Vec<String>,
    pub tech_stack: Vec<String>,
    pub known_issues: Vec<String>,
    pub last_updated: Option<String>,
}

/// Returns `workspace/.amux/knowledge.json`.
pub fn knowledge_path(workspace: &Path) -> PathBuf {
    workspace.join(".amux").join("knowledge.json")
}

/// Load knowledge from file, returns default if missing or corrupt.
pub fn load_knowledge(workspace: &Path) -> WorkspaceKnowledge {
    let path = knowledge_path(workspace);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Create parent dir if needed and persist knowledge as JSON.
pub fn save_knowledge(workspace: &Path, knowledge: &WorkspaceKnowledge) -> Result<()> {
    let path = knowledge_path(workspace);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(knowledge)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Extract structured knowledge from raw text using regex patterns.
///
/// Returns `None` if nothing meaningful could be extracted.
/// Uses regex-based extraction for architecture descriptions, file paths,
/// technology names, and known issues — no external LLM calls needed.
pub fn extract_structured_knowledge(raw_text: &str) -> Option<WorkspaceKnowledge> {
    let mut knowledge = WorkspaceKnowledge::default();
    let mut found_any = false;

    // --- Architecture extraction ---
    // Patterns: "architecture is X", "system uses X pattern", "built with X"
    let arch_patterns: &[&str] = &[
        r"(?i)architecture\s+(?:is|follows?|uses?)\s+(.+)",
        r"(?i)(?:system|app|application)\s+uses?\s+(.+?)(?:\.|$)",
        r"(?i)built\s+with\s+(.+?)(?:\.|$)",
        r"(?i)(?:using|implements?)\s+(?:a\s+)?(.+?)\s+(?:pattern|architecture)",
        r"(?i)design(?:ed)?(?:\s+as|\s*:\s*)(.+?)(?:\.|$)",
    ];
    for pat in arch_patterns {
        if let Ok(re) = Regex::new(pat)
            && let Some(caps) = re.captures(raw_text)
            && let Some(m) = caps.get(1)
        {
            let arch = m.as_str().trim().to_string();
            if !arch.is_empty() && arch.len() < 500 {
                knowledge.architecture = arch;
                found_any = true;
                break;
            }
        }
    }

    // --- Key files extraction ---
    // Reuse looks_like_path via extract_paths_from_line
    for line in raw_text.lines() {
        extract_paths_from_line(&mut knowledge.key_files, line);
    }
    // Also extract paths that appear inline in prose: backtick-quoted or quoted paths
    if let Ok(re) = Regex::new(r"`([^`]+)`") {
        for caps in re.captures_iter(raw_text) {
            if let Some(m) = caps.get(1) {
                let candidate = m.as_str();
                if looks_like_path(candidate)
                    && !knowledge.key_files.contains(&candidate.to_string())
                {
                    knowledge.key_files.push(candidate.to_string());
                }
            }
        }
    }
    if !knowledge.key_files.is_empty() {
        found_any = true;
    }

    // --- Tech stack extraction ---
    static TECH_NAMES: &[&str] = &[
        "Rust",
        "Python",
        "TypeScript",
        "JavaScript",
        "Go",
        "Java",
        "Ruby",
        "React",
        "Svelte",
        "Vue",
        "Angular",
        "Next.js",
        "NextJS",
        "Tokio",
        "Axum",
        "Actix",
        "Warp",
        "Hyper",
        "Serde",
        "Clap",
        "Anyhow",
        "Tracing",
        "Django",
        "Flask",
        "FastAPI",
        "Express",
        "Koa",
        "PostgreSQL",
        "MySQL",
        "SQLite",
        "Redis",
        "MongoDB",
        "Docker",
        "Kubernetes",
        "Terraform",
        "C++",
        "C#",
        "Swift",
        "Kotlin",
        "Scala",
        "Haskell",
        "Elixir",
        "Clojure",
        "Zig",
        "Nim",
        "Lua",
        "Webpack",
        "Vite",
        "Rollup",
        "esbuild",
        "Tailwind",
        "Bootstrap",
        "SASS",
        "CSS",
    ];
    for &tech in TECH_NAMES {
        // Case-insensitive word-boundary match
        let pat = format!(r"(?i)\b{}\b", regex::escape(tech));
        if let Ok(re) = Regex::new(&pat)
            && re.is_match(raw_text)
        {
            let lower_tech = tech.to_lowercase();
            if !knowledge
                .tech_stack
                .iter()
                .any(|t| t.eq_ignore_ascii_case(&lower_tech))
            {
                knowledge.tech_stack.push(lower_tech);
                found_any = true;
            }
        }
    }

    // --- Known issues extraction ---
    // Lines containing bug/issue/problem/error/TODO/FIXME markers
    let issue_re =
        Regex::new(r"(?i)\b(?:bug|issue|problem|error|todo|fixme|hack|workaround|known\s+issue)\b")
            .ok();
    if let Some(re) = issue_re {
        for line in raw_text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.len() > 500 {
                continue;
            }
            if re.is_match(trimmed) && !knowledge.known_issues.contains(&trimmed.to_string()) {
                knowledge.known_issues.push(trimmed.to_string());
                found_any = true;
            }
        }
    }

    // Also extract TODO/FIXME comments with context
    if let Ok(re) = Regex::new(r"(?i)(TODO|FIXME|HACK|XXX)\s*[:\(]?\s*(.{1,200})") {
        for caps in re.captures_iter(raw_text) {
            let full = caps
                .get(0)
                .expect("regex match always has group 0")
                .as_str()
                .trim()
                .to_string();
            if !knowledge.known_issues.contains(&full) {
                knowledge.known_issues.push(full);
                found_any = true;
            }
        }
    }

    if found_any {
        knowledge.last_updated = Some(chrono_now());
        Some(knowledge)
    } else {
        None
    }
}

/// Merge information extracted from a session summary into the knowledge base.
///
/// Uses simple heuristics — no LLM calls:
/// - Extract file paths mentioned (common path patterns)
/// - Extract technology names
/// - Append "issue"/"bug" lines to known_issues
/// - Update architecture if summary contains structural descriptions
/// - Dedup key_files, tech_stack, known_issues
pub fn merge_from_session(knowledge: &mut WorkspaceKnowledge, summary: &str) {
    let ts = chrono_now();
    knowledge.last_updated = Some(ts);

    // Extract file paths: look for src/... .rs, .ts, .py etc. patterns
    for line in summary.lines() {
        extract_paths_from_line(&mut knowledge.key_files, line);
    }

    // Extract tech names
    static TECH_NAMES: &[&str] = &[
        "rust",
        "typescript",
        "javascript",
        "python",
        "go",
        "java",
        "ruby",
        "react",
        "svelte",
        "vue",
        "angular",
        "nextjs",
        "next.js",
        "tokio",
        "axum",
        "actix",
        "warp",
        "hyper",
        "serde",
        "clap",
        "anyhow",
        "tracing",
        "django",
        "flask",
        "fastapi",
        "express",
        "koa",
        "postgres",
        "mysql",
        "sqlite",
        "redis",
        "mongodb",
        "docker",
        "kubernetes",
        "terraform",
    ];
    let lower = summary.to_lowercase();
    for &tech in TECH_NAMES {
        if lower.contains(tech)
            && !knowledge
                .tech_stack
                .iter()
                .any(|t| t.eq_ignore_ascii_case(tech))
        {
            knowledge.tech_stack.push(tech.to_string());
        }
    }

    // Extract issue/bug lines
    for line in summary.lines() {
        let lower = line.to_lowercase();
        let is_issue = lower.contains("issue:")
            || lower.contains("bug:")
            || lower.contains("fixme")
            || lower.contains("todo:")
            || (lower.contains("bug") && (lower.contains("found") || lower.contains("fix")))
            || lower.contains("error:")
            || lower.contains("warning:")
            || lower.contains("known issue");
        if is_issue {
            let trimmed = line.trim().to_string();
            if !trimmed.is_empty() && !knowledge.known_issues.contains(&trimmed) {
                knowledge.known_issues.push(trimmed);
            }
        }
    }

    // Fallback: if the keyword-based extraction above didn't find enough,
    // use regex-based extract_structured_knowledge as a richer fallback.
    let needs_fallback = knowledge.architecture.is_empty()
        && knowledge.key_files.is_empty()
        && knowledge.tech_stack.is_empty()
        && knowledge.known_issues.is_empty();
    if needs_fallback && let Some(extracted) = extract_structured_knowledge(summary) {
        if knowledge.architecture.is_empty() && !extracted.architecture.is_empty() {
            knowledge.architecture = extracted.architecture;
        }
        for f in extracted.key_files {
            if !knowledge.key_files.contains(&f) {
                knowledge.key_files.push(f);
            }
        }
        for t in extracted.tech_stack {
            if !knowledge
                .tech_stack
                .iter()
                .any(|e| e.eq_ignore_ascii_case(&t))
            {
                knowledge.tech_stack.push(t);
            }
        }
        for i in extracted.known_issues {
            if !knowledge.known_issues.contains(&i) {
                knowledge.known_issues.push(i);
            }
        }
    }
}

/// Format knowledge into a prompt string for injection into a new session.
pub fn generate_injection_prompt(knowledge: &WorkspaceKnowledge) -> String {
    if knowledge.architecture.is_empty()
        && knowledge.key_files.is_empty()
        && knowledge.tech_stack.is_empty()
        && knowledge.known_issues.is_empty()
    {
        return String::new();
    }

    let mut parts = Vec::new();
    parts.push("[Knowledge Base]".to_string());

    if !knowledge.architecture.is_empty() {
        parts.push(format!("Architecture: {}", knowledge.architecture));
    }
    if !knowledge.key_files.is_empty() {
        parts.push(format!("Key files: {}", knowledge.key_files.join(", ")));
    }
    if !knowledge.tech_stack.is_empty() {
        parts.push(format!("Tech stack: {}", knowledge.tech_stack.join(", ")));
    }
    if !knowledge.known_issues.is_empty() {
        parts.push(format!(
            "Known issues: {}",
            knowledge.known_issues.join("; ")
        ));
    }

    parts.join("\n")
}

/// Extract file paths from a single line of text.
fn extract_paths_from_line(paths: &mut Vec<String>, line: &str) {
    // Match patterns like src/foo.rs, lib/bar.ts, etc.
    for word in line.split_whitespace() {
        let cleaned = word.trim_matches(|c: char| {
            c == '`' || c == ',' || c == ';' || c == ':' || c == '(' || c == ')'
        });
        if looks_like_path(cleaned) && !paths.contains(&cleaned.to_string()) {
            paths.push(cleaned.to_string());
        }
    }
}

/// Heuristic: does this token look like a source file path?
fn looks_like_path(s: &str) -> bool {
    // Must contain at least one path separator or have a recognized extension
    let has_sep = s.contains('/') || s.contains('\\');
    let has_ext = s.ends_with(".rs")
        || s.ends_with(".ts")
        || s.ends_with(".tsx")
        || s.ends_with(".js")
        || s.ends_with(".jsx")
        || s.ends_with(".py")
        || s.ends_with(".go")
        || s.ends_with(".java")
        || s.ends_with(".toml")
        || s.ends_with(".yaml")
        || s.ends_with(".yml")
        || s.ends_with(".json")
        || s.ends_with(".md");
    // Must start with a path-like prefix or have extension
    let has_prefix = s.starts_with("src/")
        || s.starts_with("lib/")
        || s.starts_with("test")
        || s.starts_with("pkg/")
        || s.starts_with("cmd/")
        || s.starts_with("internal/")
        || s.starts_with("configs/")
        || s.starts_with("scripts/");
    (has_sep || has_prefix) && has_ext
}

fn chrono_now() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    // Simple ISO-ish timestamp without chrono dependency
    let secs = dur.as_secs();
    let days = secs / 86400;
    // Days since epoch to Y-M-D (simplified, good enough for timestamps)
    let (year, month, day) = epoch_days_to_ymd(days);
    let hour = (secs % 86400) / 3600;
    let min = (secs % 3600) / 60;
    format!("{:04}-{:02}-{:02}T{:02}:{:02}", year, month, day, hour, min)
}

/// Convert days since Unix epoch to (year, month, day).
fn epoch_days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let leap = is_leap(year);
    let month_days: [u64; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u64;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month, days + 1)
}

const fn is_leap(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn knowledge_path_returns_correct_location() {
        let path = knowledge_path(Path::new("/tmp/myproject"));
        assert_eq!(path, PathBuf::from("/tmp/myproject/.amux/knowledge.json"));
    }

    #[test]
    fn load_knowledge_returns_default_when_missing() {
        let dir = std::env::temp_dir().join("amux_test_knowledge_missing");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let k = load_knowledge(&dir);
        assert!(k.architecture.is_empty());
        assert!(k.key_files.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("amux_test_knowledge_rt");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let k = WorkspaceKnowledge {
            architecture: "Layered: handler -> service -> repo".into(),
            key_files: vec!["src/main.rs".into()],
            tech_stack: vec!["rust".into()],
            known_issues: vec!["bug: off-by-one in counter".into()],
            last_updated: Some("2026-01-01T00:00".into()),
        };
        save_knowledge(&dir, &k).unwrap();
        let loaded = load_knowledge(&dir);
        assert_eq!(loaded.architecture, "Layered: handler -> service -> repo");
        assert_eq!(loaded.key_files, vec!["src/main.rs"]);
        assert_eq!(loaded.tech_stack, vec!["rust"]);
        assert_eq!(loaded.known_issues, vec!["bug: off-by-one in counter"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn merge_extracts_paths() {
        let mut k = WorkspaceKnowledge::default();
        merge_from_session(&mut k, "Modified src/app/handler.rs and lib/types.ts");
        assert!(k.key_files.contains(&"src/app/handler.rs".to_string()));
        assert!(k.key_files.contains(&"lib/types.ts".to_string()));
    }

    #[test]
    fn merge_dedups_paths() {
        let mut k = WorkspaceKnowledge::default();
        merge_from_session(&mut k, "Changed src/main.rs");
        merge_from_session(&mut k, "Also changed src/main.rs again");
        let count = k.key_files.iter().filter(|f| **f == "src/main.rs").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn merge_extracts_tech() {
        let mut k = WorkspaceKnowledge::default();
        merge_from_session(&mut k, "Built with Rust and Tokio async runtime");
        assert!(k.tech_stack.contains(&"rust".to_string()));
        assert!(k.tech_stack.contains(&"tokio".to_string()));
    }

    #[test]
    fn merge_extracts_issues() {
        let mut k = WorkspaceKnowledge::default();
        merge_from_session(
            &mut k,
            "Found bug: race condition in writer\nIssue: memory leak in cache",
        );
        assert!(k.known_issues.iter().any(|i| i.contains("race condition")));
        assert!(k.known_issues.iter().any(|i| i.contains("memory leak")));
    }

    #[test]
    fn merge_extracts_architecture() {
        let mut k = WorkspaceKnowledge::default();
        merge_from_session(
            &mut k,
            "The architecture follows a layered pattern with middleware",
        );
        assert!(k.architecture.contains("layered pattern"));
    }

    #[test]
    fn generate_injection_empty_knowledge() {
        let k = WorkspaceKnowledge::default();
        assert!(generate_injection_prompt(&k).is_empty());
    }

    #[test]
    fn generate_injection_nonempty() {
        let k = WorkspaceKnowledge {
            architecture: "Modular design".into(),
            key_files: vec!["src/main.rs".into()],
            tech_stack: vec!["rust".into()],
            known_issues: vec!["bug: crash on null".into()],
            last_updated: None,
        };
        let prompt = generate_injection_prompt(&k);
        assert!(prompt.contains("[Knowledge Base]"));
        assert!(prompt.contains("src/main.rs"));
        assert!(prompt.contains("rust"));
        assert!(prompt.contains("bug: crash on null"));
    }

    #[test]
    fn looks_like_path_heuristic() {
        assert!(looks_like_path("src/main.rs"));
        assert!(looks_like_path("lib/types.ts"));
        assert!(looks_like_path("internal/server/handler.go"));
        assert!(!looks_like_path("hello"));
        assert!(!looks_like_path("main.rs")); // no path separator or prefix
    }

    #[test]
    fn epoch_days_conversion() {
        // 1970-01-01 = day 0
        let (y, m, d) = epoch_days_to_ymd(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn extract_structured_knowledge_none_on_empty() {
        assert!(extract_structured_knowledge("").is_none());
        assert!(extract_structured_knowledge("just some random text").is_none());
    }

    #[test]
    fn extract_structured_knowledge_architecture() {
        let result = extract_structured_knowledge(
            "The system uses a microservice architecture with event sourcing.",
        );
        assert!(result.is_some());
        let k = result.unwrap();
        assert!(
            k.architecture.contains("microservice"),
            "got: {}",
            k.architecture
        );
    }

    #[test]
    fn extract_structured_knowledge_built_with() {
        let result =
            extract_structured_knowledge("The app is built with Rust and uses Tokio for async.");
        assert!(result.is_some());
        let k = result.unwrap();
        assert!(k.tech_stack.contains(&"rust".to_string()));
        assert!(k.tech_stack.contains(&"tokio".to_string()));
    }

    #[test]
    fn extract_structured_knowledge_files_in_backticks() {
        let result = extract_structured_knowledge(
            "Modified `src/app/handler.rs` to fix the issue in `lib/types.ts`.",
        );
        assert!(result.is_some());
        let k = result.unwrap();
        assert!(k.key_files.contains(&"src/app/handler.rs".to_string()));
        assert!(k.key_files.contains(&"lib/types.ts".to_string()));
    }

    #[test]
    fn extract_structured_knowledge_issues() {
        let result = extract_structured_knowledge(
            "TODO: refactor the authentication module\nFIXME: memory leak in cache",
        );
        assert!(result.is_some());
        let k = result.unwrap();
        assert!(k.known_issues.iter().any(|i| i.contains("TODO")));
        assert!(k.known_issues.iter().any(|i| i.contains("FIXME")));
    }

    #[test]
    fn merge_fallback_uses_extract_structured() {
        let mut k = WorkspaceKnowledge::default();
        // "Swift" is NOT in merge_from_session's TECH_NAMES but IS in
        // extract_structured_knowledge's expanded list, so only the
        // regex fallback will find it.
        merge_from_session(&mut k, "The project is built with Swift and SwiftUI.");
        assert!(
            k.tech_stack.iter().any(|t| t.eq_ignore_ascii_case("swift")),
            "fallback should extract swift, got: {:?}",
            k.tech_stack
        );
    }
}
