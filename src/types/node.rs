use super::agent::Agent;

#[derive(Clone, Debug)]
pub enum TreeNode {
    Workspace(usize),
    /// Virtual workspace for pinned sessions (contains session indices).
    PinnedWorkspace,
    /// Virtual workspace for recent sessions (most recently active, non-pinned, non-running).
    RecentWorkspace,
    /// Warning about a workspace (e.g. path not found). Contains (workspace_index, message).
    WorkspaceWarning(usize, String),
    Session(usize, usize),
    ActiveTab(usize),
    AgentHeader(Agent),
    ArchivedHeader,
    ArchivedSession(usize, usize),
}
