use super::*;

impl App {
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
                .join(format!("{short_id}.md"));
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
                    text: format!("  {l}"),
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
                    text: format!("  • {f}"),
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
                    text: format!("  • {issue}"),
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
                text: format!("  Last updated: {ts}"),
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
}
