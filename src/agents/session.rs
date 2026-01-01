//! Session management for agent instances.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use super::{cache_dir, sessions_file};

/// Status of an agent session
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentStatus {
    Running,
    Completed { exit_code: i32 },
    Failed { error: String },
}

/// Statistics about an agent's work
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentStats {
    /// Lines of output from the agent
    pub lines_output: usize,
    /// Lines of code added (from git diff)
    pub lines_added: usize,
    /// Lines of code deleted (from git diff)
    pub lines_deleted: usize,
    /// Number of files changed
    pub files_changed: usize,
}

/// Represents a single agent session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    /// Unique session ID (UUID)
    pub id: String,
    /// Issue number being worked on
    pub issue_number: u64,
    /// Issue title
    pub issue_title: String,
    /// Project name
    pub project: String,
    /// When the session started
    pub started_at: DateTime<Utc>,
    /// Current status
    pub status: AgentStatus,
    /// Process ID of the agent
    pub pid: u32,
    /// Path to the log file
    pub log_file: PathBuf,
    /// Path to the git worktree
    pub worktree_path: PathBuf,
    /// Branch name for this issue
    pub branch_name: String,
    /// Work statistics
    pub stats: AgentStats,
    /// URL of the PR if created
    pub pr_url: Option<String>,
}

impl AgentSession {
    /// Create a new agent session
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        issue_number: u64,
        issue_title: String,
        project: String,
        pid: u32,
        log_file: PathBuf,
        worktree_path: PathBuf,
        branch_name: String,
    ) -> Self {
        Self {
            id,
            issue_number,
            issue_title,
            project,
            started_at: Utc::now(),
            status: AgentStatus::Running,
            pid,
            log_file,
            worktree_path,
            branch_name,
            stats: AgentStats::default(),
            pr_url: None,
        }
    }

    /// Check if the session is still running
    pub fn is_running(&self) -> bool {
        matches!(self.status, AgentStatus::Running)
    }

    /// Get duration since start
    pub fn duration(&self) -> chrono::Duration {
        Utc::now() - self.started_at
    }

    /// Format duration as human-readable string
    pub fn duration_str(&self) -> String {
        let dur = self.duration();
        let secs = dur.num_seconds();
        if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            format!("{}m {}s", secs / 60, secs % 60)
        } else {
            format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
        }
    }
}

/// Manages agent sessions stored in a JSON file
pub struct SessionManager {
    sessions: Vec<AgentSession>,
}

impl SessionManager {
    /// Load sessions from file or create empty manager
    pub fn load() -> Self {
        let path = sessions_file();
        let sessions = if path.exists() {
            fs::read_to_string(&path)
                .ok()
                .and_then(|content| serde_json::from_str(&content).ok())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        Self { sessions }
    }

    /// Save sessions to file
    pub fn save(&self) -> Result<(), std::io::Error> {
        let dir = cache_dir();
        fs::create_dir_all(&dir)?;

        let path = sessions_file();
        let content = serde_json::to_string_pretty(&self.sessions)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(path, content)
    }

    /// Get all sessions
    pub fn list(&self) -> &[AgentSession] {
        &self.sessions
    }

    /// Get running sessions
    pub fn running(&self) -> Vec<&AgentSession> {
        self.sessions.iter().filter(|s| s.is_running()).collect()
    }

    /// Get a session by ID
    pub fn get(&self, id: &str) -> Option<&AgentSession> {
        self.sessions.iter().find(|s| s.id == id)
    }

    /// Get a mutable session by ID
    pub fn get_mut(&mut self, id: &str) -> Option<&mut AgentSession> {
        self.sessions.iter_mut().find(|s| s.id == id)
    }

    /// Get a session by project and issue number
    pub fn get_by_issue(&self, project: &str, issue_number: u64) -> Option<&AgentSession> {
        self.sessions
            .iter()
            .find(|s| s.project == project && s.issue_number == issue_number)
    }

    /// Add a new session
    pub fn add(&mut self, session: AgentSession) {
        self.sessions.push(session);
    }

    /// Update session status
    pub fn update_status(&mut self, id: &str, status: AgentStatus) -> bool {
        if let Some(session) = self.get_mut(id) {
            session.status = status;
            true
        } else {
            false
        }
    }

    /// Update session stats
    pub fn update_stats(&mut self, id: &str, stats: AgentStats) -> bool {
        if let Some(session) = self.get_mut(id) {
            session.stats = stats;
            true
        } else {
            false
        }
    }

    /// Remove old sessions (older than `days`)
    pub fn cleanup_old_sessions(&mut self, days: u32) {
        let cutoff = Utc::now() - chrono::Duration::days(i64::from(days));
        self.sessions.retain(|s| {
            // Keep running sessions
            if s.is_running() {
                return true;
            }
            // Keep recent sessions
            s.started_at > cutoff
        });
    }

    /// Remove a session by ID
    pub fn remove(&mut self, id: &str) -> Option<AgentSession> {
        if let Some(pos) = self.sessions.iter().position(|s| s.id == id) {
            Some(self.sessions.remove(pos))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_session_is_running() {
        let session = AgentSession::new(
            "test-id".to_string(),
            123,
            "Test issue".to_string(),
            "test-project".to_string(),
            1234,
            PathBuf::from("/tmp/test.log"),
            PathBuf::from("/tmp/worktree"),
            "issue-123".to_string(),
        );

        assert!(session.is_running());
    }

    #[test]
    fn agent_session_completed() {
        let mut session = AgentSession::new(
            "test-id".to_string(),
            123,
            "Test issue".to_string(),
            "test-project".to_string(),
            1234,
            PathBuf::from("/tmp/test.log"),
            PathBuf::from("/tmp/worktree"),
            "issue-123".to_string(),
        );

        session.status = AgentStatus::Completed { exit_code: 0 };
        assert!(!session.is_running());
    }

    #[test]
    fn session_manager_add_and_get() {
        let mut manager = SessionManager { sessions: Vec::new() };

        let session = AgentSession::new(
            "test-id".to_string(),
            123,
            "Test issue".to_string(),
            "test-project".to_string(),
            1234,
            PathBuf::from("/tmp/test.log"),
            PathBuf::from("/tmp/worktree"),
            "issue-123".to_string(),
        );

        manager.add(session);

        assert!(manager.get("test-id").is_some());
        assert!(manager.get("non-existent").is_none());
    }

    #[test]
    fn session_manager_update_status() {
        let mut manager = SessionManager { sessions: Vec::new() };

        let session = AgentSession::new(
            "test-id".to_string(),
            123,
            "Test issue".to_string(),
            "test-project".to_string(),
            1234,
            PathBuf::from("/tmp/test.log"),
            PathBuf::from("/tmp/worktree"),
            "issue-123".to_string(),
        );

        manager.add(session);

        assert!(manager.update_status("test-id", AgentStatus::Completed { exit_code: 0 }));
        assert!(!manager.get("test-id").unwrap().is_running());
    }

    #[test]
    fn duration_formatting() {
        let session = AgentSession::new(
            "test-id".to_string(),
            123,
            "Test issue".to_string(),
            "test-project".to_string(),
            1234,
            PathBuf::from("/tmp/test.log"),
            PathBuf::from("/tmp/worktree"),
            "issue-123".to_string(),
        );

        // Just check it doesn't panic
        let _dur = session.duration_str();
    }

    #[test]
    fn session_serialization() {
        let session = AgentSession::new(
            "test-id".to_string(),
            123,
            "Test issue".to_string(),
            "test-project".to_string(),
            1234,
            PathBuf::from("/tmp/test.log"),
            PathBuf::from("/tmp/worktree"),
            "issue-123".to_string(),
        );

        // Test JSON round-trip
        let json = serde_json::to_string(&session).unwrap();
        let deserialized: AgentSession = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, "test-id");
        assert_eq!(deserialized.issue_number, 123);
        assert_eq!(deserialized.issue_title, "Test issue");
        assert_eq!(deserialized.project, "test-project");
        assert!(deserialized.is_running());
    }

    #[test]
    fn session_manager_running_filter() {
        let mut manager = SessionManager { sessions: Vec::new() };

        let running = AgentSession::new(
            "running".to_string(),
            1,
            "Running".to_string(),
            "project".to_string(),
            100,
            PathBuf::from("/tmp/1.log"),
            PathBuf::from("/tmp/1"),
            "issue-1".to_string(),
        );

        let mut completed = AgentSession::new(
            "completed".to_string(),
            2,
            "Completed".to_string(),
            "project".to_string(),
            200,
            PathBuf::from("/tmp/2.log"),
            PathBuf::from("/tmp/2"),
            "issue-2".to_string(),
        );
        completed.status = AgentStatus::Completed { exit_code: 0 };

        manager.add(running);
        manager.add(completed);

        let running_sessions = manager.running();
        assert_eq!(running_sessions.len(), 1);
        assert_eq!(running_sessions[0].id, "running");
    }

    #[test]
    fn session_manager_update_stats() {
        let mut manager = SessionManager { sessions: Vec::new() };

        let session = AgentSession::new(
            "test-id".to_string(),
            123,
            "Test issue".to_string(),
            "test-project".to_string(),
            1234,
            PathBuf::from("/tmp/test.log"),
            PathBuf::from("/tmp/worktree"),
            "issue-123".to_string(),
        );

        manager.add(session);

        let new_stats = AgentStats {
            lines_output: 100,
            lines_added: 50,
            lines_deleted: 20,
            files_changed: 5,
        };

        assert!(manager.update_stats("test-id", new_stats));
        let updated = manager.get("test-id").unwrap();
        assert_eq!(updated.stats.lines_output, 100);
        assert_eq!(updated.stats.lines_added, 50);
        assert_eq!(updated.stats.files_changed, 5);
    }

    #[test]
    fn session_manager_remove() {
        let mut manager = SessionManager { sessions: Vec::new() };

        let session = AgentSession::new(
            "test-id".to_string(),
            123,
            "Test issue".to_string(),
            "test-project".to_string(),
            1234,
            PathBuf::from("/tmp/test.log"),
            PathBuf::from("/tmp/worktree"),
            "issue-123".to_string(),
        );

        manager.add(session);
        assert!(manager.get("test-id").is_some());

        let removed = manager.remove("test-id");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, "test-id");
        assert!(manager.get("test-id").is_none());
    }

    #[test]
    fn cleanup_old_sessions() {
        let mut manager = SessionManager { sessions: Vec::new() };

        // Create a running session (should not be removed)
        let running = AgentSession::new(
            "running".to_string(),
            1,
            "Running".to_string(),
            "project".to_string(),
            100,
            PathBuf::from("/tmp/1.log"),
            PathBuf::from("/tmp/1"),
            "issue-1".to_string(),
        );

        // Create a recent completed session (should not be removed)
        let mut recent = AgentSession::new(
            "recent".to_string(),
            2,
            "Recent".to_string(),
            "project".to_string(),
            200,
            PathBuf::from("/tmp/2.log"),
            PathBuf::from("/tmp/2"),
            "issue-2".to_string(),
        );
        recent.status = AgentStatus::Completed { exit_code: 0 };

        manager.add(running);
        manager.add(recent);

        // Cleanup sessions older than 7 days (none should be removed)
        manager.cleanup_old_sessions(7);

        assert_eq!(manager.list().len(), 2);
    }
}
