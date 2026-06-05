use super::*;

/// Collect git branch/diff info and populate `slot.info.git_info` + `slot.info.diff_summary`.
fn collect_git_info(slot: &mut PtySlot) {
    let ws = &slot.info.workspace_path;
    let branch = git_cmd(ws, &["rev-parse", "--abbrev-ref", "HEAD"]).ok();
    let commit = git_cmd(ws, &["rev-parse", "--short", "HEAD"]).ok();
    let stat_output = git_cmd(ws, &["diff", "--stat"]).ok();
    let diff_stat = stat_output.as_ref().and_then(|s| {
        if s.is_empty() {
            None
        } else {
            s.lines().last().map(|l| l.to_string())
        }
    });
    let summary_line = stat_output.and_then(|s| {
        if s.is_empty() {
            None
        } else {
            s.lines().last().map(|l| l.to_string())
        }
    });
    let numstat_output = git_cmd(ws, &["diff", "--numstat"]).ok();
    let (files, ins, del) = numstat_output
        .map(|s| {
            let (files, total_ins, total_del) = s.lines().filter(|l| !l.is_empty()).fold(
                (Vec::new(), 0u32, 0u32),
                |(mut f, a, b), l| {
                    let parts: Vec<&str> = l.split('\t').collect();
                    if parts.len() >= 3 {
                        f.push(parts[2].to_string());
                        (
                            f,
                            a + parts[0].parse::<u32>().unwrap_or(0),
                            b + parts[1].parse::<u32>().unwrap_or(0),
                        )
                    } else if parts.len() >= 2 {
                        (
                            f,
                            a + parts[0].parse::<u32>().unwrap_or(0),
                            b + parts[1].parse::<u32>().unwrap_or(0),
                        )
                    } else {
                        (f, a, b)
                    }
                },
            );
            (files, total_ins, total_del)
        })
        .unwrap_or_default();
    slot.info.git_info = GitInfo {
        branch,
        commit,
        diff_stat,
    };
    slot.info.diff_summary = DiffSummary {
        files_changed: files,
        insertions: ins,
        deletions: del,
        summary_line,
    };
}

/// Spawn a background thread to run post-completion checks (cargo test/clippy etc.).
fn spawn_completion_check(idx: usize, slot: &mut PtySlot, check_override: Option<String>) {
    let project_type = slot.info.project_type;
    slot.info.check_status = CheckStatus::Running;
    let ws = slot.info.workspace_path.clone();
    std::thread::spawn(move || {
        let result = if let Some(ref cmd_str) = check_override {
            let parts: Vec<&str> = cmd_str.split_whitespace().collect();
            if parts.is_empty() {
                CheckStatus::Passed
            } else {
                let (prog, args) = (parts[0], &parts[1..]);
                let out = std::process::Command::new(prog)
                    .args(args)
                    .current_dir(&ws)
                    .output();
                if out.as_ref().map(|o| o.status.success()).unwrap_or(false) {
                    CheckStatus::Passed
                } else {
                    CheckStatus::Failed(cmd_str.clone())
                }
            }
        } else {
            let commands = project_type.check_commands();
            if commands.is_empty() {
                CheckStatus::Passed
            } else {
                let mut errs = Vec::new();
                for (prog, args) in &commands {
                    let out = std::process::Command::new(*prog)
                        .args(args)
                        .current_dir(&ws)
                        .output();
                    if !out.as_ref().map(|o| o.status.success()).unwrap_or(false) {
                        errs.push(format!("{} {} failed", prog, args.join(" ")));
                    }
                }
                if errs.is_empty() {
                    CheckStatus::Passed
                } else {
                    CheckStatus::Failed(errs.join(", "))
                }
            }
        };
        let marker = crate::config::data_dir().join(format!(".check-result-{}", idx));
        let _ = std::fs::write(&marker, serde_json::to_string(&result).unwrap_or_default());
    });
}

/// Save recording metadata (.meta.json) for a completed PTY session.
fn save_recording_meta(slot: &PtySlot) {
    let rec_dir = crate::config::data_dir().join("recordings");
    let id = slot.info.session_id.as_deref().unwrap_or("unknown");
    let path = rec_dir.join(format!("{}.cast", &id[..id.len().min(16)]));
    if path.exists() {
        let header = serde_json::json!({
            "version": 2, "width": 80, "height": 24,
            "timestamp": slot.info.started_at,
            "title": slot.info.title,
            "env": { "TERM": "xterm-256color" }
        });
        let header_path = rec_dir.join(format!("{}.meta.json", &id[..id.len().min(16)]));
        let _ = std::fs::write(
            &header_path,
            serde_json::to_string_pretty(&header).unwrap_or_default(),
        );
    }
}

/// Merge session summary into the workspace knowledge base.
fn merge_session_knowledge(slot: &PtySlot) {
    let ws = &slot.info.workspace_path;
    let summary_text = if let Some(ref session_id) = slot.info.session_id {
        let short_id = &session_id[..session_id.len().min(16)];
        let summary_path = crate::config::data_dir()
            .join("summaries")
            .join(format!("{}.md", short_id));
        std::fs::read_to_string(&summary_path).unwrap_or_default()
    } else {
        String::new()
    };
    if !summary_text.is_empty() {
        let mut kb = crate::knowledge::load_knowledge(ws);
        crate::knowledge::merge_from_session(&mut kb, &summary_text);
        let _ = crate::knowledge::save_knowledge(ws, &kb);
    }
}

/// Execute on_complete hook plugins for a completed PTY session.
fn run_completion_hooks(slot: &PtySlot, plugins: &[crate::types::Plugin]) {
    let session_id = slot.info.session_id.clone().unwrap_or_default();
    let title = slot.info.title.clone();
    for plugin in plugins {
        if plugin.hooks.iter().any(|h| h == "on_complete") {
            let mut cmd = plugin
                .command
                .replace("{workspace}", &slot.info.workspace_path.to_string_lossy())
                .replace("{session_id}", &session_id)
                .replace("{title}", &title);
            cmd.push_str(&format!(" --event on_complete --session_id {}", session_id));
            let _ = std::process::Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .output();
        }
    }
}

impl App {
    pub(crate) fn archive_old_sessions(&mut self) {
        let Some(days) = self.sessions.archive_days else {
            return;
        };
        if days == 0 {
            return;
        }

        let now = now_secs();
        let threshold = now - (days * 86400);

        // Build set of active PTY session IDs — never archive those
        let active_ids: std::collections::HashSet<String> = self
            .ptys
            .ptys
            .iter()
            .filter_map(|slot| slot.info.session_id.clone())
            .collect();

        // First, restore any previously archived sessions that are no longer
        // stale (e.g. if archive_days was increased) back into the main list.
        let restored: Vec<Session> = self
            .sessions
            .archived_sessions
            .drain(..)
            .filter(|s| s.last_active >= threshold || active_ids.contains(&s.id))
            .collect();
        self.sessions.sessions.extend(restored);

        // Now partition: old sessions move to archived_sessions.
        let (still_active, newly_archived): (Vec<Session>, Vec<Session>) =
            std::mem::take(&mut self.sessions.sessions)
                .into_iter()
                .partition(|s| s.last_active >= threshold || active_ids.contains(&s.id));

        self.sessions.sessions = still_active;
        let count = newly_archived.len();
        self.sessions.archived_sessions.extend(newly_archived);

        if count > 0 {
            self.rebuild_tree();
        }
    }

    pub(crate) fn toggle_agent_filter(&mut self, agent: Agent) {
        if self.view.agent_filter == Some(agent) {
            self.view.agent_filter = None;
            self.view.status = "Filter: all agents".to_string();
        } else {
            self.view.agent_filter = Some(agent);
            self.view.status = format!("Filter: {}", agent.label());
        }
        self.rebuild_tree();
    }

    pub(crate) fn refresh_sessions(&mut self) {
        // Load project configs from .amux.json — skip reload if mtime unchanged
        for (wi, _ws) in self.sessions.workspaces.iter().enumerate() {
            let path = self.workspace_cwd(wi);
            let config_path = path.join(".amux.json");
            let current_mtime = std::fs::metadata(&config_path)
                .ok()
                .and_then(|m| m.modified().ok());

            let cached_mtime = self.sessions.project_config_mtimes.get(&path);
            if cached_mtime.is_none() || cached_mtime != current_mtime.as_ref() {
                let config = crate::config::load_project_config(&path);
                self.sessions.project_configs.insert(path.clone(), config);
                if let Some(mt) = current_mtime {
                    self.sessions.project_config_mtimes.insert(path.clone(), mt);
                }
            }
        }
        // Remove entries for deleted workspaces
        let cwd_set: Vec<_> = self
            .sessions
            .workspaces
            .iter()
            .enumerate()
            .map(|(wi, _)| self.workspace_cwd(wi))
            .collect();
        self.sessions
            .project_configs
            .retain(|k, _| cwd_set.contains(k));
        self.sessions
            .project_config_mtimes
            .retain(|k, _| cwd_set.contains(k));
        self.sessions.sessions =
            discover_sessions_cached(&self.sessions.workspaces, &mut self.sessions.session_cache);
        // Filter sessions matching ignore_sessions patterns from project configs
        let mut to_remove = Vec::new();
        for (i, session) in self.sessions.sessions.iter().enumerate() {
            if let Some(pc) = self.sessions.project_configs.get(&session.workspace_path) {
                for pattern in &pc.ignore_sessions {
                    if session.id.contains(pattern) || session.title.contains(pattern) {
                        to_remove.push(i);
                        break;
                    }
                }
            }
        }
        for i in to_remove.into_iter().rev() {
            self.sessions.sessions.remove(i);
        }
        for slot in &mut self.ptys.ptys {
            if slot.info.session_id.is_none()
                && let Some(found) = self.sessions.sessions.iter().find(|s| {
                    s.workspace_path == slot.info.workspace_path
                        && s.last_active >= slot.info.started_at
                })
            {
                slot.info.session_id = Some(found.id.clone());
            }
        }
        self.rebuild_tree();
        self.rebuild_search_index();
        self.archive_old_sessions();
        // Detect file conflicts between running sessions (throttled to 30s)
        if self.last_conflict_check.elapsed() > std::time::Duration::from_secs(30) {
            self.detect_file_conflicts();
            self.last_conflict_check = std::time::Instant::now();
        }
        // Check token budget (throttled to 30s)
        if self.last_budget_check.elapsed() > std::time::Duration::from_secs(30) {
            self.check_token_budget();
            self.last_budget_check = std::time::Instant::now();
        }
        // Collect process resource stats from /proc (throttled to 30s)
        if self.last_stats_check.elapsed() > std::time::Duration::from_secs(30) {
            for slot in &mut self.ptys.ptys {
                if slot.handle.is_alive()
                    && let Some(pid) = slot.handle.child_pid()
                {
                    match crate::procfs::read_process_stats(pid) {
                        Ok(mut stats) => {
                            if let Some(prev) = &slot.process_stats {
                                stats.prev_cpu_user = prev.cpu_user;
                                stats.prev_cpu_system = prev.cpu_system;
                                stats.prev_instant = prev.prev_instant;
                            }
                            crate::procfs::compute_cpu_percent(&mut stats);
                            slot.process_stats = Some(stats);
                        }
                        Err(_) => { /* process exited or no permission */ }
                    }
                }
            }
            self.last_stats_check = std::time::Instant::now();
            self.sync_pty_stats();
        }
    }

    /// Rebuild the BM25 search index from session titles and summaries.
    pub(crate) fn rebuild_search_index(&mut self) {
        self.search_index = crate::search_engine::SearchIndex::new();
        for session in &self.sessions.sessions {
            let text = format!("{} {}", session.title, session.id);
            self.search_index.add_document(&session.id, &text);
        }
    }

    /// Update related sessions for the active PTY using BM25 search.
    pub(crate) fn update_related_sessions(&mut self) {
        let Some(idx) = self.ptys.active_pty else {
            self.view.related_sessions.clear();
            return;
        };
        let Some(slot) = self.ptys.ptys.get(idx) else {
            self.view.related_sessions.clear();
            return;
        };
        // Extract query: prefer the session's last_message, then title,
        // falling back to the PTY slot title.
        let query = slot
            .info
            .session_id
            .as_ref()
            .and_then(|sid| {
                self.sessions
                    .sessions
                    .iter()
                    .find(|s| &s.id == sid)
                    .and_then(|s| {
                        s.last_message
                            .as_deref()
                            .or(Some(&s.title))
                            .map(str::to_owned)
                    })
            })
            .unwrap_or_else(|| slot.info.title.clone());
        if query.trim().is_empty() {
            self.view.related_sessions.clear();
            return;
        }
        let mut results = self.search_index.search(&query, 4);
        // Exclude the active session's own ID from results.
        if let Some(sid) = slot.info.session_id.as_ref() {
            results.retain(|(id, _)| id != sid);
        }
        results.truncate(3);
        self.view.related_sessions = results;
    }

    pub(crate) fn detect_file_conflicts(&mut self) {
        // Group running PTYs by workspace
        let mut ws_ptys: std::collections::HashMap<PathBuf, Vec<usize>> =
            std::collections::HashMap::new();
        for (i, slot) in self.ptys.ptys.iter().enumerate() {
            if !slot.info.completed {
                ws_ptys
                    .entry(slot.info.workspace_path.clone())
                    .or_default()
                    .push(i);
            }
        }

        let mut new_warnings: Vec<String> = Vec::new();
        for (_ws, indices) in ws_ptys {
            if indices.len() < 2 {
                continue;
            }
            // Collect changed files for each running PTY in this workspace
            let mut pty_files: Vec<(usize, Vec<String>)> = Vec::new();
            for &idx in &indices {
                let ws = &self.ptys.ptys[idx].info.workspace_path;
                let files: Vec<String> = git_cmd(ws, &["diff", "--name-only"])
                    .map(|s| {
                        s.lines()
                            .filter(|l| !l.is_empty())
                            .map(|l| l.to_string())
                            .collect()
                    })
                    .unwrap_or_default();
                pty_files.push((idx, files));
            }

            // Check for overlapping files between any two PTYs
            for a in 0..pty_files.len() {
                for b in (a + 1)..pty_files.len() {
                    let (_, files_a) = &pty_files[a];
                    let (_, files_b) = &pty_files[b];
                    let overlap: Vec<&String> =
                        files_a.iter().filter(|f| files_b.contains(f)).collect();
                    if !overlap.is_empty() {
                        let title_a = &self.ptys.ptys[pty_files[a].0].info.title;
                        let title_b = &self.ptys.ptys[pty_files[b].0].info.title;
                        let file_list = overlap
                            .iter()
                            .map(|f| f.as_str())
                            .collect::<Vec<_>>()
                            .join(", ");
                        new_warnings.push(format!(
                            "[{}] {} & {} both modifying: {}",
                            self.ptys.ptys[pty_files[a].0]
                                .info
                                .workspace_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("?"),
                            title_a,
                            title_b,
                            file_list
                        ));
                    }
                }
            }
        }

        // Only update if there are new conflicts (don't clear existing warnings that user hasn't seen)
        if !new_warnings.is_empty() {
            self.popup.conflict_warnings = new_warnings;
            if self.view.input_mode == InputMode::None {
                self.view.input_mode = InputMode::ConflictResolve;
            }
        }
    }

    /// Check cumulative token usage against configured budget limits.
    pub(crate) fn check_token_budget(&mut self) {
        let Some(ref budget) = self.token_budget else {
            return;
        };

        let now = crate::util::now_secs();
        let sessions = self.sessions.sessions.clone();
        let alert = crate::budget::check_budget(&sessions, budget, now, |session| {
            crate::discovery::find_session_jsonl(session)
                .and_then(|path| crate::discovery::extract_token_usage(&path))
        });

        if let Some(a) = alert {
            let was_none = self.popup.budget_alert.is_none();
            self.popup.budget_alert = Some(a.message.clone());
            // Auto-show budget warning popup on first detection
            if was_none && self.view.input_mode == InputMode::None {
                self.view.input_mode = InputMode::BudgetWarning;
            }
            // Send desktop notification on first detection
            if was_none {
                self.send_desktop_notification("amux: Budget Alert", &a.message);
            }
        } else {
            self.popup.budget_alert = None;
        }
    }

    /// Create git worktrees for all conflicting PTYs.
    pub(crate) fn isolate_conflicts(&mut self) {
        if !crate::worktree::git_available() {
            self.view.status = "Error: git is not installed or not on PATH.".into();
            return;
        }

        // Identify PTYs involved in conflicts
        let mut ws_ptys: std::collections::HashMap<PathBuf, Vec<usize>> =
            std::collections::HashMap::new();
        for (i, slot) in self.ptys.ptys.iter().enumerate() {
            if !slot.info.completed && slot.info.worktree_branch.is_none() {
                ws_ptys
                    .entry(slot.info.workspace_path.clone())
                    .or_default()
                    .push(i);
            }
        }

        let mut isolated = 0usize;
        let mut errors = Vec::new();

        for indices in ws_ptys.values() {
            if indices.len() < 2 {
                continue;
            }

            // Only isolate if there's no git repo or if all involved PTYs share the same workspace
            let ws = self.ptys.ptys[indices[0]].info.workspace_path.clone();
            if !crate::worktree::is_git_repo(&ws) {
                errors.push(format!("{}: not a git repository", ws.display()));
                continue;
            }

            // For each conflicting PTY (except the first), create a worktree
            for &idx in &indices[1..] {
                let slot = &self.ptys.ptys[idx];
                let has_branch = slot.info.worktree_branch.is_some();
                let slot_title = slot.info.title.clone();
                let slot_agent = slot.info.agent;
                let slot_session_id = slot.info.session_id.clone();
                if has_branch {
                    continue;
                }
                let branch = crate::worktree::branch_name(&slot_title, idx, self.ptys.pty_counter);
                let title = slot_title;
                match crate::worktree::create_worktree(&ws, &branch) {
                    Ok(worktree_path) => {
                        // Restart the PTY in the worktree directory
                        let agent = slot_agent;
                        let session_id = slot_session_id;
                        let chat_size = self.chat_size();
                        let env = self.project_env(&worktree_path);

                        // Try spawning a new PTY in the worktree
                        match crate::pty::PtyHandle::spawn(
                            agent,
                            &worktree_path,
                            session_id.as_deref(),
                            Some(&title),
                            chat_size,
                            &env,
                            &[],
                        ) {
                            Ok(new_pty) => {
                                // Unregister old PTY
                                self.unregister_pty(&self.ptys.ptys[idx].id);

                                // Replace the PTY slot in-place
                                let pty_id = self.next_pty_id();
                                self.ptys.ptys[idx] = PtySlot {
                                    id: pty_id.clone(),
                                    handle: new_pty,
                                    info: RunningInfo {
                                        workspace_path: worktree_path,
                                        title,
                                        session_id,
                                        started_at: crate::util::now_secs(),
                                        completed: false,
                                        agent,
                                        git_info: GitInfo::default(),
                                        check_status: CheckStatus::Pending,
                                        diff_summary: DiffSummary::default(),
                                        project_type: crate::discovery::ProjectType::detect(&ws),
                                        worktree_branch: Some(branch.clone()),
                                        snapshot_commit: None,
                                    },
                                    last_screen_hash: 0,
                                    last_recording_at: std::time::Instant::now(),
                                    process_stats: None,
                                };
                                self.register_pty(&pty_id, &self.ptys.ptys[idx]);
                                self.worktree_branches.push((ws.clone(), branch));
                                isolated += 1;
                            }
                            Err(e) => {
                                // Clean up the worktree if PTY spawn fails
                                let _ = crate::worktree::remove_worktree(&ws, &branch);
                                errors.push(format!(
                                    "{}: failed to restart in worktree: {}",
                                    title, e
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        errors.push(format!("{}: worktree creation failed: {}", title, e));
                    }
                }
            }
        }

        if isolated > 0 {
            self.view.status = format!("Isolated {} session(s) into worktrees.", isolated);
        } else if errors.is_empty() {
            self.view.status = "No conflicts to isolate.".into();
        }

        if !errors.is_empty() {
            self.view.status = format!("{} Errors: {}", self.view.status, errors.join("; "));
        }

        self.popup.conflict_warnings.clear();
        self.view.input_mode = InputMode::None;
    }

    /// Remove all worktrees created during this session.
    pub(crate) fn cleanup_worktrees(&mut self) {
        for (repo_path, branch) in self.worktree_branches.drain(..) {
            if let Err(e) = crate::worktree::remove_worktree(&repo_path, &branch) {
                eprintln!("warning: failed to clean up worktree {}: {}", branch, e);
            }
        }
    }

    pub(crate) fn rebuild_tree(&mut self) {
        let mut tree = Vec::new();
        let mut ws_map = Vec::new();
        let parsed = self
            .view
            .search_query
            .as_deref()
            .map(crate::util::parse_search_query);
        let query = parsed.as_ref().and_then(|p| p.text.as_deref());
        let date_cutoff = parsed.as_ref().and_then(|p| p.min_last_active);
        // Collect pinned session indices first — they go into a virtual "Pinned" workspace
        let pinned_idxs: Vec<usize> = self
            .sessions
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, s)| s.pinned)
            .map(|(i, _)| i)
            .collect();
        if !pinned_idxs.is_empty() {
            tree.push(TreeNode::PinnedWorkspace);
            if self.sessions.pinned_expanded {
                let mut sorted_pins = pinned_idxs;
                sorted_pins.sort_by(|&a, &b| {
                    self.sessions.sessions[b]
                        .last_active
                        .cmp(&self.sessions.sessions[a].last_active)
                });
                for &si in &sorted_pins {
                    let ws_path = &self.sessions.sessions[si].workspace_path;
                    let wi = self
                        .sessions
                        .workspaces
                        .iter()
                        .position(|w| {
                            w.path.as_deref() == Some(ws_path)
                                || w.path.as_ref().is_some_and(|p| ws_path.starts_with(p))
                        })
                        .unwrap_or(0);
                    tree.push(TreeNode::Session(wi, si));
                }
            }
        }
        // Collect recent sessions: non-pinned, non-active, sorted by last_active desc, top 10
        let active_session_ids: Vec<String> = self
            .ptys
            .ptys
            .iter()
            .filter_map(|slot| slot.info.session_id.clone())
            .collect();
        let mut recent_idxs: Vec<usize> = self
            .sessions
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                !s.pinned
                    && !active_session_ids.iter().any(|sid| sid == &s.id)
                    && s.last_active > 0
                    && {
                        // Only sessions active within the last 7 days
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        now.saturating_sub(s.last_active) < 7 * 24 * 3600
                    }
                    && self.view.agent_filter.is_none_or(|agent| s.agent == agent)
                    && self
                        .view
                        .tag_filter
                        .as_ref()
                        .is_none_or(|tag| s.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)))
                    && date_cutoff.is_none_or(|cutoff| s.last_active >= cutoff)
            })
            .map(|(i, _)| i)
            .collect();
        recent_idxs.sort_by(|&a, &b| {
            self.sessions.sessions[b]
                .last_active
                .cmp(&self.sessions.sessions[a].last_active)
        });
        recent_idxs.truncate(10);
        self.sessions.recent_count = recent_idxs.len();
        if !recent_idxs.is_empty() {
            tree.push(TreeNode::RecentWorkspace);
            if self.sessions.recent_expanded {
                for &si in &recent_idxs {
                    let ws_path = &self.sessions.sessions[si].workspace_path;
                    let wi = self
                        .sessions
                        .workspaces
                        .iter()
                        .position(|w| {
                            w.path.as_deref() == Some(ws_path)
                                || w.path.as_ref().is_some_and(|p| ws_path.starts_with(p))
                        })
                        .unwrap_or(0);
                    tree.push(TreeNode::Session(wi, si));
                }
            }
        }
        for (wi, _ws) in self.sessions.workspaces.iter().enumerate() {
            let sess_idxs: Vec<usize> = self
                .sessions
                .sessions
                .iter()
                .enumerate()
                .filter(|(_i, s)| {
                    self.ws_matches_path(wi, &s.workspace_path)
                            && !s.pinned  // pinned sessions shown in virtual workspace above
                            // recent sessions are ALSO shown in their normal workspaces (per spec)
                            && self.view.agent_filter.is_none_or(|agent| s.agent == agent)
                            && self.view.tag_filter.as_ref().is_none_or(|tag| {
                                s.tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
                            })
                            && date_cutoff.is_none_or(|cutoff| s.last_active >= cutoff)
                })
                .map(|(i, _)| i)
                .collect();
            if let Some(q) = query {
                // Fuzzy-filter sessions for this workspace
                let mut matching_sessions: Vec<usize> = sess_idxs
                    .into_iter()
                    .filter(|&si| {
                        let session = &self.sessions.sessions[si];
                        let short_id = &session.id[..session.id.len().min(8)];
                        session_fuzzy_score(session.title.as_str(), short_id, q)
                            || session_fuzzy_score(&self.sessions.workspaces[wi].name, short_id, q)
                    })
                    .collect();
                self.sort_session_indices(&mut matching_sessions);
                // Fuzzy-filter active PTYs for this workspace
                let matching_ptys: Vec<usize> = self
                    .ptys
                    .ptys
                    .iter()
                    .enumerate()
                    .filter(|(_pi, slot)| {
                        self.ws_matches_path(wi, &slot.info.workspace_path)
                            && slot.info.session_id.is_none()
                            && self.view.agent_filter.is_none_or(|a| slot.info.agent == a)
                            && session_fuzzy_score(&slot.info.title, &slot.info.title, q)
                    })
                    .map(|(pi, _)| pi)
                    .collect();
                // Include workspace only if it matches itself or has matching children
                let ws_matches = session_fuzzy_score(&self.sessions.workspaces[wi].name, "", q);
                if ws_matches || !matching_sessions.is_empty() || !matching_ptys.is_empty() {
                    tree.push(TreeNode::Workspace(wi));
                    if let Some(ref ws_path) = self.sessions.workspaces[wi].path
                        && !ws_path.exists()
                    {
                        tree.push(TreeNode::WorkspaceWarning(
                            wi,
                            format!(
                                "Path not found: {}. Update config.json or create the directory.",
                                ws_path.display()
                            ),
                        ));
                    }
                    for &pi in &matching_ptys {
                        tree.push(TreeNode::ActiveTab(pi));
                    }
                    if self.view.sort_mode == SortMode::AgentGroup {
                        Self::append_agent_grouped(
                            &self.sessions.sessions,
                            &matching_sessions,
                            wi,
                            &mut tree,
                        );
                    } else {
                        for &si in &matching_sessions {
                            tree.push(TreeNode::Session(wi, si));
                        }
                    }
                }
                ws_map.push(matching_sessions);
            } else {
                let mut sorted_idxs = sess_idxs.clone();
                self.sort_session_indices(&mut sorted_idxs);
                tree.push(TreeNode::Workspace(wi));
                if let Some(ref ws_path) = self.sessions.workspaces[wi].path
                    && !ws_path.exists()
                {
                    tree.push(TreeNode::WorkspaceWarning(
                        wi,
                        format!(
                            "Path not found: {}. Update config.json or create the directory.",
                            ws_path.display()
                        ),
                    ));
                }
                if self.sessions.workspaces[wi].expanded {
                    for (pi, slot) in self.ptys.ptys.iter().enumerate() {
                        if self.ws_matches_path(wi, &slot.info.workspace_path)
                            && slot.info.session_id.is_none()
                            && self.view.agent_filter.is_none_or(|a| slot.info.agent == a)
                        {
                            tree.push(TreeNode::ActiveTab(pi));
                        }
                    }
                    if self.view.sort_mode == SortMode::AgentGroup {
                        Self::append_agent_grouped(
                            &self.sessions.sessions,
                            &sorted_idxs,
                            wi,
                            &mut tree,
                        );
                    } else {
                        for &si in &sorted_idxs {
                            tree.push(TreeNode::Session(wi, si));
                        }
                    }
                }
                ws_map.push(sess_idxs);
            }
        }

        // Append archived section when toggled visible
        if self.sessions.show_archived && !self.sessions.archived_sessions.is_empty() {
            tree.push(TreeNode::ArchivedHeader);
            let mut archived_idxs: Vec<usize> =
                (0..self.sessions.archived_sessions.len()).collect();
            archived_idxs.sort_by(|&a, &b| {
                self.sessions.archived_sessions[b]
                    .last_active
                    .cmp(&self.sessions.archived_sessions[a].last_active)
            });
            for ai in archived_idxs {
                // Find workspace index for the archived session
                let ws_path = &self.sessions.archived_sessions[ai].workspace_path;
                let wi = self
                    .sessions
                    .workspaces
                    .iter()
                    .position(|w| {
                        w.path.as_deref() == Some(ws_path)
                            || w.path.as_ref().is_some_and(|p| ws_path.starts_with(p))
                    })
                    .unwrap_or(0);
                tree.push(TreeNode::ArchivedSession(wi, ai));
            }
        }

        self.sessions.tree = tree;
        self.sessions.ws_session_map = ws_map;

        // Clamp selection to valid range
        if !self.sessions.tree.is_empty() {
            self.move_sel(0);
        }
    }

    pub(crate) fn move_sel(&mut self, delta: isize) {
        let len = self.sessions.tree.len();
        if len == 0 {
            return;
        }
        let cur = self
            .sessions
            .tree_state
            .selected()
            .unwrap_or(0)
            .min(len - 1) as isize;
        self.sessions
            .tree_state
            .select(Some(((cur + delta).rem_euclid(len as isize)) as usize));
    }

    /// Navigate the sidebar tree to select a specific session by ID.
    pub(crate) fn navigate_to_session(&mut self, session_id: &str) {
        // Find the tree index for the session.
        let tree_idx = self.sessions.tree.iter().position(|node| {
            if let TreeNode::Session(_wi, si) = node {
                self.sessions
                    .sessions
                    .get(*si)
                    .is_some_and(|s| s.id == session_id)
            } else {
                false
            }
        });
        if let Some(idx) = tree_idx {
            self.sessions.tree_state.select(Some(idx));
            self.view.status = format!(
                "Selected session {}.",
                &session_id[..8.min(session_id.len())]
            );
        } else {
            self.view.status = format!(
                "Session {} not found in sidebar.",
                &session_id[..8.min(session_id.len())]
            );
        }
    }

    pub(crate) fn toggle_expand(&mut self) {
        match self.selected_node() {
            Some(TreeNode::Workspace(wi)) => {
                let wi = *wi;
                self.sessions.workspaces[wi].expanded = !self.sessions.workspaces[wi].expanded;
                self.rebuild_tree();
            }
            Some(TreeNode::PinnedWorkspace) => {
                self.sessions.pinned_expanded = !self.sessions.pinned_expanded;
                self.rebuild_tree();
            }
            Some(TreeNode::RecentWorkspace) => {
                self.sessions.recent_expanded = !self.sessions.recent_expanded;
                self.rebuild_tree();
            }
            _ => {}
        }
    }

    pub(crate) fn request_delete(&mut self) {
        let node = self.selected_node().cloned();
        match &node {
            Some(TreeNode::Workspace(wi)) => {
                let name = self.sessions.workspaces[*wi].name.clone();
                let session_count = self
                    .sessions
                    .ws_session_map
                    .get(*wi)
                    .map(|v| v.len())
                    .unwrap_or(0);
                self.view.status = format!(
                    "Delete workspace \"{}\" ({} sessions)? y/n",
                    name, session_count
                );
                self.pending_delete = node;
                self.view.input_mode = InputMode::ConfirmDelete;
            }
            Some(TreeNode::Session(_wi, _si)) => {
                if !self.view.selected_set.is_empty() {
                    // Batch delete all marked sessions
                    let count = self.view.selected_set.len();
                    self.view.status = format!("Delete {} marked session(s)? y/n", count);
                    self.pending_batch_delete = true;
                    self.view.input_mode = InputMode::ConfirmDelete;
                } else {
                    if let Some(TreeNode::Session(_, si)) = node.as_ref() {
                        if *si >= self.sessions.sessions.len() {
                            return;
                        }
                        let title = self.sessions.sessions[*si].title.clone();
                        self.view.status = format!("Delete session \"{}\"? y/n", title);
                        self.pending_delete = node;
                        self.view.input_mode = InputMode::ConfirmDelete;
                    }
                }
            }
            Some(TreeNode::ActiveTab(pi)) => {
                // Closing a tab doesn't destroy data, no confirmation needed
                let title = self
                    .ptys
                    .ptys
                    .get(*pi)
                    .map(|s| s.info.title.clone())
                    .unwrap_or_default();
                if let Some(slot) = self.ptys.ptys.get(*pi) {
                    self.unregister_pty(&slot.id);
                }
                self.ptys.ptys.remove(*pi);
                if let Some(cur) = self.ptys.active_pty
                    && cur >= self.ptys.ptys.len()
                {
                    self.ptys.active_pty = if self.ptys.ptys.is_empty() {
                        None
                    } else {
                        Some(self.ptys.ptys.len() - 1)
                    };
                }
                if self.ptys.ptys.is_empty() {
                    self.view.focus = Focus::Sidebar;
                }
                self.rebuild_tree();
                self.view.status = format!("Closed tab: {}", title);
            }
            _ => {}
        }
    }

    pub(crate) fn confirm_delete(&mut self) {
        self.view.input_mode = InputMode::None;

        if self.pending_batch_delete {
            // Batch delete all marked sessions
            let count = self.view.selected_set.len();
            // Sort descending so indices stay valid as we remove
            let mut to_delete: Vec<usize> = self.view.selected_set.iter().copied().collect();
            to_delete.sort_by(|a, b| b.cmp(a));
            for si in to_delete {
                if si >= self.sessions.sessions.len() {
                    continue;
                }
                let session = self.sessions.sessions[si].clone();
                if let Some(pi) = self.pty_index_for_session(&session.id) {
                    if let Some(slot) = self.ptys.ptys.get(pi) {
                        self.unregister_pty(&slot.id);
                    }
                    self.ptys.ptys.remove(pi);
                }
                let title_path = title_override_path(&session.id);
                let _ = fs::remove_file(&title_path);
                if let Some(jsonl) = find_session_jsonl(&session) {
                    let _ = fs::remove_file(&jsonl);
                }
                self.sessions.sessions.remove(si);
            }
            if let Some(cur) = self.ptys.active_pty
                && cur >= self.ptys.ptys.len()
            {
                self.ptys.active_pty = if self.ptys.ptys.is_empty() {
                    None
                } else {
                    Some(self.ptys.ptys.len() - 1)
                };
            }
            if self.ptys.ptys.is_empty() {
                self.view.focus = Focus::Sidebar;
            }
            self.view.selected_set.clear();
            self.pending_batch_delete = false;
            self.rebuild_tree();
            self.view.status = format!("Deleted {} session(s)", count);
            return;
        }

        let node = self.pending_delete.take();
        match node {
            Some(TreeNode::Workspace(wi)) => {
                let name = self.sessions.workspaces[wi].name.clone();
                self.sessions.workspaces.remove(wi);
                self.save_config();
                self.refresh_sessions();
                self.view.status = format!("Deleted workspace: {}", name);
            }
            Some(TreeNode::Session(_wi, si)) => {
                if si >= self.sessions.sessions.len() {
                    return;
                }
                let session = self.sessions.sessions[si].clone();
                if let Some(pi) = self.pty_index_for_session(&session.id) {
                    if let Some(slot) = self.ptys.ptys.get(pi) {
                        self.unregister_pty(&slot.id);
                    }
                    self.ptys.ptys.remove(pi);
                    if let Some(cur) = self.ptys.active_pty
                        && cur >= self.ptys.ptys.len()
                    {
                        self.ptys.active_pty = if self.ptys.ptys.is_empty() {
                            None
                        } else {
                            Some(self.ptys.ptys.len() - 1)
                        };
                    }
                }
                let title_path = title_override_path(&session.id);
                let _ = fs::remove_file(&title_path);
                if let Some(jsonl) = find_session_jsonl(&session) {
                    let _ = fs::remove_file(&jsonl);
                }
                let title = session.title;
                self.sessions.sessions.remove(si);
                self.rebuild_tree();
                self.view.status = format!("Deleted session: {}", title);
            }
            _ => {}
        }
    }

    pub(crate) fn cancel_delete(&mut self) {
        self.pending_delete = None;
        self.pending_batch_delete = false;
        self.view.input_mode = InputMode::None;
        self.view.status.clear();
    }

    pub(crate) fn toggle_selection(&mut self) {
        if let Some(TreeNode::Session(_wi, si)) = self.selected_node().cloned() {
            if self.view.selected_set.contains(&si) {
                self.view.selected_set.remove(&si);
                self.view.status = format!("Unmarked session ({})", self.view.selected_set.len());
            } else {
                self.view.selected_set.insert(si);
                self.view.status = format!("Marked session ({})", self.view.selected_set.len());
            }
        }
    }

    pub(crate) fn start_session_preview(&mut self) {
        let node = self.selected_node().cloned();
        if let Some(TreeNode::Session(_wi, si)) = node {
            if si >= self.sessions.sessions.len() {
                return;
            }
            let session = self.sessions.sessions[si].clone();
            if let Some(jsonl_path) = find_session_jsonl(&session) {
                if let Some(lines) = preview_session_content(&jsonl_path, 5) {
                    self.popup.preview_lines = lines;
                    self.popup.preview_show_summary = false;
                    self.popup.preview_session_id = Some(session.id.clone());
                    self.view.input_mode = InputMode::SessionPreview;
                    self.view.status = format!(
                        "Preview: {} (s=summary  k=knowledge  any key=close)",
                        session.title
                    );
                } else {
                    self.view.status = "No preview available.".into();
                }
            } else {
                self.view.status = "Session file not found.".into();
            }
        }
    }

    /// Load the summary file for the currently-previewed session into preview_lines.
    pub(crate) fn load_preview_summary(&mut self) {
        if let Some(ref sid) = self.popup.preview_session_id {
            let short_id = &sid[..sid.len().min(16)];
            let path = crate::config::data_dir()
                .join("summaries")
                .join(format!("{}.md", short_id));
            if let Ok(content) = std::fs::read_to_string(&path) {
                self.popup.preview_lines = content
                    .lines()
                    .map(|l| PreviewLine {
                        role: if l.starts_with('#') {
                            "heading"
                        } else {
                            "text"
                        }
                        .to_string(),
                        text: l.to_string(),
                    })
                    .collect();
                self.view.status = "Summary view (s=content  k=knowledge  any key=close)".into();
            } else {
                self.popup.preview_lines = vec![PreviewLine {
                    role: "text".into(),
                    text: "No summary available.".into(),
                }];
                self.view.status = "No summary file found for this session.".into();
            }
        }
    }

    /// Reload the JSONL content for the currently-previewed session.
    pub(crate) fn reload_preview_content(&mut self) {
        if let Some(ref sid) = self.popup.preview_session_id
            && let Some(session) = self.sessions.sessions.iter().find(|s| s.id == *sid)
            && let Some(jsonl_path) = find_session_jsonl(session)
            && let Some(lines) = preview_session_content(&jsonl_path, 5)
        {
            self.popup.preview_lines = lines;
            self.view.status = format!(
                "Preview: {} (s=summary  k=knowledge  any key=close)",
                session.title
            );
            return;
        }
        self.view.status = "Could not reload content.".into();
    }

    /// Load workspace knowledge into preview_lines for the knowledge view.
    pub(crate) fn load_knowledge_preview(&mut self) {
        let ws_path = self
            .popup
            .preview_session_id
            .as_ref()
            .and_then(|sid| self.sessions.sessions.iter().find(|s| s.id == *sid))
            .map(|s| s.workspace_path.clone());
        let Some(ws_path) = ws_path else {
            self.popup.preview_lines = vec![PreviewLine {
                role: "text".into(),
                text: "No session selected.".into(),
            }];
            self.view.status = "Knowledge: no workspace context.".into();
            return;
        };
        let knowledge = crate::knowledge::load_knowledge(&ws_path);
        let mut lines: Vec<PreviewLine> = Vec::new();
        lines.push(PreviewLine {
            role: "heading".into(),
            text: "## Knowledge Base".into(),
        });
        lines.push(PreviewLine {
            role: "text".into(),
            text: String::new(),
        });
        if !knowledge.architecture.is_empty() {
            lines.push(PreviewLine {
                role: "heading".into(),
                text: "### Architecture".into(),
            });
            for l in knowledge.architecture.lines() {
                lines.push(PreviewLine {
                    role: "text".into(),
                    text: format!("  {}", l),
                });
            }
            lines.push(PreviewLine {
                role: "text".into(),
                text: String::new(),
            });
        }
        if !knowledge.key_files.is_empty() {
            lines.push(PreviewLine {
                role: "heading".into(),
                text: "### Key Files".into(),
            });
            for f in &knowledge.key_files {
                lines.push(PreviewLine {
                    role: "text".into(),
                    text: format!("  • {}", f),
                });
            }
            lines.push(PreviewLine {
                role: "text".into(),
                text: String::new(),
            });
        }
        if !knowledge.tech_stack.is_empty() {
            lines.push(PreviewLine {
                role: "heading".into(),
                text: "### Tech Stack".into(),
            });
            lines.push(PreviewLine {
                role: "text".into(),
                text: format!("  {}", knowledge.tech_stack.join(", ")),
            });
            lines.push(PreviewLine {
                role: "text".into(),
                text: String::new(),
            });
        }
        if !knowledge.known_issues.is_empty() {
            lines.push(PreviewLine {
                role: "heading".into(),
                text: "### Known Issues".into(),
            });
            for issue in &knowledge.known_issues {
                lines.push(PreviewLine {
                    role: "text".into(),
                    text: format!("  • {}", issue),
                });
            }
            lines.push(PreviewLine {
                role: "text".into(),
                text: String::new(),
            });
        }
        if let Some(ref ts) = knowledge.last_updated {
            lines.push(PreviewLine {
                role: "text".into(),
                text: format!("  Last updated: {}", ts),
            });
        }
        if lines.len() <= 2 {
            lines.push(PreviewLine {
                role: "text".into(),
                text: "  (empty — no knowledge accumulated yet)".into(),
            });
        }
        self.popup.preview_lines = lines;
        self.view.status = "Knowledge (k=back, c=clear, any key=close)".into();
    }

    /// Clear the knowledge base for the current workspace.
    pub(crate) fn clear_workspace_knowledge(&mut self) {
        let ws_path = self
            .popup
            .preview_session_id
            .as_ref()
            .and_then(|sid| self.sessions.sessions.iter().find(|s| s.id == *sid))
            .map(|s| s.workspace_path.clone());
        if let Some(ref ws_path) = ws_path {
            let empty = crate::knowledge::WorkspaceKnowledge::default();
            let _ = crate::knowledge::save_knowledge(ws_path, &empty);
            self.load_knowledge_preview();
            self.view.status = "Knowledge cleared.".into();
        }
    }

    pub(crate) fn export_selected_session(&mut self) {
        let node = self.selected_node().cloned();
        if let Some(TreeNode::Session(_wi, si)) = node {
            if si >= self.sessions.sessions.len() {
                return;
            }
            let session = self.sessions.sessions[si].clone();
            if let Some(jsonl_path) = crate::discovery::find_session_jsonl(&session) {
                let export_dir = crate::config::data_dir().join("exports");
                match crate::discovery::export_session_to_markdown(
                    &jsonl_path,
                    &session.title,
                    &export_dir,
                ) {
                    Ok(path) => {
                        self.view.status = format!("Exported to: {}", path.display());
                    }
                    Err(e) => {
                        self.view.status = format!("Export failed: {}", e);
                    }
                }
            } else {
                self.view.status = "Session file not found.".into();
            }
        }
    }

    pub(crate) fn copy_selected_info(&mut self) {
        let node = self.selected_node().cloned();
        match node {
            Some(TreeNode::Session(_wi, si)) if si < self.sessions.sessions.len() => {
                let session = &self.sessions.sessions[si];
                let text = format!(
                    "[{}] {} ({})",
                    &session.id[..session.id.len().min(8)],
                    session.title,
                    session.agent.label()
                );
                match clipboard_copy(&text) {
                    Ok(()) => self.view.status = format!("Copied: {}", text),
                    Err(e) => self.view.status = format!("Copy failed: {}", e),
                }
            }
            Some(TreeNode::ActiveTab(pi)) => {
                if let Some(slot) = self.ptys.ptys.get(pi) {
                    let text = format!("{} ({})", slot.info.title, slot.info.agent.label());
                    match clipboard_copy(&text) {
                        Ok(()) => self.view.status = format!("Copied: {}", text),
                        Err(e) => self.view.status = format!("Copy failed: {}", e),
                    }
                }
            }
            Some(TreeNode::Workspace(wi)) if wi < self.sessions.workspaces.len() => {
                let ws = &self.sessions.workspaces[wi];
                let path = ws
                    .path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "virtual".into());
                let text = format!("{} ({})", ws.name, path);
                match clipboard_copy(&text) {
                    Ok(()) => self.view.status = format!("Copied: {}", text),
                    Err(e) => self.view.status = format!("Copy failed: {}", e),
                }
            }
            _ => {
                self.view.status = "Nothing to copy.".into();
            }
        }
    }

    pub(crate) fn save_config(&self) {
        let config = Config {
            workspaces: self.sessions.workspaces.clone(),
            theme: self.view.theme_name.clone(),
            keybinds: self.view.keybinds.clone(),
            templates: self.templates.clone(),
            automations: self.automations.clone(),
            archive_days: self.sessions.archive_days,
            remote_hosts: self.remote_hosts.clone(),
            plugins: self.plugins.clone(),
            serve_port: None,
            serve_token: None,
            check_command: self.check_command.clone(),
            token_budget: self.token_budget.clone(),
            chains: self.chains.chains.clone(),
            unset_env: Vec::new(),
        };
        if let Err(e) = save_config_file(&config) {
            eprintln!("Failed to save config: {}", e);
        }
    }

    pub(crate) fn start_branch(&mut self) {
        let node = self.selected_node();
        let session = match node {
            Some(TreeNode::Session(_wi, si)) => self.sessions.sessions.get(*si).cloned(),
            _ => None,
        };
        let Some(session) = session else {
            self.view.status = "Select a session to branch from.".into();
            return;
        };

        let jsonl_path = find_session_jsonl(&session);
        let Some(jsonl_path) = jsonl_path else {
            self.view.status = "Cannot find session JSONL file.".into();
            return;
        };

        match extract_branch_points(&jsonl_path) {
            Some(points) if points.is_empty() => {
                self.view.status = "No user messages found in this session.".into();
            }
            Some(points) => {
                self.popup.branch_points = points;
                self.branch_state.select(Some(0));
                self.view.input_mode = InputMode::BranchSelect;
                self.view.status = "Select branch point (Enter=branch, Esc=cancel)".into();
            }
            None => {
                self.view.status = "Failed to read session data.".into();
            }
        }
    }

    pub(crate) fn start_diff(&mut self) {
        use crate::discovery::{compute_diff, extract_session_output, find_session_jsonl};

        let node = self.selected_node();
        let session_idx = match node {
            Some(TreeNode::Session(_wi, si)) => *si,
            _ => {
                self.view.status = "Select a session to diff.".into();
                return;
            }
        };

        if let Some(left_idx) = self.popup.diff_left_session {
            // Second session selected — compute diff
            if left_idx == session_idx {
                self.view.status = "Cannot diff a session with itself.".into();
                self.popup.diff_left_session = None;
                return;
            }

            let left_session = self.sessions.sessions.get(left_idx).cloned();
            let right_session = self.sessions.sessions.get(session_idx).cloned();
            let Some(left_session) = left_session else {
                self.view.status = "First session no longer available.".into();
                self.popup.diff_left_session = None;
                return;
            };
            let Some(right_session) = right_session else {
                self.view.status = "Second session no longer available.".into();
                self.popup.diff_left_session = None;
                return;
            };

            let left_jsonl = find_session_jsonl(&left_session);
            let right_jsonl = find_session_jsonl(&right_session);

            let left_output = left_jsonl
                .as_ref()
                .and_then(|p| extract_session_output(p))
                .unwrap_or_default();
            let right_output = right_jsonl
                .as_ref()
                .and_then(|p| extract_session_output(p))
                .unwrap_or_default();

            self.popup.diff_lines = compute_diff(&left_output, &right_output);
            self.view.input_mode = InputMode::DiffView;
            self.popup.diff_left_session = None;
            self.view.status = "Session Diff (any key to close)".into();
        } else {
            // First session selected
            self.popup.diff_left_session = Some(session_idx);
            let session = &self.sessions.sessions[session_idx];
            self.view.status = format!(
                "Diff: selected '{}' — select second session, press X again",
                &session.title[..session.title.len().min(30)]
            );
        }
    }

    pub(crate) fn flush_pending_inputs(&mut self) {
        if self.ptys.pending_inputs.is_empty() {
            return;
        }
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX);
        let mut i = 0;
        while i < self.ptys.pending_inputs.len() {
            if self.ptys.pending_inputs[i].fire_at_ms <= now_ms {
                let step = self.ptys.pending_inputs.remove(i);
                if let Some(pi) = self.ptys.active_pty
                    && let Some(slot) = self.ptys.ptys.get(pi)
                {
                    let data = format!("{}\n", step.text);
                    if let Err(e) = slot.handle.write_input(data.as_bytes()) {
                        self.view.status = format!("Write error: {e}");
                    }
                }
            } else {
                i += 1;
            }
        }
    }

    /// Open the selected session's workspace directory in the system file manager.
    pub(crate) fn open_workspace_dir(&mut self) {
        let node = self.selected_node();
        let ws_path: Option<PathBuf> = match &node {
            Some(TreeNode::Session(wi, _)) => self
                .sessions
                .workspaces
                .get(*wi)
                .and_then(|ws| ws.path.clone()),
            Some(TreeNode::ArchivedSession(wi, _)) => self
                .sessions
                .workspaces
                .get(*wi)
                .and_then(|ws| ws.path.clone()),
            Some(TreeNode::Workspace(wi)) => self
                .sessions
                .workspaces
                .get(*wi)
                .and_then(|ws| ws.path.clone()),
            _ => None,
        };
        let Some(path) = ws_path else {
            self.view.status = "No workspace path available.".into();
            return;
        };
        if !path.exists() {
            self.view.status = format!("Path not found: {}", path.display());
            return;
        }
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        match std::process::Command::new(opener).arg(&path).spawn() {
            Ok(_) => self.view.status = format!("Opened {}", path.display()),
            Err(e) => self.view.status = format!("Failed to open: {}", e),
        }
    }

    pub(crate) fn open_theme_panel(&mut self) {
        let mut themes = vec![
            crate::theme::ThemeName::Dark,
            crate::theme::ThemeName::Light,
            crate::theme::ThemeName::Mocha,
            crate::theme::ThemeName::TokyoNight,
        ];
        if let Some(customs) = crate::theme::discover_custom_themes() {
            themes.extend(customs);
        }
        let sel = themes
            .iter()
            .position(|t| t == &self.view.theme_name)
            .unwrap_or(0);
        self.theme_list = themes;
        self.theme_list_state.select(Some(sel));
        self.view.input_mode = InputMode::ThemeSelect;
        self.view.picker_query.clear();
        self.view.status = "Select theme (Enter=apply, Esc=cancel)".into();
    }

    pub(crate) fn activate_selection(&mut self) -> Result<()> {
        match self.selected_node().cloned() {
            Some(TreeNode::Workspace(_)) => {
                self.view.input_mode = InputMode::SessionName;
                self.input_buffer.clear();
                self.view.status = "Enter session name (empty = unnamed, Esc = cancel):".into();
            }
            Some(TreeNode::Session(_wi, si)) => {
                let session = self.sessions.sessions[si].clone();
                self.spawn_with_agent(session.agent, None)?;
            }
            Some(TreeNode::ActiveTab(pi)) => {
                self.ptys.active_pty = Some(pi);
                self.view.focus = Focus::Chat;
                self.update_related_sessions();
            }
            Some(TreeNode::AgentHeader(_)) => {}
            Some(TreeNode::PinnedWorkspace) => {}
            Some(TreeNode::RecentWorkspace) => {}
            Some(TreeNode::ArchivedHeader) => {}
            Some(TreeNode::WorkspaceWarning(_, _)) => {}
            Some(TreeNode::ArchivedSession(_wi, ai))
                if ai < self.sessions.archived_sessions.len() =>
            {
                let session = self.sessions.archived_sessions[ai].clone();
                self.spawn_with_agent(session.agent, None)?;
            }
            Some(TreeNode::ArchivedSession(_, _)) => {}
            None => {}
        }
        Ok(())
    }

    /// Ctrl+Click on PTY area: extract URL from the clicked line and open it.
    pub(crate) fn ctrl_click_open(&mut self, col: u16, row: u16) {
        let rect = self.view.last_chat_area;
        if col < rect.x
            || col >= rect.x + rect.width
            || row < rect.y + 1
            || row >= rect.y + rect.height
        {
            return;
        }
        let Some(idx) = self.ptys.active_pty else {
            return;
        };
        let Some(slot) = self.ptys.ptys.get(idx) else {
            return;
        };

        let (term_rows, _term_cols) = slot.handle.grid_size();
        let pty_row = (row - rect.y).saturating_sub(1);
        if pty_row as usize >= term_rows {
            return;
        }
        let mut line = String::new();
        for c in 0..rect.width.saturating_sub(2) {
            match slot.handle.cell_contents(pty_row as usize, c as usize) {
                Some(t) => line.push_str(&t),
                None => line.push(' '),
            }
        }

        let click_in_line = (col - rect.x) as usize;
        if let Some(url) = extract_url_from_line(&line, click_in_line) {
            let opener = if cfg!(target_os = "macos") {
                "open"
            } else {
                "xdg-open"
            };
            match std::process::Command::new(opener).arg(&url).spawn() {
                Ok(_) => self.view.status = format!("Opened {}", url),
                Err(e) => self.view.status = format!("Failed to open: {}", e),
            }
        }
    }

    pub(crate) fn poll_states(&mut self) {
        // Track when status string changes for auto-expire in status bar
        // Track when status string changes for auto-expire in status bar
        if self.view.status != self.view.prev_status {
            self.view.prev_status = self.view.status.clone();
            self.view.status_set_at = std::time::Instant::now();
        }
        let mut notification = None;
        let pty_count = self.ptys.ptys.len();
        // Deferred chain step to execute after the PTY loop (borrow checker)
        let mut chain_step: Option<crate::chain::ActiveChain> = None;
        let mut chain_completed = false;
        let mut any_screen_changed = false;
        // Pre-extract chain data so we don't borrow self inside iter_mut
        let pre_chain = self.chains.active_chain.clone();
        let pre_session_map: std::collections::HashMap<String, PathBuf> =
            if !self.ptys.ptys.is_empty() && self.chains.active_chain.is_some() {
                self.sessions
                    .sessions
                    .iter()
                    .filter_map(|s| {
                        crate::discovery::find_session_jsonl(s).map(|p| (s.id.clone(), p))
                    })
                    .collect()
            } else {
                std::collections::HashMap::new()
            };
        // Pre-extract global check command for use inside iter_mut
        let global_check = self.check_command.clone();
        for (i, slot) in self.ptys.ptys.iter_mut().enumerate() {
            let state = slot.handle.state();
            if state == PtyState::Running {
                slot.info.completed = false;
                if slot.record_screen_frame() {
                    any_screen_changed = true;
                }
            } else if !slot.info.completed && state == PtyState::Completed {
                slot.info.completed = true;
                any_screen_changed = true;

                if self.ptys.prev_states.get(i) == Some(&PtyState::Running) {
                    notification = Some(format!(
                        "✔ {} completed ({}/{})",
                        slot.info.title,
                        i + 1,
                        pty_count
                    ));
                    // Save recording metadata
                    save_recording_meta(slot);
                    // Record git state + diff summary
                    collect_git_info(slot);
                    // Save snapshot_commit to session meta for rollback support
                    if let Some(ref session_id) = slot.info.session_id
                        && let Some(ref snapshot) = slot.info.snapshot_commit
                    {
                        let _ = crate::config::save_snapshot_meta(session_id, snapshot);
                    }
                    // Auto-generate session summary (saved to file, no popup)
                    if let Some(ref session_id) = slot.info.session_id
                        && let Some(session) =
                            self.sessions.sessions.iter().find(|s| &s.id == session_id)
                        && let Some(summary) = crate::discovery::generate_session_summary(session)
                    {
                        let summary_dir = crate::config::data_dir().join("summaries");
                        let _ = std::fs::create_dir_all(&summary_dir);
                        let short_id = &session_id[..session_id.len().min(16)];
                        let path = summary_dir.join(format!("{}.md", short_id));
                        let _ = std::fs::write(&path, &summary);
                    }
                    // Merge session knowledge into workspace knowledge base
                    merge_session_knowledge(slot);
                    // Execute on_complete hook plugins
                    run_completion_hooks(slot, &self.plugins);
                    // Chain continuation: pre-extracted data (can't borrow self inside iter_mut)
                    if let Some(ref pc) = pre_chain
                        && slot.info.completed
                    {
                        if pc.current_step + 1 < pc.total_steps {
                            let prev_output = slot
                                .info
                                .session_id
                                .as_ref()
                                .and_then(|sid| pre_session_map.get(sid.as_str()))
                                .and_then(|path| crate::discovery::extract_session_output(path));
                            let mut updated = pc.clone();
                            updated.prev_output = prev_output;
                            updated.current_step += 1;
                            chain_step = Some(updated);
                        } else {
                            chain_completed = true;
                        }
                    }
                    // Post-completion check
                    let check_override = global_check.clone().or_else(|| {
                        self.sessions
                            .project_configs
                            .get(&slot.info.workspace_path)
                            .and_then(|pc| pc.check_command.clone())
                    });
                    spawn_completion_check(i, slot, check_override);
                }
            }
        }

        // Execute deferred chain step (borrow checker requires this outside iter_mut)
        self.execute_chain_step(chain_step, chain_completed);

        // Poll check results from background threads
        for i in 0..self.ptys.ptys.len() {
            let marker = crate::config::data_dir().join(format!(".check-result-{}", i));
            if marker.exists() {
                if let Ok(content) = std::fs::read_to_string(&marker)
                    && let Ok(status) = serde_json::from_str::<CheckStatus>(&content)
                {
                    self.ptys.ptys[i].info.check_status = status;
                }
                let _ = std::fs::remove_file(&marker);
            }
        }

        let before = self.ptys.ptys.len();
        // Unregister dead PTYs from shared state before retaining only live ones.
        for slot in &self.ptys.ptys {
            if !slot.handle.is_alive() {
                self.unregister_pty(&slot.id);
            }
        }
        self.ptys.ptys.retain(|slot| slot.handle.is_alive());
        if self.ptys.ptys.len() != before {
            if let Some(cur) = self.ptys.active_pty
                && cur >= self.ptys.ptys.len()
            {
                self.ptys.active_pty = if self.ptys.ptys.is_empty() {
                    None
                } else {
                    Some(self.ptys.ptys.len() - 1)
                };
            }
            self.rebuild_tree();
        }

        if let Some(ref msg) = notification {
            self.view.status = msg.clone();
            self.send_desktop_notification("amux", msg);
        }

        self.ptys.prev_states = self.ptys.ptys.iter().map(|s| s.handle.state()).collect();
        // Auto-close terminal split when shell exits
        // Auto-close terminal split when shell exits
        if let Some(term) = &self.terminal
            && term.handle.state() == PtyState::Completed
        {
            self.terminal = None;
            self.view.screen_changed = true;
        }
        // Detect terminal split screen changes
        if let Some(term) = &mut self.terminal
            && term.handle.state() == PtyState::Running
        {
            use std::hash::{Hash, Hasher};
            let content = term.handle.screen_contents();
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            content.hash(&mut hasher);
            let hash = hasher.finish();
            if hash != term.last_screen_hash {
                term.last_screen_hash = hash;
                any_screen_changed = true;
            }
        }
        if any_screen_changed {
            self.view.screen_changed = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::app::tests::{sess, sess_with_agent, sess_with_time, test_app, ws};
    use crate::types::{Agent, SortMode, TreeNode};

    // ── Test 1: cycle_sort_mode cycles through all variants ──
    #[test]
    fn cycle_sort_mode_cycles_through_all_variants() {
        let mut app = test_app(vec![], vec![]);
        assert_eq!(app.view.sort_mode, SortMode::TimeDesc);

        app.cycle_sort_mode();
        assert_eq!(app.view.sort_mode, SortMode::TimeAsc);
        assert!(app.view.status.contains("time"));

        app.cycle_sort_mode();
        assert_eq!(app.view.sort_mode, SortMode::NameAsc);

        app.cycle_sort_mode();
        assert_eq!(app.view.sort_mode, SortMode::NameDesc);

        app.cycle_sort_mode();
        assert_eq!(app.view.sort_mode, SortMode::AgentGroup);

        // Wraps back to the first variant
        app.cycle_sort_mode();
        assert_eq!(app.view.sort_mode, SortMode::TimeDesc);
    }

    // ── Test 2: sort_session_indices with various sort modes ──
    #[test]
    fn sort_session_indices_orders_by_time_desc() {
        let mut app = test_app(
            vec![ws("w1", "ws", "/tmp/ws1")],
            vec![
                sess_with_time("s1", "alpha", "/tmp/ws1", 100),
                sess_with_time("s2", "bravo", "/tmp/ws1", 300),
                sess_with_time("s3", "charlie", "/tmp/ws1", 200),
            ],
        );
        app.view.sort_mode = SortMode::TimeDesc;
        let mut indices = vec![0, 1, 2];
        app.sort_session_indices(&mut indices);
        // Most recent first: s2(300), s3(200), s1(100)
        assert_eq!(indices, vec![1, 2, 0]);
    }

    #[test]
    fn sort_session_indices_orders_by_name_asc() {
        let mut app = test_app(
            vec![ws("w1", "ws", "/tmp/ws1")],
            vec![
                sess("s1", "charlie", "/tmp/ws1"),
                sess("s2", "alpha", "/tmp/ws1"),
                sess("s3", "bravo", "/tmp/ws1"),
            ],
        );
        app.view.sort_mode = SortMode::NameAsc;
        let mut indices = vec![0, 1, 2];
        app.sort_session_indices(&mut indices);
        assert_eq!(indices, vec![1, 2, 0]); // alpha, bravo, charlie
    }

    // ── Test 3: toggle_agent_filter adds and removes agents ──
    #[test]
    fn toggle_agent_filter_adds_and_removes() {
        let mut app = test_app(
            vec![ws("w1", "ws", "/tmp/ws1")],
            vec![
                sess_with_agent("s1", "a", "/tmp/ws1", Agent::Claude),
                sess_with_agent("s2", "b", "/tmp/ws1", Agent::Codex),
            ],
        );
        // No filter initially
        assert!(app.view.agent_filter.is_none());

        // Set filter to Claude
        app.toggle_agent_filter(Agent::Claude);
        assert_eq!(app.view.agent_filter, Some(Agent::Claude));
        assert!(app.view.status.contains("Claude"));
        // Only Claude sessions in tree
        let session_nodes: Vec<_> = app
            .sessions
            .tree
            .iter()
            .filter(|n| matches!(n, TreeNode::Session(_, _)))
            .collect();
        assert_eq!(session_nodes.len(), 1);

        // Toggle same agent again — clears filter
        app.toggle_agent_filter(Agent::Claude);
        assert!(app.view.agent_filter.is_none());
        assert!(app.view.status.contains("all agents"));
        // All sessions visible again
        let session_nodes: Vec<_> = app
            .sessions
            .tree
            .iter()
            .filter(|n| matches!(n, TreeNode::Session(_, _)))
            .collect();
        assert_eq!(session_nodes.len(), 2);
    }

    // ── Test 4: move_sel clamps at boundaries and wraps ──
    #[test]
    fn move_sel_wraps_at_boundaries() {
        let mut app = test_app(
            vec![ws("w1", "ws", "/tmp/ws1")],
            vec![sess("s1", "a", "/tmp/ws1")],
        );
        // Tree: [Workspace(0), WorkspaceWarning(0,..), Session(0,0)] — 3 nodes
        let len = app.sessions.tree.len();
        assert!(len >= 3);

        // Select last item
        app.sessions.tree_state.select(Some(len - 1));

        // Move down (+1) wraps to first
        app.move_sel(1);
        assert_eq!(app.sessions.tree_state.selected(), Some(0));

        // Move up (-1) wraps to last
        app.move_sel(-1);
        assert_eq!(app.sessions.tree_state.selected(), Some(len - 1));
    }

    // ── Test 5: selected_node returns None for empty tree ──
    #[test]
    fn selected_node_returns_none_for_empty_tree() {
        let mut app = test_app(vec![], vec![]);
        // Empty workspaces + sessions → empty tree
        assert!(app.sessions.tree.is_empty());
        assert!(app.selected_node().is_none());

        // Even after explicitly selecting something (shouldn't happen, but verify robustness)
        app.sessions.tree_state.select(Some(0));
        assert!(app.selected_node().is_none());
    }
}
