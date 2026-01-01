//! Claude Code integration for dispatching issues.

use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use super::{
    agents_log_dir, create_worktree, get_diff_stats, new_session_id, send_notification,
    AgentError, AgentSession, AgentStats, AgentStatus, SessionManager,
};
use crate::github::IssueDetail;

/// Dispatch an issue to Claude Code for processing.
///
/// This creates a git worktree, launches Claude Code in the background,
/// and returns immediately with a session handle.
pub async fn dispatch_to_claude(
    issue: &IssueDetail,
    local_path: &Path,
    project: &str,
) -> Result<AgentSession, AgentError> {
    let session_id = new_session_id();

    // Create the worktree
    let (worktree_path, branch_name) = create_worktree(local_path, project, issue.number)?;

    // Ensure log directory exists
    let log_dir = agents_log_dir();
    fs::create_dir_all(&log_dir)?;

    // Create log file
    let log_file = log_dir.join(format!("{}.log", session_id));

    // Build the prompt
    let prompt = build_prompt(issue);

    // Launch Claude Code
    let child = launch_claude(&worktree_path, &prompt, &log_file)?;
    let pid = child.id();

    // Create session
    let session = AgentSession::new(
        session_id.clone(),
        issue.number,
        issue.title.clone(),
        project.to_string(),
        pid,
        log_file.clone(),
        worktree_path.clone(),
        branch_name,
    );

    // Save session
    let mut manager = SessionManager::load();
    manager.add(session.clone());
    manager.save()?;

    // Start monitoring thread
    start_monitoring(session_id, child, worktree_path, log_file);

    Ok(session)
}

/// Build the prompt for Claude Code from an issue.
fn build_prompt(issue: &IssueDetail) -> String {
    let mut prompt = format!(
        "Fix GitHub issue #{}: {}\n\n",
        issue.number, issue.title
    );

    if let Some(ref body) = issue.body {
        prompt.push_str(body);
    }

    prompt
}

/// Launch Claude Code as a background process.
fn launch_claude(worktree_path: &Path, prompt: &str, log_file: &Path) -> Result<Child, AgentError> {
    let log = File::create(log_file)?;
    let log_err = log.try_clone()?;

    let child = Command::new("claude")
        .current_dir(worktree_path)
        .args(["-p", prompt])
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err))
        .spawn()
        .map_err(|e| AgentError::ProcessError(format!("Failed to launch claude: {}", e)))?;

    Ok(child)
}

/// Start a monitoring thread for the Claude process.
fn start_monitoring(
    session_id: String,
    child: Child,
    worktree_path: std::path::PathBuf,
    log_file: std::path::PathBuf,
) {
    thread::spawn(move || {
        let child = Arc::new(Mutex::new(child));
        let child_clone = Arc::clone(&child);

        loop {
            thread::sleep(Duration::from_secs(5));

            // Update stats
            let (lines_added, lines_deleted, files_changed) = get_diff_stats(&worktree_path);
            let lines_output = count_log_lines(&log_file);

            let stats = AgentStats {
                lines_output,
                lines_added,
                lines_deleted,
                files_changed,
            };

            // Update session
            let mut manager = SessionManager::load();
            manager.update_stats(&session_id, stats);
            let _ = manager.save();

            // Check if process is done
            let mut child = child_clone.lock().unwrap();
            match child.try_wait() {
                Ok(Some(status)) => {
                    // Process finished
                    let new_status = if status.success() {
                        AgentStatus::Completed {
                            exit_code: status.code().unwrap_or(0),
                        }
                    } else {
                        AgentStatus::Failed {
                            error: format!("Exit code: {}", status.code().unwrap_or(-1)),
                        }
                    };

                    let mut manager = SessionManager::load();
                    manager.update_status(&session_id, new_status.clone());
                    let _ = manager.save();

                    // Send notification
                    if let Some(session) = manager.get(&session_id) {
                        let title = "Claude Code";
                        let message = match &new_status {
                            AgentStatus::Completed { .. } => {
                                format!("Finished issue #{}", session.issue_number)
                            }
                            AgentStatus::Failed { error } => {
                                format!("Failed issue #{}: {}", session.issue_number, error)
                            }
                            _ => String::new(),
                        };
                        send_notification(title, &message);
                    }

                    break;
                }
                Ok(None) => {
                    // Still running
                }
                Err(e) => {
                    // Error checking status
                    let mut manager = SessionManager::load();
                    manager.update_status(
                        &session_id,
                        AgentStatus::Failed {
                            error: format!("Monitor error: {}", e),
                        },
                    );
                    let _ = manager.save();
                    break;
                }
            }
        }
    });
}

/// Count lines in a log file.
fn count_log_lines(log_file: &Path) -> usize {
    if let Ok(file) = File::open(log_file) {
        BufReader::new(file).lines().count()
    } else {
        0
    }
}

/// Kill an agent by session ID.
pub fn kill_agent(session_id: &str) -> Result<(), AgentError> {
    let manager = SessionManager::load();

    if let Some(session) = manager.get(session_id)
        && session.is_running() {
            // Try to kill the process
            #[cfg(unix)]
            {
                let _ = Command::new("kill")
                    .args(["-9", &session.pid.to_string()])
                    .status();
            }

            #[cfg(not(unix))]
            {
                // On Windows, use taskkill
                let _ = Command::new("taskkill")
                    .args(["/F", "/PID", &session.pid.to_string()])
                    .status();
            }

            // Update status
            let mut manager = SessionManager::load();
            manager.update_status(
                session_id,
                AgentStatus::Failed {
                    error: "Killed by user".to_string(),
                },
            );
            manager.save()?;
        }

    Ok(())
}

/// Create a PR from a completed session.
pub fn create_pr(session: &AgentSession) -> Result<String, AgentError> {
    let output = Command::new("gh")
        .current_dir(&session.worktree_path)
        .args([
            "pr",
            "create",
            "--title",
            &format!("Fix #{}: {}", session.issue_number, session.issue_title),
            "--body",
            &format!(
                "Closes #{}\n\nAutomatically generated by Claude Code.",
                session.issue_number
            ),
        ])
        .output()
        .map_err(|e| AgentError::ProcessError(format!("Failed to create PR: {}", e)))?;

    if output.status.success() {
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(url)
    } else {
        Err(AgentError::ProcessError(format!(
            "gh pr create failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_prompt_with_body() {
        let issue = IssueDetail {
            number: 123,
            title: "Fix the bug".to_string(),
            body: Some("This is the description".to_string()),
            html_url: "https://github.com/test/test/issues/123".to_string(),
            labels: vec![],
            state: "Open".to_string(),
            assignees: vec![],
            comments: vec![],
        };

        let prompt = build_prompt(&issue);
        assert!(prompt.contains("Fix GitHub issue #123"));
        assert!(prompt.contains("Fix the bug"));
        assert!(prompt.contains("This is the description"));
    }

    #[test]
    fn build_prompt_without_body() {
        let issue = IssueDetail {
            number: 456,
            title: "Another issue".to_string(),
            body: None,
            html_url: "https://github.com/test/test/issues/456".to_string(),
            labels: vec![],
            state: "Open".to_string(),
            assignees: vec![],
            comments: vec![],
        };

        let prompt = build_prompt(&issue);
        assert!(prompt.contains("Fix GitHub issue #456"));
        assert!(prompt.contains("Another issue"));
    }

    #[test]
    fn count_log_lines_nonexistent() {
        let count = count_log_lines(Path::new("/nonexistent/file.log"));
        assert_eq!(count, 0);
    }

    #[test]
    fn build_prompt_with_special_characters() {
        let issue = IssueDetail {
            number: 789,
            title: "Fix \"quotes\" and `backticks`".to_string(),
            body: Some("Description with\nnewlines\nand special chars: <>&".to_string()),
            html_url: "https://github.com/test/test/issues/789".to_string(),
            labels: vec![],
            state: "Open".to_string(),
            assignees: vec![],
            comments: vec![],
        };

        let prompt = build_prompt(&issue);
        assert!(prompt.contains("Fix GitHub issue #789"));
        assert!(prompt.contains("\"quotes\""));
        assert!(prompt.contains("`backticks`"));
        assert!(prompt.contains("<>&"));
    }
}
