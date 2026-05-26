use std::{
    env, fs, io,
    io::{IsTerminal, Write as _},
    path::{Path, PathBuf},
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::Sender,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use bytes::Bytes;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use serde::{Deserialize, Serialize};
use tui_term::widget::PseudoTerminal;

// ─── Data types ────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Agent {
    Claude,
    Codex,
}

impl Agent {
    fn cmd(&self) -> &str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
        }
    }

    fn label(&self) -> &str {
        match self {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex",
        }
    }

    fn icon(&self) -> &str {
        match self {
            Agent::Claude => "C",
            Agent::Codex => "X",
        }
    }

    fn color(&self) -> Color {
        match self {
            Agent::Claude => Color::Cyan,
            Agent::Codex => Color::Green,
        }
    }

    fn build_new_cmd(&self, workspace_path: &Path, session_name: Option<&str>) -> CommandBuilder {
        match self {
            Agent::Claude => {
                let mut cmd = CommandBuilder::new("claude");
                cmd.cwd(workspace_path);
                cmd.env("TERM", "xterm-256color");
                cmd.env_remove("KITTY_WINDOW_ID");
                cmd.env_remove("KITTY_LISTEN_ON");
                cmd.env_remove("TERM_PROGRAM");
                cmd.env_remove("GHOSTTY_RESOURCES_DIR");
                if let Some(name) = session_name {
                    cmd.arg("-n");
                    cmd.arg(name);
                }
                cmd
            }
            Agent::Codex => {
                let mut cmd = CommandBuilder::new("codex");
                cmd.cwd(workspace_path);
                cmd.env("TERM", "xterm-256color");
                cmd.env_remove("KITTY_WINDOW_ID");
                cmd.env_remove("KITTY_LISTEN_ON");
                cmd.env_remove("TERM_PROGRAM");
                cmd.env_remove("GHOSTTY_RESOURCES_DIR");
                // Codex doesn't support -n; pass prompt directly if name given
                if let Some(name) = session_name {
                    cmd.arg(name);
                }
                cmd
            }
        }
    }

    fn build_resume_cmd(&self, workspace_path: &Path, session_id: &str) -> CommandBuilder {
        match self {
            Agent::Claude => {
                let mut cmd = CommandBuilder::new("claude");
                cmd.cwd(workspace_path);
                cmd.env("TERM", "xterm-256color");
                cmd.env_remove("KITTY_WINDOW_ID");
                cmd.env_remove("KITTY_LISTEN_ON");
                cmd.env_remove("TERM_PROGRAM");
                cmd.env_remove("GHOSTTY_RESOURCES_DIR");
                cmd.arg("--resume");
                cmd.arg(session_id);
                cmd
            }
            Agent::Codex => {
                let mut cmd = CommandBuilder::new("codex");
                cmd.cwd(workspace_path);
                cmd.env("TERM", "xterm-256color");
                cmd.env_remove("KITTY_WINDOW_ID");
                cmd.env_remove("KITTY_LISTEN_ON");
                cmd.env_remove("TERM_PROGRAM");
                cmd.env_remove("GHOSTTY_RESOURCES_DIR");
                cmd.arg("resume");
                cmd.arg(session_id);
                cmd
            }
        }
    }

    fn sessions_dir(&self) -> Option<PathBuf> {
        match self {
            Agent::Claude => {
                let dir = PathBuf::from(env::var("HOME").unwrap_or_default())
                    .join(".claude/projects");
                if dir.exists() { Some(dir) } else { None }
            }
            Agent::Codex => {
                let dir = PathBuf::from(env::var("HOME").unwrap_or_default())
                    .join(".codex/sessions");
                if dir.exists() { Some(dir) } else { None }
            }
        }
    }

}

fn detect_agents() -> Vec<Agent> {
    let mut agents = Vec::new();
    if which("claude").is_some() {
        agents.push(Agent::Claude);
    }
    if which("codex").is_some() {
        agents.push(Agent::Codex);
    }
    agents
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Workspace {
    id: String,
    name: String,
    path: Option<PathBuf>,
    created_at: u64,
    #[serde(skip)]
    expanded: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Config {
    workspaces: Vec<Workspace>,
}

#[derive(Clone, Debug)]
struct Session {
    id: String,
    workspace_path: PathBuf,
    title: String,
    last_active: u64,
    agent: Agent,
}

#[derive(Clone, Debug)]
enum TreeNode {
    Workspace(usize),
    Session(usize, usize),
    ActiveTab(usize), // index into App.ptys
}

struct PtySlot {
    handle: PtyHandle,
    info: RunningInfo,
}

#[derive(Clone, Debug)]
struct RunningInfo {
    workspace_path: PathBuf,
    title: String,
    session_id: Option<String>,
    started_at: u64,
    completed: bool,
    agent: Agent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
    Sidebar,
    Chat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InputMode {
    None,
    SessionName,
    SelectAgent,
    RenameSession,
    RenameWorkspace,
    NewWorkspaceName,
    BrowseDir,
}

#[derive(Clone, Debug)]
struct DirEntry {
    name: String,
    path: PathBuf,
    is_dir: bool,
}

const SELECT_CURRENT: &str = "\u{2713} Select this directory";
const SELECT_VIRTUAL: &str = "\u{25cb} Virtual (no directory)";
const PARENT_DIR: &str = "..";

enum Action {
    Continue,
    Quit,
}

#[derive(Deserialize)]
struct ClaudeRecord {
    #[serde(rename = "type")]
    record_type: Option<String>,
    message: Option<ClaudeMessage>,
}

#[derive(Deserialize)]
struct ClaudeMessage {
    role: Option<String>,
    content: Option<serde_json::Value>,
}

// ─── PTY with vt100 parser ────────────────────────────────

struct PtyHandle {
    parser: Arc<RwLock<vt100::Parser>>,
    writer_tx: Sender<Bytes>,
    alive: Arc<AtomicBool>,
    last_output_at: Arc<AtomicU64>,
}

/// How long with no PTY output before we consider Claude "completed" (idle).
const IDLE_THRESHOLD_SECS: u64 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PtyState {
    Running,   // Claude is generating output
    Completed, // Claude finished, idle waiting for input
}

impl PtyHandle {
    fn state(&self) -> PtyState {
        let last = self.last_output_at.load(Ordering::Relaxed);
        let now = now_secs();
        if self.alive.load(Ordering::Relaxed) && now.saturating_sub(last) <= IDLE_THRESHOLD_SECS {
            PtyState::Running
        } else {
            PtyState::Completed
        }
    }

    fn spawn(
        agent: Agent,
        workspace_path: &Path,
        session_id: Option<&str>,
        session_name: Option<&str>,
        size: (u16, u16),
    ) -> Result<Self> {
        let pty_system = NativePtySystem::default();

        let pty_size = PtySize {
            rows: size.1,
            cols: size.0,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system.openpty(pty_size).context("failed to open PTY")?;

        let cmd = if let Some(id) = session_id {
            agent.build_resume_cmd(workspace_path, id)
        } else {
            agent.build_new_cmd(workspace_path, session_name)
        };

        let mut child = pair
            .slave
            .spawn_command(cmd)
            .context(format!("failed to spawn {}", agent.label()))?;

        let master = pair.master;
        let mut reader = master
            .try_clone_reader()
            .context("failed to clone PTY reader")?;

        let parser = Arc::new(RwLock::new(vt100::Parser::new(size.1, size.0, 0)));
        let alive = Arc::new(AtomicBool::new(true));
        let last_output_at = Arc::new(AtomicU64::new(now_secs()));

        {
            let parser = parser.clone();
            let last_output_at = last_output_at.clone();
            thread::spawn(move || {
                let mut buf = [0u8; 8192];
                loop {
                    match io::Read::read(&mut reader, &mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let mut p = parser.write().unwrap();
                            p.process(&buf[..n]);
                            last_output_at.store(now_secs(), Ordering::Relaxed);
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        {
            let alive = alive.clone();
            thread::spawn(move || {
                let _ = child.wait();
                alive.store(false, Ordering::Relaxed);
            });
        }

        let (writer_tx, writer_rx) = std::sync::mpsc::channel::<Bytes>();
        {
            thread::spawn(move || {
                let mut writer = master.take_writer().unwrap();
                while let Ok(bytes) = writer_rx.recv() {
                    if writer.write_all(&bytes).is_err() {
                        break;
                    }
                }
            });
        }

        Ok(Self {
            parser,
            writer_tx,
            alive,
            last_output_at,
        })
    }

    fn write_input(&self, data: &[u8]) {
        let _ = self.writer_tx.send(Bytes::from(data.to_vec()));
    }

    fn resize(&self, size: (u16, u16)) {
        if let Ok(mut p) = self.parser.write() {
            p.screen_mut().set_size(size.1, size.0);
        }
    }

    fn screen(&self) -> Arc<RwLock<vt100::Parser>> {
        self.parser.clone()
    }
}

// ─── Workspace discovery ──────────────────────────────────

fn data_dir() -> PathBuf {
    let xdg = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var("HOME").unwrap_or_default()).join(".local/share"));
    xdg.join("agent-workspace-tui")
}

fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

fn ensure_data_dir() -> io::Result<()> {
    let dir = data_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    let sessions_dir = dir.join("sessions");
    if !sessions_dir.exists() {
        fs::create_dir_all(&sessions_dir)?;
    }
    Ok(())
}

fn load_config() -> Result<Config> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config {
            workspaces: Vec::new(),
        });
    }
    let content = fs::read_to_string(&path).context("failed to read config.json")?;
    let config: Config =
        serde_json::from_str(&content).context("failed to parse config.json")?;
    Ok(config)
}

fn save_config_file(config: &Config) -> Result<()> {
    ensure_data_dir().context("failed to create data directory")?;
    let path = config_path();
    let content = serde_json::to_string_pretty(config).context("failed to serialize config")?;
    fs::write(path, content).context("failed to write config.json")?;
    Ok(())
}

fn generate_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("ws-{}-{}", now_secs(), count)
}

fn discover_workspaces_from_fs() -> Vec<Workspace> {
    let roots = env::var_os("AGENT_WORKSPACES")
        .map(|v| env::split_paths(&v).collect())
        .unwrap_or_else(default_roots);

    let mut ws: Vec<_> = roots
        .into_iter()
        .filter(|p| p.join(".git").exists())
        .map(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("?").into();
            Workspace {
                id: generate_id(),
                name,
                path: Some(p),
                created_at: now_secs(),
                expanded: true,
            }
        })
        .collect();
    ws.sort_by(|a, b| a.name.cmp(&b.name));
    ws
}

fn encode_project_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    s.replace('/', "-")
}

fn default_roots() -> Vec<PathBuf> {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let parent = cwd.parent().unwrap_or(&cwd);
    fs::read_dir(parent)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect()
}

// ─── Session discovery from ~/.claude/projects/ ───────────

fn claude_projects_dir() -> PathBuf {
    PathBuf::from(env::var("HOME").unwrap_or_default()).join(".claude/projects")
}

/// Custom title override stored in our data directory.
fn title_override_path(session_id: &str) -> PathBuf {
    data_dir().join("sessions").join(format!("{}.title", session_id))
}

/// Legacy title override from Claude's project dir (fallback).
fn legacy_title_override_path(workspace_path: &Path, session_id: &str) -> PathBuf {
    let encoded = encode_project_path(workspace_path);
    claude_projects_dir()
        .join(encoded)
        .join(format!("{}.title", session_id))
}

fn save_session_title(session_id: &str, title: &str) -> io::Result<()> {
    let _ = ensure_data_dir();
    let path = title_override_path(session_id);
    fs::write(path, title)
}

fn load_session_title(session_id: &str, workspace_path: Option<&Path>) -> Option<String> {
    // New location first
    let new_path = title_override_path(session_id);
    if let Ok(content) = fs::read_to_string(&new_path) {
        let trimmed = content.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    // Fall back to legacy location
    if let Some(wp) = workspace_path {
        let legacy_path = legacy_title_override_path(wp, session_id);
        if let Ok(content) = fs::read_to_string(&legacy_path) {
            let trimmed = content.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    None
}

fn discover_sessions(workspaces: &[Workspace]) -> Vec<Session> {
    let mut sessions = Vec::new();
    discover_claude_sessions(workspaces, &mut sessions);
    discover_codex_sessions(workspaces, &mut sessions);
    sessions.sort_by(|a, b| b.last_active.cmp(&a.last_active));
    sessions
}

fn find_session_jsonl(session: &Session) -> Option<PathBuf> {
    match session.agent {
        Agent::Claude => {
            let projects_dir = Agent::Claude.sessions_dir()?;
            let encoded = encode_project_path(&session.workspace_path);
            let path = projects_dir.join(encoded).join(format!("{}.jsonl", session.id));
            if path.exists() { Some(path) } else { None }
        }
        Agent::Codex => {
            // Codex sessions are under ~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl
            // Scan for a file containing this session ID
            let sessions_root = Agent::Codex.sessions_dir()?;
            walk_codex_jsonl(&sessions_root, &session.id)
        }
    }
}

fn walk_codex_jsonl(root: &Path, session_id: &str) -> Option<PathBuf> {
    if let Ok(years) = fs::read_dir(root) {
        for year in years.flatten() {
            if !year.path().is_dir() { continue; }
            if let Ok(months) = fs::read_dir(year.path()) {
                for month in months.flatten() {
                    if !month.path().is_dir() { continue; }
                    if let Ok(days) = fs::read_dir(month.path()) {
                        for day in days.flatten() {
                            if !day.path().is_dir() { continue; }
                            if let Ok(files) = fs::read_dir(day.path()) {
                                for file in files.flatten() {
                                    let path = file.path();
                                    if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                                        continue;
                                    }
                                    // Quick check: see if the file contains this session ID
                                    if let Ok(content) = fs::read_to_string(&path) {
                                        if content.contains(session_id) {
                                            return Some(path);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn discover_claude_sessions(workspaces: &[Workspace], out: &mut Vec<Session>) {
    let projects_dir = match Agent::Claude.sessions_dir() {
        Some(d) => d,
        None => return,
    };

    for ws in workspaces {
        let ws_path = ws.path.clone().unwrap_or_else(|| {
            let dir = data_dir().join("workspaces").join(&ws.id);
            let _ = fs::create_dir_all(&dir);
            dir
        });
        let encoded = encode_project_path(&ws_path);
        let proj_dir = projects_dir.join(encoded);
        let entries = match fs::read_dir(&proj_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("?").to_string();
            let last_active = fs::metadata(&path)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let title = load_session_title(&id, Some(&ws_path))
                .or_else(|| extract_claude_title(&path))
                .unwrap_or_else(|| format!("Session {}", &id[..8.min(id.len())]));

            out.push(Session {
                id,
                workspace_path: ws_path.clone(),
                title,
                last_active,
                agent: Agent::Claude,
            });
        }
    }
}

fn extract_claude_title(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let record: ClaudeRecord = serde_json::from_str(line).ok()?;
        if record.record_type.as_deref() != Some("user") { continue; }
        let msg = record.message?;
        if msg.role.as_deref() != Some("user") { continue; }
        let text = extract_text_from_content(msg.content?)?;
        let cleaned = clean_user_message(&text);
        if !cleaned.is_empty() {
            return Some(cleaned.chars().take(50).collect());
        }
    }
    None
}

fn discover_codex_sessions(workspaces: &[Workspace], out: &mut Vec<Session>) {
    let sessions_root = match Agent::Codex.sessions_dir() {
        Some(d) => d,
        None => return,
    };

    // Collect valid workspace paths (with HOME as fallback for virtual)
    let ws_paths: Vec<PathBuf> = workspaces.iter().map(|ws| {
        ws.path.clone().unwrap_or_else(|| {
            let dir = data_dir().join("workspaces").join(&ws.id);
            let _ = fs::create_dir_all(&dir);
            dir
        })
    }).collect();

    // Walk sessions/YYYY/MM/DD/*.jsonl
    if let Ok(years) = fs::read_dir(&sessions_root) {
        for year in years.flatten() {
            if !year.path().is_dir() { continue; }
            if let Ok(months) = fs::read_dir(year.path()) {
                for month in months.flatten() {
                    if !month.path().is_dir() { continue; }
                    if let Ok(days) = fs::read_dir(month.path()) {
                        for day in days.flatten() {
                            if !day.path().is_dir() { continue; }
                            if let Ok(files) = fs::read_dir(day.path()) {
                                for file in files.flatten() {
                                    let path = file.path();
                                    if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                                        continue;
                                    }

                                    let meta = parse_codex_session(&path);
                                    let (id, title, cwd) = match meta {
                                        Some(m) => m,
                                        None => continue,
                                    };

                                    // Find which workspace this belongs to
                                    let ws_path = ws_paths.iter()
                                        .find(|p| cwd == p.to_string_lossy().as_ref())
                                        .cloned()
                                        .unwrap_or_else(|| ws_paths.first().cloned().unwrap_or_default());

                                    let last_active = fs::metadata(&path)
                                        .ok()
                                        .and_then(|m| m.modified().ok())
                                        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                                        .map(|d| d.as_secs())
                                        .unwrap_or(0);

                                    out.push(Session {
                                        id,
                                        workspace_path: ws_path,
                                        title: title.unwrap_or_else(|| "Codex session".into()),
                                        last_active,
                                        agent: Agent::Codex,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn parse_codex_session(path: &Path) -> Option<(String, Option<String>, String)> {
    let content = fs::read_to_string(path).ok()?;
    let mut id = String::new();
    let mut cwd = String::new();
    let mut first_user_msg: Option<String> = None;

    for line in content.lines() {
        let record: serde_json::Value = serde_json::from_str(line).ok()?;
        let r#type = record.get("type")?.as_str()?;

        match r#type {
            "session_meta" => {
                let p = record.get("payload")?;
                id = p.get("id")?.as_str()?.to_string();
                cwd = p.get("cwd").and_then(|v| v.as_str()).unwrap_or("").to_string();
            }
            "user_message" if first_user_msg.is_none() => {
                let text = record.get("payload")?.get("text")?.as_str()?;
                let truncated: String = text.chars().take(50).collect();
                first_user_msg = Some(truncated);
            }
            _ => {}
        }

        if !id.is_empty() && first_user_msg.is_some() {
            break;
        }
    }

    if id.is_empty() { return None; }
    Some((id, first_user_msg, cwd))
}

fn clean_user_message(text: &str) -> String {
    let mut cleaned = text.to_string();

    if let Some(start) = cleaned.find("P>|") {
        if let Some(end) = cleaned[start..].find('\\') {
            cleaned = format!("{}{}", &cleaned[..start], &cleaned[start + end + 1..]);
        }
    }

    let noise_prefixes = ["\x1b", "P>|", "P<|"];
    for prefix in noise_prefixes {
        if cleaned.starts_with(prefix) {
            return String::new();
        }
    }

    cleaned.trim().to_string()
}

fn extract_text_from_content(content: serde_json::Value) -> Option<String> {
    match content {
        serde_json::Value::String(s) => Some(s),
        serde_json::Value::Array(arr) => {
            let mut texts = Vec::new();
            for item in arr {
                if item.get("type").and_then(|v| v.as_str()) == Some("text") {
                    if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                        texts.push(t.to_string());
                    }
                }
            }
            if texts.is_empty() {
                None
            } else {
                Some(texts.join(" "))
            }
        }
        _ => None,
    }
}

// ─── Time utilities ───────────────────────────────────────

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn relative_time(secs: u64) -> String {
    let diff = now_secs().saturating_sub(secs);
    match diff {
        0..=59 => "just now".into(),
        60..=3599 => format!("{}m ago", diff / 60),
        3600..=86399 => format!("{}h ago", diff / 3600),
        _ => format!("{}d ago", diff / 86400),
    }
}

// ─── App ──────────────────────────────────────────────────

struct App {
    workspaces: Vec<Workspace>,
    sessions: Vec<Session>,
    tree: Vec<TreeNode>,
    ws_session_map: Vec<Vec<usize>>,
    tree_state: ListState,
    focus: Focus,
    input_mode: InputMode,
    input_buffer: String,
    rename_target: Option<usize>, // session index for rename
    rename_workspace_target: Option<usize>, // workspace index for rename
    new_workspace_name: Option<String>, // temp name during workspace creation
    pending_session_name: Option<String>, // name entered before agent selection
    available_agents: Vec<Agent>, // detected at startup
    agent_state: ListState, // selection state for agent picker
    browse_dir: PathBuf,          // current directory in browser
    browse_entries: Vec<DirEntry>, // cached directory listing
    browse_state: ListState,      // selection state for browser
    ptys: Vec<PtySlot>,
    active_pty: Option<usize>, // which PTY is shown in chat area
    status: String,
    last_chat_area: Rect,
    last_refresh: std::time::Instant,
    prev_states: Vec<PtyState>, // track transitions
}

impl App {
    fn new() -> Self {
        let mut config = load_config().unwrap_or_else(|_| Config {
            workspaces: Vec::new(),
        });

        // First run: auto-discover from filesystem
        if config.workspaces.is_empty() {
            config.workspaces = discover_workspaces_from_fs();
            let _ = save_config_file(&config);
        }

        // Ensure all workspaces start expanded
        for ws in &mut config.workspaces {
            ws.expanded = true;
        }

        let sessions = discover_sessions(&config.workspaces);
        let mut app = Self {
            workspaces: config.workspaces,
            sessions,
            tree: Vec::new(),
            ws_session_map: Vec::new(),
            tree_state: ListState::default(),
            focus: Focus::Sidebar,
            input_mode: InputMode::None,
            input_buffer: String::new(),
            rename_target: None,
            rename_workspace_target: None,
            new_workspace_name: None,
            pending_session_name: None,
            available_agents: detect_agents(),
            agent_state: ListState::default(),
            browse_dir: PathBuf::new(),
            browse_entries: Vec::new(),
            browse_state: ListState::default(),
            ptys: Vec::new(),
            active_pty: None,
            status: "Enter:new/resume e:expand r:refresh R:rename N:new-ws D:del-ws q:quit".into(),
            last_chat_area: Rect::default(),
            last_refresh: std::time::Instant::now(),
            prev_states: Vec::new(),
        };
        app.rebuild_tree();
        if !app.tree.is_empty() {
            app.tree_state.select(Some(0));
        }
        app
    }

    fn poll_states(&mut self) {
        for slot in &mut self.ptys {
            if !slot.info.completed && slot.handle.state() == PtyState::Completed {
                slot.info.completed = true;
            }
        }

        // Auto-remove dead Codex sessions (no resume support)
        let before = self.ptys.len();
        self.ptys.retain(|slot| {
            if slot.info.agent == Agent::Codex
                && !slot.handle.alive.load(Ordering::Relaxed)
            {
                return false;
            }
            true
        });
        if self.ptys.len() != before {
            // Fix active_pty index if needed
            if let Some(cur) = self.active_pty {
                if cur >= self.ptys.len() {
                    self.active_pty = if self.ptys.is_empty() { None } else { Some(self.ptys.len() - 1) };
                }
            }
            self.rebuild_tree();
        }

        self.prev_states = self.ptys.iter().map(|s| s.handle.state()).collect();
    }

    fn pty_display_state(&self, pi: usize) -> PtyState {
        if let Some(slot) = self.ptys.get(pi) {
            if slot.info.completed {
                return PtyState::Completed;
            }
            slot.handle.state()
        } else {
            PtyState::Running
        }
    }

    fn refresh_sessions(&mut self) {
        self.sessions = discover_sessions(&self.workspaces);

        // Link newly created sessions to running PTYs
        for slot in &mut self.ptys {
            if slot.info.session_id.is_none() {
                if let Some(found) = self.sessions.iter().find(|s| {
                    s.workspace_path == slot.info.workspace_path
                        && s.last_active >= slot.info.started_at
                }) {
                    slot.info.session_id = Some(found.id.clone());
                }
            }
        }

        self.rebuild_tree();
    }

    fn rebuild_tree(&mut self) {
        let mut tree = Vec::new();
        let mut ws_map = Vec::new();

        for (wi, _ws) in self.workspaces.iter().enumerate() {
            let sess_idxs: Vec<usize> = self
                .sessions
                .iter()
                .enumerate()
                .filter(|(_, s)| self.ws_matches_path(wi, &s.workspace_path))
                .map(|(i, _)| i)
                .collect();

            tree.push(TreeNode::Workspace(wi));
            if self.workspaces[wi].expanded {
                // Active PTYs not yet discovered on disk
                for (pi, slot) in self.ptys.iter().enumerate() {
                    if self.ws_matches_path(wi, &slot.info.workspace_path)
                        && slot.info.session_id.is_none()
                    {
                        tree.push(TreeNode::ActiveTab(pi));
                    }
                }
                for &si in &sess_idxs {
                    tree.push(TreeNode::Session(wi, si));
                }
            }
            ws_map.push(sess_idxs);
        }

        self.tree = tree;
        self.ws_session_map = ws_map;
    }

    fn pty_index_for_session(&self, session_id: &str) -> Option<usize> {
        self.ptys
            .iter()
            .position(|s| s.info.session_id.as_deref() == Some(session_id))
    }

    fn selected_node(&self) -> Option<&TreeNode> {
        self.tree_state.selected().and_then(|i| self.tree.get(i))
    }

    fn move_sel(&mut self, delta: isize) {
        let len = self.tree.len();
        if len == 0 {
            return;
        }
        let cur = self.tree_state.selected().unwrap_or(0).min(len - 1) as isize;
        self.tree_state
            .select(Some(((cur + delta).rem_euclid(len as isize)) as usize));
    }

    fn toggle_expand(&mut self) {
        if let Some(TreeNode::Workspace(wi)) = self.selected_node() {
            let wi = *wi;
            self.workspaces[wi].expanded = !self.workspaces[wi].expanded;
            self.rebuild_tree();
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<Action> {
        if self.input_mode != InputMode::None {
            return self.handle_input_key(key);
        }

        if self.focus == Focus::Chat {
            if let Some(idx) = self.active_pty {
                if key.code == KeyCode::Tab && !key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.focus = Focus::Sidebar;
                    self.refresh_sessions();
                    self.status = "Sessions refreshed.".into();
                    return Ok(Action::Continue);
                }
                if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    // Kill the active PTY
                    self.ptys.remove(idx);
                    self.active_pty = None;
                    self.focus = Focus::Sidebar;
                    self.refresh_sessions();
                    self.status = "Claude Code terminated. Sessions refreshed.".into();
                    return Ok(Action::Continue);
                }
                // Ctrl+J / Ctrl+K: switch between active PTYs
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    if key.code == KeyCode::Char('j') || key.code == KeyCode::Char('k') {
                        if self.ptys.len() > 1 {
                            let cur = self.active_pty.unwrap_or(0);
                            let delta = if key.code == KeyCode::Char('j') {
                                1isize
                            } else {
                                -1
                            };
                            let next = ((cur as isize + delta).rem_euclid(self.ptys.len() as isize))
                                as usize;
                            self.active_pty = Some(next);
                            self.status = format!(
                                "Switched to: {} ({}/{})",
                                self.ptys[next].info.title,
                                next + 1,
                                self.ptys.len()
                            );
                        }
                        return Ok(Action::Continue);
                    }
                }
                let bytes = key_to_bytes(&key);
                if !bytes.is_empty() {
                    if let Some(slot) = self.ptys.get(idx) {
                        slot.handle.write_input(&bytes);
                    }
                }
                return Ok(Action::Continue);
            }

            match key.code {
                KeyCode::Tab => {
                    self.focus = Focus::Sidebar;
                    self.refresh_sessions();
                }
                KeyCode::Char('q') | KeyCode::Esc => return Ok(Action::Quit),
                _ => {}
            }
            return Ok(Action::Continue);
        }

        // Sidebar mode
        match key {
            KeyEvent {
                code: KeyCode::Char('q') | KeyCode::Esc,
                ..
            } => Ok(Action::Quit),

            KeyEvent {
                code: KeyCode::Char('j') | KeyCode::Down,
                ..
            } => {
                self.move_sel(1);
                Ok(Action::Continue)
            }

            KeyEvent {
                code: KeyCode::Char('k') | KeyCode::Up,
                ..
            } => {
                self.move_sel(-1);
                Ok(Action::Continue)
            }

            KeyEvent {
                code: KeyCode::Char('e'),
                ..
            } => {
                self.toggle_expand();
                Ok(Action::Continue)
            }

            KeyEvent {
                code: KeyCode::Char('r'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.refresh_sessions();
                self.status = "Sessions refreshed.".into();
                Ok(Action::Continue)
            }

            KeyEvent {
                code: KeyCode::Char('R'),
                ..
            } => {
                self.start_rename();
                Ok(Action::Continue)
            }

            KeyEvent {
                code: KeyCode::Char('N'),
                ..
            } => {
                self.start_new_workspace();
                Ok(Action::Continue)
            }

            KeyEvent {
                code: KeyCode::Char('D'),
                ..
            } => {
                self.delete_selected();
                Ok(Action::Continue)
            }

            KeyEvent {
                code: KeyCode::Tab, ..
            } => {
                if self.ptys.is_empty() {
                    self.status = "No active session. Press Enter to start one.".into();
                } else {
                    self.focus = Focus::Chat;
                    if self.active_pty.is_none() {
                        self.active_pty = Some(0);
                    }
                }
                Ok(Action::Continue)
            }

            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                self.activate_selection()?;
                Ok(Action::Continue)
            }

            _ => Ok(Action::Continue),
        }
    }

    fn handle_input_key(&mut self, key: KeyEvent) -> Result<Action> {
        if self.input_mode == InputMode::BrowseDir {
            return self.handle_browse_key(key);
        }
        if self.input_mode == InputMode::SelectAgent {
            return self.handle_agent_key(key);
        }

        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::None;
                self.input_buffer.clear();
                self.rename_target = None;
                self.rename_workspace_target = None;
                self.status = "Cancelled.".into();
            }
            KeyCode::Enter => {
                self.confirm_input()?;
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            _ => {}
        }
        Ok(Action::Continue)
    }

    fn handle_browse_key(&mut self, key: KeyEvent) -> Result<Action> {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::None;
                self.new_workspace_name = None;
                self.status = "Cancelled.".into();
            }
            KeyCode::Enter => {
                self.browse_select();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.browse_move(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.browse_move(-1);
            }
            KeyCode::Backspace | KeyCode::Char('h') => {
                self.browse_up();
            }
            _ => {}
        }
        Ok(Action::Continue)
    }

    fn handle_agent_key(&mut self, key: KeyEvent) -> Result<Action> {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::None;
                self.pending_session_name = None;
                self.status = "Cancelled.".into();
            }
            KeyCode::Enter => {
                self.confirm_input()?;
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                if self.available_agents.contains(&Agent::Claude) {
                    self.agent_state.select(Some(
                        self.available_agents.iter().position(|a| *a == Agent::Claude).unwrap()
                    ));
                    self.confirm_input()?;
                }
            }
            KeyCode::Char('x') | KeyCode::Char('X') => {
                if self.available_agents.contains(&Agent::Codex) {
                    self.agent_state.select(Some(
                        self.available_agents.iter().position(|a| *a == Agent::Codex).unwrap()
                    ));
                    self.confirm_input()?;
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let len = self.available_agents.len();
                if len > 0 {
                    let cur = self.agent_state.selected().unwrap_or(0).min(len - 1);
                    let next = (cur + 1) % len;
                    self.agent_state.select(Some(next));
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let len = self.available_agents.len();
                if len > 0 {
                    let cur = self.agent_state.selected().unwrap_or(0).min(len - 1);
                    let prev = (cur + len - 1) % len;
                    self.agent_state.select(Some(prev));
                }
            }
            _ => {}
        }
        Ok(Action::Continue)
    }

    fn spawn_session(&mut self, agent: Agent) -> Result<()> {
        let name = self.pending_session_name.take();
        self.spawn_with_agent(agent, name)
    }

    fn spawn_with_agent(&mut self, agent: Agent, name: Option<String>) -> Result<()> {
        let chat_size = self.chat_size();

        match self.selected_node().cloned() {
            Some(TreeNode::Workspace(wi)) => {
                let path = self.workspace_cwd(wi);
                let display_name = name.clone().unwrap_or_else(|| "unnamed".into());
                self.status = format!(
                    "Starting {} '{}' in {}...",
                    agent.label(), display_name, self.workspaces[wi].name
                );
                let pty = PtyHandle::spawn(agent, &path, None, name.as_deref(), chat_size)
                    .context(format!("failed to spawn {}", agent.label()))?;
                let idx = self.ptys.len();
                self.ptys.push(PtySlot {
                    handle: pty,
                    info: RunningInfo {
                        workspace_path: path,
                        title: display_name,
                        session_id: None,
                        started_at: now_secs(),
                        completed: false,
                        agent,
                    },
                });
                self.active_pty = Some(idx);
                self.focus = Focus::Chat;
                self.rebuild_tree();
            }
            Some(TreeNode::Session(_wi, si)) => {
                let session = self.sessions[si].clone();
                if let Some(existing) = self.pty_index_for_session(&session.id) {
                    self.active_pty = Some(existing);
                    self.focus = Focus::Chat;
                    return Ok(());
                }
                let path = session.workspace_path.clone();
                let id = session.id.clone();
                let title = session.title.clone();
                self.status = format!("Resuming: {}...", &id[..8.min(id.len())]);
                let pty = PtyHandle::spawn(agent, &path, Some(&id), name.as_deref(), chat_size)
                    .context("failed to resume session")?;
                let idx = self.ptys.len();
                self.ptys.push(PtySlot {
                    handle: pty,
                    info: RunningInfo {
                        workspace_path: path,
                        title,
                        session_id: Some(id),
                        started_at: now_secs(),
                        completed: false,
                        agent,
                    },
                });
                self.active_pty = Some(idx);
                self.focus = Focus::Chat;
                self.rebuild_tree();
            }
            Some(TreeNode::ActiveTab(pi)) => {
                self.active_pty = Some(pi);
                self.focus = Focus::Chat;
            }
            None => {}
        }
        Ok(())
    }

    fn handle_paste(&mut self, text: &str) -> Result<Action> {
        if self.input_mode != InputMode::None {
            self.input_buffer.push_str(text);
        } else if self.focus == Focus::Chat {
            if let Some(idx) = self.active_pty {
                if let Some(slot) = self.ptys.get(idx) {
                    slot.handle.write_input(text.as_bytes());
                }
            }
        }
        Ok(Action::Continue)
    }

    fn confirm_input(&mut self) -> Result<()> {
        match self.input_mode {
            InputMode::SessionName => {
                let name = if self.input_buffer.trim().is_empty() {
                    None
                } else {
                    Some(self.input_buffer.clone())
                };
                self.pending_session_name = name;
                self.input_buffer.clear();
                if self.available_agents.len() == 1 {
                    // Only one agent available, skip picker
                    let agent = self.available_agents[0];
                    self.input_mode = InputMode::None;
                    self.spawn_session(agent)?;
                } else {
                    self.input_mode = InputMode::SelectAgent;
                    self.agent_state.select(Some(0));
                    self.status = "Select agent \u{00b7} Enter to confirm \u{00b7} Esc to cancel".into();
                }
            }
            InputMode::RenameSession => {
                if let Some(si) = self.rename_target {
                    let new_title = if self.input_buffer.trim().is_empty() {
                        self.sessions[si].id[..8.min(self.sessions[si].id.len())].to_string()
                    } else {
                        self.input_buffer.clone()
                    };
                    let _ = save_session_title(&self.sessions[si].id, &new_title);
                    self.sessions[si].title = new_title.clone();
                    self.status = format!("Renamed to: {}", new_title);
                    self.rebuild_tree();
                }
                self.input_mode = InputMode::None;
                self.input_buffer.clear();
                self.rename_target = None;
            }
            InputMode::NewWorkspaceName => {
                let name = self.input_buffer.trim().to_string();
                if name.is_empty() {
                    self.status = "Workspace name cannot be empty.".into();
                    self.input_mode = InputMode::None;
                    self.input_buffer.clear();
                } else {
                    self.new_workspace_name = Some(name);
                    self.input_buffer.clear();
                    self.start_browse_dir();
                }
            }
            InputMode::RenameWorkspace => {
                if let Some(wi) = self.rename_workspace_target {
                    let new_name = if self.input_buffer.trim().is_empty() {
                        self.workspaces[wi].name.clone()
                    } else {
                        self.input_buffer.clone()
                    };
                    self.workspaces[wi].name = new_name.clone();
                    self.save_config();
                    self.status = format!("Workspace renamed to: {}", new_name);
                    self.rebuild_tree();
                }
                self.input_mode = InputMode::None;
                self.input_buffer.clear();
                self.rename_workspace_target = None;
            }
            InputMode::SelectAgent => {
                if let Some(idx) = self.agent_state.selected() {
                    if let Some(&agent) = self.available_agents.get(idx) {
                        self.input_mode = InputMode::None;
                        self.spawn_session(agent)?;
                        return Ok(());
                    }
                }
                self.input_mode = InputMode::None;
            }
            InputMode::None | InputMode::BrowseDir => {}
        }
        Ok(())
    }

    fn start_rename(&mut self) {
        match self.selected_node().cloned() {
            Some(TreeNode::Workspace(wi)) => {
                if wi < self.workspaces.len() {
                    self.input_mode = InputMode::RenameWorkspace;
                    self.rename_workspace_target = Some(wi);
                    self.input_buffer = self.workspaces[wi].name.clone();
                    self.status = "Edit workspace name (Enter=confirm, Esc=cancel):".into();
                }
            }
            Some(TreeNode::Session(_, si)) => {
                if si < self.sessions.len() {
                    self.input_mode = InputMode::RenameSession;
                    self.rename_target = Some(si);
                    self.input_buffer = self.sessions[si].title.clone();
                    self.status = "Edit session name (Enter=confirm, Esc=cancel):".into();
                }
            }
            _ => {}
        }
    }

    fn start_new_workspace(&mut self) {
        self.input_mode = InputMode::NewWorkspaceName;
        self.input_buffer.clear();
        self.new_workspace_name = None;
        self.status = "Workspace name (Esc = cancel):".into();
    }

    fn start_browse_dir(&mut self) {
        let home = PathBuf::from(env::var("HOME").unwrap_or_else(|_| "/".into()));
        self.browse_dir = home;
        self.load_browse_entries();
        self.input_mode = InputMode::BrowseDir;
        self.status = "Select directory \u{00b7} Enter: open/select \u{00b7} Backspace: up \u{00b7} Esc: cancel".into();
    }

    fn load_browse_entries(&mut self) {
        let mut entries = Vec::new();

        // Special entries: select current, virtual
        entries.push(DirEntry {
            name: SELECT_CURRENT.into(),
            path: self.browse_dir.clone(),
            is_dir: true,
        });
        entries.push(DirEntry {
            name: SELECT_VIRTUAL.into(),
            path: PathBuf::new(),
            is_dir: false,
        });

        // Parent
        if self.browse_dir.parent().is_some() {
            entries.push(DirEntry {
                name: PARENT_DIR.into(),
                path: self.browse_dir.parent().unwrap().to_path_buf(),
                is_dir: true,
            });
        }

        // Actual directory contents (only directories)
        if let Ok(rd) = fs::read_dir(&self.browse_dir) {
            let mut subdirs: Vec<DirEntry> = rd
                .flatten()
                .filter(|e| e.path().is_dir())
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    // Skip hidden dirs
                    if name.starts_with('.') {
                        return None;
                    }
                    Some(DirEntry {
                        name,
                        path: e.path(),
                        is_dir: true,
                    })
                })
                .collect();
            subdirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            entries.extend(subdirs);
        }

        self.browse_entries = entries;
        self.browse_state.select(Some(0));
    }

    fn browse_move(&mut self, delta: isize) {
        let len = self.browse_entries.len();
        if len == 0 {
            return;
        }
        let cur = self.browse_state.selected().unwrap_or(0).min(len - 1) as isize;
        self.browse_state
            .select(Some(((cur + delta).rem_euclid(len as isize)) as usize));
    }

    fn browse_select(&mut self) {
        let idx = match self.browse_state.selected() {
            Some(i) => i,
            None => return,
        };
        let entry = match self.browse_entries.get(idx) {
            Some(e) => e.clone(),
            None => return,
        };

        match entry.name.as_str() {
            SELECT_CURRENT => {
                // Create workspace bound to this directory
                let name = self.new_workspace_name.take().unwrap_or_default();
                let ws = Workspace {
                    id: generate_id(),
                    name,
                    path: Some(entry.path.clone()),
                    created_at: now_secs(),
                    expanded: true,
                };
                self.status = format!("Created workspace: {} \u{2192} {}", ws.name, ws.path.as_ref().unwrap().display());
                self.workspaces.push(ws);
                self.save_config();
                self.rebuild_tree();
                self.input_mode = InputMode::None;
            }
            SELECT_VIRTUAL => {
                // Create virtual workspace
                let name = self.new_workspace_name.take().unwrap_or_default();
                let ws = Workspace {
                    id: generate_id(),
                    name,
                    path: None,
                    created_at: now_secs(),
                    expanded: true,
                };
                self.status = format!("Created virtual workspace: {}", ws.name);
                self.workspaces.push(ws);
                self.save_config();
                self.rebuild_tree();
                self.input_mode = InputMode::None;
            }
            PARENT_DIR => {
                self.browse_dir = entry.path;
                self.load_browse_entries();
            }
            _ => {
                // Enter subdirectory
                self.browse_dir = entry.path;
                self.load_browse_entries();
            }
        }
    }

    fn browse_up(&mut self) {
        if let Some(parent) = self.browse_dir.parent() {
            self.browse_dir = parent.to_path_buf();
            self.load_browse_entries();
        }
    }

    fn delete_selected(&mut self) {
        match self.selected_node().cloned() {
            Some(TreeNode::Workspace(wi)) => {
                let name = self.workspaces[wi].name.clone();
                self.workspaces.remove(wi);
                self.save_config();
                self.refresh_sessions();
                self.status = format!("Deleted workspace: {}", name);
            }
            Some(TreeNode::Session(_wi, si)) => {
                if si >= self.sessions.len() { return; }
                let session = self.sessions[si].clone();
                // Kill active PTY if running
                if let Some(pi) = self.pty_index_for_session(&session.id) {
                    self.ptys.remove(pi);
                    if let Some(cur) = self.active_pty {
                        if cur >= self.ptys.len() {
                            self.active_pty = if self.ptys.is_empty() { None } else { Some(self.ptys.len() - 1) };
                        }
                    }
                }
                // Delete title override
                let title_path = title_override_path(&session.id);
                let _ = fs::remove_file(&title_path);
                // Delete the session JSONL file
                if let Some(jsonl) = find_session_jsonl(&session) {
                    let _ = fs::remove_file(&jsonl);
                }
                let title = session.title.clone();
                self.sessions.remove(si);
                self.rebuild_tree();
                self.status = format!("Deleted session: {}", title);
            }
            _ => {}
        }
    }

    fn save_config(&self) {
        let config = Config {
            workspaces: self.workspaces.clone(),
        };
        if let Err(e) = save_config_file(&config) {
            eprintln!("Failed to save config: {}", e);
        }
    }

    fn workspace_cwd(&self, wi: usize) -> PathBuf {
        match &self.workspaces[wi].path {
            Some(p) => p.clone(),
            None => {
                let dir = data_dir().join("workspaces").join(&self.workspaces[wi].id);
                let _ = fs::create_dir_all(&dir);
                dir
            }
        }
    }

    fn ws_matches_path(&self, wi: usize, path: &Path) -> bool {
        match &self.workspaces[wi].path {
            Some(p) => p == path,
            None => path == self.workspace_cwd(wi),
        }
    }

    fn activate_selection(&mut self) -> Result<()> {
        match self.selected_node().cloned() {
            Some(TreeNode::Workspace(_)) => {
                self.input_mode = InputMode::SessionName;
                self.input_buffer.clear();
                self.status = "Enter session name (empty = unnamed, Esc = cancel):".into();
            }
            Some(TreeNode::Session(_wi, si)) => {
                let agent = self.sessions[si].agent;
                self.spawn_with_agent(agent, None)?;
            }
            Some(TreeNode::ActiveTab(pi)) => {
                self.active_pty = Some(pi);
                self.focus = Focus::Chat;
            }
            None => {}
        }
        Ok(())
    }

    fn chat_size(&self) -> (u16, u16) {
        (
            self.last_chat_area.width.saturating_sub(2),
            self.last_chat_area.height.saturating_sub(2),
        )
    }

    // ─── Rendering ────────────────────────────────────────

    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(4), Constraint::Length(3)])
            .split(area);

        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(chunks[0]);

        self.render_sidebar(frame, cols[0]);
        self.last_chat_area = cols[1];
        self.render_chat(frame, cols[1]);
        self.render_status(frame, chunks[1]);

        if self.input_mode != InputMode::None {
            self.render_input_popup(frame, area);
        }
    }

    fn render_sidebar(&mut self, frame: &mut Frame, area: Rect) {
        let pty_states: Vec<(String, PtyState)> = self
            .ptys
            .iter()
            .enumerate()
            .map(|(i, s)| {
                (
                    s.info.session_id.clone().unwrap_or_default(),
                    self.pty_display_state(i),
                )
            })
            .collect();
        let active_tab_states: Vec<(PtyState, Agent)> = self
            .ptys
            .iter()
            .enumerate()
            .map(|(i, s)| (self.pty_display_state(i), s.info.agent))
            .collect();

        let items: Vec<ListItem> = self
            .tree
            .iter()
            .map(|node| match node {
                TreeNode::Workspace(wi) => {
                    let ws = &self.workspaces[*wi];
                    let icon = if ws.expanded { "\u{25bc}" } else { "\u{25b6}" };
                    let count = self.ws_session_map.get(*wi).map(|v| v.len()).unwrap_or(0);

                    let (binding_icon, binding_style, subtitle) = match &ws.path {
                        Some(p) => (
                            "\u{25c6}",
                            Style::default().fg(Color::Cyan),
                            format!("   {} sessions \u{00b7} {}", count, p.display()),
                        ),
                        None => (
                            "\u{25c7}",
                            Style::default().fg(Color::Yellow),
                            format!("   {} sessions \u{00b7} virtual", count),
                        ),
                    };

                    ListItem::new(vec![
                        Line::from(vec![
                            Span::styled(format!("{} {} ", icon, binding_icon), binding_style.add_modifier(Modifier::BOLD)),
                            Span::styled(ws.name.clone(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        ]),
                        Line::from(subtitle).style(Style::default().fg(Color::DarkGray)),
                    ])
                }
                TreeNode::Session(_wi, si) => {
                    if let Some(session) = self.sessions.get(*si) {
                        let short_id = &session.id[..8.min(session.id.len())];
                        let pty_info = pty_states
                            .iter()
                            .find(|(sid, _)| sid == &session.id);
                        let pty_state = pty_info.map(|(_, s)| *s);

                        // Always show agent tag from session data
                        let agent_tag = Span::styled(
                            format!(" [{}]", session.agent.icon()),
                            Style::default().fg(session.agent.color()),
                        );

                        let (marker, state_tag) = match pty_state {
                            Some(PtyState::Running) => (
                                Span::styled("   \u{25cf} ", Style::default().fg(Color::Yellow)),
                                Span::styled(" [running]", Style::default().fg(Color::Yellow)),
                            ),
                            Some(PtyState::Completed) => (
                                Span::styled("   \u{25cf} ", Style::default().fg(Color::Green)),
                                Span::styled(" \u{2714} done", Style::default().fg(Color::Green)),
                            ),
                            None => (
                                Span::styled("   \u{25cb} ", Style::default().fg(Color::DarkGray)),
                                Span::raw(""),
                            ),
                        };
                        let mut spans = vec![
                            marker,
                            Span::styled(
                                relative_time(session.last_active),
                                Style::default().fg(Color::White),
                            ),
                            Span::styled(
                                format!(" ({})", short_id),
                                Style::default().fg(Color::DarkGray),
                            ),
                            state_tag,
                        ];
                        spans.push(agent_tag);
                        ListItem::new(vec![
                            Line::from(spans),
                            Line::from(format!("     {}", session.title))
                                .style(Style::default().fg(Color::Gray)),
                        ])
                    } else {
                        ListItem::new(Line::from("   \u{25cf} ?"))
                    }
                }
                TreeNode::ActiveTab(pi) => {
                    let title = self.ptys.get(*pi).map(|s| s.info.title.as_str()).unwrap_or("New Session");
                    let info = active_tab_states.get(*pi);
                    let state = info.map(|(s, _)| *s).unwrap_or(PtyState::Running);
                    let agent = info.map(|(_, a)| *a).unwrap_or(Agent::Claude);
                    let (dot_color, state_text) = match state {
                        PtyState::Running => (Color::Yellow, " [running]"),
                        PtyState::Completed => (Color::Green, " \u{2714} done"),
                    };
                    let title_spans = vec![
                        Span::styled("   \u{25cf} ", Style::default().fg(dot_color)),
                        Span::styled(title, Style::default().fg(Color::White)),
                        Span::styled(state_text, Style::default().fg(Color::Green)),
                        Span::styled(format!(" [{}]", agent.icon()), Style::default().fg(agent.color())),
                    ];
                    ListItem::new(vec![
                        Line::from(title_spans),
                        Line::from("     waiting for session file...")
                            .style(Style::default().fg(Color::DarkGray)),
                    ])
                }
            })
            .collect();

        let border_color = if self.focus == Focus::Sidebar {
            Color::Yellow
        } else {
            Color::DarkGray
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Workspaces ")
            .border_style(Style::default().fg(border_color));

        let list = List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(24, 36, 72))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("\u{203a}");

        frame.render_stateful_widget(list, area, &mut self.tree_state);
    }

    fn render_chat(&mut self, frame: &mut Frame, area: Rect) {
        let border_color = if self.focus == Focus::Chat {
            Color::Yellow
        } else {
            Color::DarkGray
        };

        let title = if let Some(idx) = self.active_pty {
            if let Some(slot) = self.ptys.get(idx) {
                format!(" {} [{}] ({}/{}) ", slot.info.title, slot.info.agent.label(), idx + 1, self.ptys.len())
            } else {
                " Agent ".into()
            }
        } else {
            " Agent ".into()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(border_color));

        if let Some(idx) = self.active_pty {
            if let Some(slot) = self.ptys.get(idx) {
                let inner = block.inner(area);
                slot.handle.resize((inner.width, inner.height));

                let parser = slot.handle.screen();
                let screen = parser.read().unwrap().screen().clone();
                let term = PseudoTerminal::new(&screen).block(block);
                frame.render_widget(term, area);
                return;
            }
        }

        let lines = self.render_placeholder();
        frame.render_widget(
            Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn render_placeholder(&self) -> Vec<Line<'static>> {
        let mut lines: Vec<Line> = Vec::new();

        match self.selected_node() {
            Some(TreeNode::Workspace(wi)) => {
                let ws = &self.workspaces[*wi];
                lines.push(
                    Line::from(ws.name.clone()).style(
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
                match &ws.path {
                    Some(p) => {
                        lines.push(
                            Line::from(format!("\u{25c6} {}", p.display()))
                                .style(Style::default().fg(Color::Green)),
                        );
                    }
                    None => {
                        lines.push(
                            Line::from("\u{25c7} Virtual workspace (no directory)")
                                .style(Style::default().fg(Color::Yellow)),
                        );
                    }
                }
                lines.push(Line::from(""));
                lines.push(
                    Line::from("Press Enter to start a named Claude Code session")
                        .style(Style::default().fg(Color::Yellow)),
                );
            }
            Some(TreeNode::Session(_wi, si)) => {
                let session = &self.sessions[*si];
                lines.push(
                    Line::from(session.title.clone()).style(
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
                lines.push(
                    Line::from(format!("ID: {}", session.id))
                        .style(Style::default().fg(Color::DarkGray)),
                );
                lines.push(Line::from(format!(
                    "Last active: {}",
                    relative_time(session.last_active)
                )));
                if self.pty_index_for_session(&session.id).is_some() {
                    lines.push(Line::from(""));
                    lines.push(
                        Line::from("This session is already running - Enter to switch to it")
                            .style(Style::default().fg(Color::Green)),
                    );
                } else {
                    lines.push(Line::from(""));
                    lines.push(
                        Line::from("Press Enter to resume this session")
                            .style(Style::default().fg(Color::Yellow)),
                    );
                }
            }
            Some(&TreeNode::ActiveTab(pi)) => {
                let title = self
                    .ptys
                    .get(pi)
                    .map(|s| s.info.title.clone())
                    .unwrap_or_else(|| "New Session".into());
                lines.push(
                    Line::from(title).style(
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
                lines.push(
                    Line::from("Session is running...").style(Style::default().fg(Color::DarkGray)),
                );
                lines.push(Line::from(""));
                lines.push(
                    Line::from("Press Enter to switch to this session")
                        .style(Style::default().fg(Color::Yellow)),
                );
            }
            None => {
                lines.push(Line::from("No selection").style(Style::default().fg(Color::DarkGray)));
            }
        }

        lines.push(Line::from(""));
        lines.push(
            Line::from("\u{2500}\u{2500} Keybindings \u{2500}\u{2500}")
                .style(Style::default().fg(Color::DarkGray)),
        );
        lines.push(Line::from("Enter        New (with name) / Resume / Switch"));
        lines.push(Line::from("e            Expand / collapse workspace"));
        lines.push(Line::from("j/k \u{2191}\u{2193}     Navigate tree"));
        lines.push(Line::from("r            Refresh sessions"));
        lines.push(Line::from("R            Rename selected session"));
        lines.push(Line::from("N            New workspace"));
        lines.push(Line::from("D            Delete workspace"));
        lines.push(Line::from("Tab          Toggle sidebar/chat"));
        lines.push(Line::from("Ctrl+J/K     Switch between active sessions"));
        lines.push(Line::from("Ctrl+Q       Kill current Claude Code session"));
        lines.push(Line::from("q / Esc      Quit"));

        lines
    }

    fn render_input_popup(&mut self, frame: &mut Frame, area: Rect) {
        if self.input_mode == InputMode::BrowseDir {
            self.render_browse_popup(frame, area);
            return;
        }
        if self.input_mode == InputMode::SelectAgent {
            self.render_agent_popup(frame, area);
            return;
        }

        let popup = centered_rect(60, 20, area);
        frame.render_widget(Clear, popup);

        let (title, label) = match self.input_mode {
            InputMode::SessionName => (" New Session ", "Session name: "),
            InputMode::RenameSession => (" Rename Session ", "New name: "),
            InputMode::RenameWorkspace => (" Rename Workspace ", "New name: "),
            InputMode::NewWorkspaceName => (" New Workspace ", "Workspace name: "),
            _ => return,
        };

        let input = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(label, Style::default().fg(Color::Cyan).bold()),
                Span::styled(&self.input_buffer, Style::default().fg(Color::White)),
                Span::styled("_", Style::default().fg(Color::Gray)),
            ]),
            Line::from(""),
            Line::from("Enter to confirm \u{00b7} Esc to cancel")
                .style(Style::default().fg(Color::DarkGray)),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(Color::Yellow)),
        );

        frame.render_widget(input, popup);
    }

    fn render_agent_popup(&mut self, frame: &mut Frame, area: Rect) {
        let popup = centered_rect(50, 30, area);
        frame.render_widget(Clear, popup);

        let items: Vec<ListItem> = self
            .available_agents
            .iter()
            .map(|agent| {
                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(format!(" [{}] ", agent.icon()), Style::default().fg(agent.color()).add_modifier(Modifier::BOLD)),
                        Span::styled(agent.label(), Style::default().fg(Color::White)),
                    ]),
                    Line::from(format!("     {}", agent.cmd()))
                        .style(Style::default().fg(Color::DarkGray)),
                ])
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Select Agent ")
            .border_style(Style::default().fg(Color::Yellow));

        let inner = block.inner(popup);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(1)])
            .split(inner);

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(24, 36, 72))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("\u{203a}");
        frame.render_stateful_widget(list, chunks[0], &mut self.agent_state);

        let help = Line::from(" C:Claude  X:Codex  j/k:navigate  Enter:confirm  Esc:cancel")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(Paragraph::new(help), chunks[1]);
        frame.render_widget(block, popup);
    }

    fn render_browse_popup(&mut self, frame: &mut Frame, area: Rect) {
        let popup = centered_rect(70, 70, area);
        frame.render_widget(Clear, popup);

        let ws_name = self.new_workspace_name.as_deref().unwrap_or("?");
        let title = format!(" {} \u{2192} Select Directory ", ws_name);

        let items: Vec<ListItem> = self
            .browse_entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let is_select_current = entry.name == SELECT_CURRENT;
                let is_virtual = entry.name == SELECT_VIRTUAL;
                let is_parent = entry.name == PARENT_DIR;

                let style = if is_select_current {
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                } else if is_virtual {
                    Style::default().fg(Color::Yellow)
                } else if is_parent {
                    Style::default().fg(Color::DarkGray)
                } else if i % 2 == 0 {
                    Style::default().fg(Color::White)
                } else {
                    Style::default().fg(Color::Gray)
                };

                let display = if is_select_current {
                    format!("{} {}", entry.name, entry.path.display())
                } else if entry.is_dir && !is_parent && !is_virtual {
                    format!("  \u{25b8} {}", entry.name)
                } else {
                    format!("  {}", entry.name)
                };

                ListItem::new(Line::from(display)).style(style)
            })
            .collect();

        let path_line = Line::from(format!(" {}", self.browse_dir.display()))
            .style(Style::default().fg(Color::Cyan));

        let help_line = Line::from(" j/k:navigate  Enter:open/select  Backspace/h:up  Esc:cancel")
            .style(Style::default().fg(Color::DarkGray));

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(Color::Yellow));

        let inner = block.inner(popup);

        // Split: path header (1 line), list (rest), help footer (1 line)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(3), Constraint::Length(1)])
            .split(inner);

        frame.render_widget(Paragraph::new(path_line), chunks[0]);

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(24, 36, 72))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("\u{203a}");
        frame.render_stateful_widget(list, chunks[1], &mut self.browse_state);

        frame.render_widget(Paragraph::new(help_line), chunks[2]);
        frame.render_widget(block, popup);
    }

    fn render_status(&self, frame: &mut Frame, area: Rect) {
        let active_count = self.ptys.len();
        let pty_status = if active_count > 0 {
            let current = self
                .active_pty
                .map(|i| {
                    self.ptys
                        .get(i)
                        .map(|s| s.info.title.as_str())
                        .unwrap_or("?")
                })
                .unwrap_or("none");
            Span::styled(
                format!(" [{} active: {}]", active_count, current),
                Style::default().fg(Color::Green),
            )
        } else {
            Span::raw("")
        };

        let line = Line::from(vec![
            Span::styled(self.status.clone(), Style::default().fg(Color::White)),
            pty_status,
            Span::raw("  "),
            Span::styled(
                "Enter:new/resume e:expand r:refresh R:rename N:new-ws D:del-ws Tab:toggle Ctrl+J/K:switch Ctrl+Q:kill q:quit",
                Style::default().fg(Color::DarkGray),
            ),
        ]);

        frame.render_widget(
            Paragraph::new(line).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            ),
            area,
        );
    }
}

// ─── Key to PTY bytes ────────────────────────────────────

fn key_to_bytes(key: &KeyEvent) -> Vec<u8> {
    match key.code {
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Delete => vec![27, 91, 51, 126],
        KeyCode::Up => vec![27, 91, 65],
        KeyCode::Down => vec![27, 91, 66],
        KeyCode::Right => vec![27, 91, 67],
        KeyCode::Left => vec![27, 91, 68],
        KeyCode::Home => vec![27, 91, 72],
        KeyCode::End => vec![27, 91, 70],
        KeyCode::PageUp => vec![27, 91, 53, 126],
        KeyCode::PageDown => vec![27, 91, 54, 126],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::Esc => vec![27],
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                let code = c.to_ascii_lowercase();
                if ('a'..='z').contains(&code) {
                    vec![(code as u8) - b'a' + 1]
                } else {
                    c.to_string().into_bytes()
                }
            } else {
                c.to_string().into_bytes()
            }
        }
        KeyCode::F(n) => match n {
            1 => vec![27, 79, 80],
            2 => vec![27, 79, 81],
            3 => vec![27, 79, 82],
            4 => vec![27, 79, 83],
            5 => vec![27, 91, 49, 53, 126],
            6 => vec![27, 91, 49, 55, 126],
            7 => vec![27, 91, 49, 56, 126],
            8 => vec![27, 91, 49, 57, 126],
            9 => vec![27, 91, 50, 48, 126],
            10 => vec![27, 91, 50, 49, 126],
            11 => vec![27, 91, 50, 51, 126],
            12 => vec![27, 91, 50, 52, 126],
            _ => vec![],
        },
        _ => vec![],
    }
}

// ─── UI helpers ───────────────────────────────────────────

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

// ─── Terminal helpers ─────────────────────────────────────

fn init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout)).context("failed to initialize terminal")
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

// ─── Main ─────────────────────────────────────────────────

fn main() -> Result<()> {
    let agents = detect_agents();
    if agents.is_empty() {
        anyhow::bail!("No agent CLI found. Install Claude Code or Codex.");
    }

    ensure_data_dir().context("failed to create data directory")?;

    let mut app = App::new();

    if !io::stdout().is_terminal() {
        let sessions = discover_sessions(&app.workspaces);
        for (wi, ws) in app.workspaces.iter().enumerate() {
            println!("{} {}", ws.name, ws.path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "virtual".into()));
            for s in sessions.iter().filter(|s| app.ws_matches_path(wi, &s.workspace_path)) {
                println!(
                    "  [{}] {} - {}",
                    &s.id[..8],
                    relative_time(s.last_active),
                    s.title
                );
            }
        }
        return Ok(());
    }

    let mut terminal = init_terminal()?;

    let result = loop {
        terminal.draw(|frame| app.render(frame))?;

        // Poll PTY state transitions (cheap, just atomic reads)
        app.poll_states();

        // Auto-refresh sessions every 5s when any PTY is active
        if !app.ptys.is_empty() && app.last_refresh.elapsed() > Duration::from_secs(5) {
            app.refresh_sessions();
            app.last_refresh = std::time::Instant::now();
        }

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        match app.handle_key(key)? {
                            Action::Continue => {}
                            Action::Quit => break Ok(()),
                        }
                    }
                }
                Event::Paste(text) => {
                    app.handle_paste(&text)?;
                }
                _ => {}
            }
        }
    };

    app.ptys.clear();
    restore_terminal(&mut terminal)?;
    result
}

fn which(cmd: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths).find_map(|dir| {
            let full = dir.join(cmd);
            full.is_file().then_some(full)
        })
    })
}
