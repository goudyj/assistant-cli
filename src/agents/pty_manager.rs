//! PTY Manager for handling multiple PTY sessions.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use super::pty_session::PtySession;

/// Global PTY manager singleton.
static PTY_MANAGER: std::sync::LazyLock<PtyManager> =
    std::sync::LazyLock::new(PtyManager::new);

/// Get the global PTY manager instance.
pub fn pty_manager() -> &'static PtyManager {
    &PTY_MANAGER
}

/// Manager for multiple PTY sessions.
pub struct PtyManager {
    sessions: Arc<RwLock<HashMap<String, Arc<PtySession>>>>,
}

impl PtyManager {
    /// Create a new PTY manager.
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Spawn a new PTY session.
    ///
    /// # Arguments
    /// * `id` - Unique session identifier (e.g., "project-issue-42")
    /// * `program` - The program to run (e.g., "claude" or "opencode")
    /// * `args` - Arguments to pass to the program
    /// * `working_dir` - Working directory for the command
    /// * `rows` - Terminal height in rows
    /// * `cols` - Terminal width in columns
    pub fn spawn_session(
        &self,
        id: &str,
        program: &str,
        args: &[&str],
        working_dir: &Path,
        rows: u16,
        cols: u16,
    ) -> Result<Arc<PtySession>, String> {
        // Check if session already exists
        {
            let sessions = self.sessions.read().unwrap();
            if let Some(existing) = sessions.get(id) {
                if existing.is_running() {
                    return Err(format!("Session '{}' already exists and is running", id));
                }
            }
        }

        // Spawn the new session
        let session = PtySession::spawn(id, program, args, working_dir, rows, cols)?;
        let session = Arc::new(session);

        // Store in map
        {
            let mut sessions = self.sessions.write().unwrap();
            sessions.insert(id.to_string(), Arc::clone(&session));
        }

        Ok(session)
    }

    /// Get a session by ID.
    pub fn get_session(&self, id: &str) -> Option<Arc<PtySession>> {
        let sessions = self.sessions.read().unwrap();
        sessions.get(id).cloned()
    }

    /// List all session IDs.
    pub fn list_sessions(&self) -> Vec<String> {
        let sessions = self.sessions.read().unwrap();
        sessions.keys().cloned().collect()
    }

    /// List all running session IDs.
    pub fn list_running_sessions(&self) -> Vec<String> {
        let sessions = self.sessions.read().unwrap();
        sessions
            .iter()
            .filter(|(_, session)| session.is_running())
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Check if a session exists and is running.
    pub fn is_session_running(&self, id: &str) -> bool {
        let sessions = self.sessions.read().unwrap();
        sessions
            .get(id)
            .is_some_and(|session| session.is_running())
    }

    /// Kill a session by ID.
    pub fn kill_session(&self, id: &str) -> bool {
        let sessions = self.sessions.read().unwrap();
        if let Some(session) = sessions.get(id) {
            session.kill();
            true
        } else {
            false
        }
    }

    /// Remove a session from the manager (cleanup).
    pub fn remove_session(&self, id: &str) -> Option<Arc<PtySession>> {
        let mut sessions = self.sessions.write().unwrap();
        sessions.remove(id)
    }

    /// Cleanup dead sessions from the manager.
    pub fn cleanup_dead_sessions(&self) {
        let mut sessions = self.sessions.write().unwrap();
        sessions.retain(|_, session| session.is_running());
    }

    /// Get the number of active sessions.
    pub fn session_count(&self) -> usize {
        let sessions = self.sessions.read().unwrap();
        sessions.len()
    }

    /// Get the number of running sessions.
    pub fn running_session_count(&self) -> usize {
        let sessions = self.sessions.read().unwrap();
        sessions.values().filter(|s| s.is_running()).count()
    }
}

impl Default for PtyManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_creation() {
        let manager = PtyManager::new();
        assert_eq!(manager.session_count(), 0);
    }

    #[test]
    fn test_list_sessions_empty() {
        let manager = PtyManager::new();
        assert!(manager.list_sessions().is_empty());
    }
}
