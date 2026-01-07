//! Agent management for dispatching issues to Claude Code and other agents.

mod claude;
mod opencode;
mod session;
mod traits;
mod worktree;

pub use claude::{
    attach_tmux_command, create_pr, dispatch_to_agent, dispatch_to_claude, is_tmux_session_running,
    kill_agent, kill_tmux_session, list_tmux_sessions, resume_monitoring_for_running_sessions,
    tmux_session_name, ClaudeCodeAgent,
};
pub use opencode::OpencodeAgent;
pub use session::{AgentSession, AgentStats, AgentStatus, SessionManager};
pub use traits::{get_agent, CodingAgent};
pub use worktree::{
    create_worktree, get_diff_stats, list_orphaned_worktrees, list_worktrees, open_in_ide,
    prune_worktrees, remove_worktree, WorktreeError, WorktreeInfo,
};

use std::path::PathBuf;
use uuid::Uuid;

/// Error types for agent operations
#[derive(Debug)]
pub enum AgentError {
    WorktreeError(WorktreeError),
    SessionError(String),
    ProcessError(String),
    IoError(std::io::Error),
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentError::WorktreeError(e) => write!(f, "Worktree error: {}", e),
            AgentError::SessionError(msg) => write!(f, "Session error: {}", msg),
            AgentError::ProcessError(msg) => write!(f, "Process error: {}", msg),
            AgentError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for AgentError {}

impl From<WorktreeError> for AgentError {
    fn from(e: WorktreeError) -> Self {
        AgentError::WorktreeError(e)
    }
}

impl From<std::io::Error> for AgentError {
    fn from(e: std::io::Error) -> Self {
        AgentError::IoError(e)
    }
}

/// Get the cache directory for the assistant
pub fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("assistant")
}

/// Get the directory for agent logs
pub fn agents_log_dir() -> PathBuf {
    cache_dir().join("agents")
}

/// Get the directory for git worktrees
pub fn worktrees_dir() -> PathBuf {
    cache_dir().join("worktrees")
}

/// Get the sessions file path
pub fn sessions_file() -> PathBuf {
    cache_dir().join("sessions.json")
}

/// Generate a new unique session ID
pub fn new_session_id() -> String {
    Uuid::new_v4().to_string()
}

/// Send a macOS notification with sound
#[cfg(target_os = "macos")]
pub fn send_notification(title: &str, message: &str) {
    use std::process::Command;
    let script = format!(
        "display notification \"{}\" with title \"{}\" sound name \"Glass\"",
        message.replace('"', "\\\"").replace('\n', " "),
        title.replace('"', "\\\"")
    );
    // Use output() instead of spawn() to ensure the command completes
    let _ = Command::new("osascript")
        .args(["-e", &script])
        .output();
}

#[cfg(not(target_os = "macos"))]
pub fn send_notification(_title: &str, _message: &str) {
    // No-op on non-macOS platforms
}

/// Build the prompt for dispatching an issue to a coding agent.
pub fn build_issue_prompt(issue: &crate::github::IssueDetail) -> String {
    let mut prompt = format!(
        "Read and implement GitHub issue #{}: {}\n\n",
        issue.number, issue.title
    );

    if let Some(ref body) = issue.body {
        prompt.push_str(body);
    }

    prompt
}
