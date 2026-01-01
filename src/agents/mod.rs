//! Agent management for dispatching issues to Claude Code and other agents.

mod claude;
mod session;
mod worktree;

pub use claude::{create_pr, dispatch_to_claude, kill_agent};
pub use session::{AgentSession, AgentStats, AgentStatus, SessionManager};
pub use worktree::{create_worktree, get_diff_stats, remove_worktree, WorktreeError};

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

/// Send a macOS notification
#[cfg(target_os = "macos")]
pub fn send_notification(title: &str, message: &str) {
    use std::process::Command;
    let script = format!(
        "display notification \"{}\" with title \"{}\"",
        message.replace('"', "\\\""),
        title.replace('"', "\\\"")
    );
    let _ = Command::new("osascript")
        .args(["-e", &script])
        .spawn();
}

#[cfg(not(target_os = "macos"))]
pub fn send_notification(_title: &str, _message: &str) {
    // No-op on non-macOS platforms
}
