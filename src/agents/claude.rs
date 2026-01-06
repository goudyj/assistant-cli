//! Claude Code integration for dispatching issues.

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use super::traits::CodingAgent;
use super::{
    agents_log_dir, create_worktree, get_diff_stats, new_session_id, send_notification,
    AgentError, AgentSession, AgentStats, AgentStatus, SessionManager,
};
use crate::config::CodingAgentType;
use crate::github::IssueDetail;

/// Claude Code agent for processing GitHub issues.
pub struct ClaudeCodeAgent;

impl CodingAgent for ClaudeCodeAgent {
    fn name(&self) -> &'static str {
        "Claude Code"
    }

    fn cli_command(&self) -> &'static str {
        "claude"
    }

    fn is_idle(&self, pane_content: &str) -> bool {
        is_claude_idle(pane_content)
    }
}

/// Dispatch an issue to a coding agent for processing.
///
/// This creates a git worktree, launches the agent in an interactive
/// tmux session, and returns immediately with a session handle.
pub async fn dispatch_to_agent(
    issue: &IssueDetail,
    local_path: &Path,
    project: &str,
    agent_type: &CodingAgentType,
) -> Result<AgentSession, AgentError> {
    use super::traits::get_agent;

    let agent = get_agent(agent_type);
    let session_id = new_session_id();

    // Create the worktree
    let (worktree_path, branch_name) = create_worktree(local_path, project, issue.number)?;

    // Ensure log directory exists
    let log_dir = agents_log_dir();
    fs::create_dir_all(&log_dir)?;

    // Create log file (for session metadata)
    let log_file = log_dir.join(format!("{}.log", session_id));

    // Build the prompt
    let prompt = build_prompt(issue);

    // Get tmux session name
    let tmux_name = tmux_session_name(project, issue.number);

    // Launch agent in tmux using trait method
    launch_agent_tmux(&*agent, &worktree_path, &prompt, &tmux_name)?;

    // Create session with agent type
    let session = AgentSession::new(
        session_id.clone(),
        issue.number,
        issue.title.clone(),
        project.to_string(),
        0, // No direct PID, we use tmux session name
        log_file.clone(),
        worktree_path.clone(),
        branch_name,
        agent_type.clone(),
    );

    // Save session
    let mut manager = SessionManager::load();
    manager.add(session.clone());
    manager.save()?;

    // Start monitoring thread for tmux session with agent type
    start_tmux_monitoring(session_id, tmux_name, worktree_path, agent_type.clone());

    Ok(session)
}

/// Dispatch an issue to Claude Code (backward compatibility wrapper).
pub async fn dispatch_to_claude(
    issue: &IssueDetail,
    local_path: &Path,
    project: &str,
) -> Result<AgentSession, AgentError> {
    dispatch_to_agent(issue, local_path, project, &CodingAgentType::Claude).await
}

/// Build the prompt for the coding agent from an issue.
fn build_prompt(issue: &IssueDetail) -> String {
    let mut prompt = format!(
        "Implement GitHub issue #{}: {}\n\n",
        issue.number, issue.title
    );

    if let Some(ref body) = issue.body {
        prompt.push_str(body);
    }

    prompt
}

/// Launch a coding agent in an interactive tmux session.
fn launch_agent_tmux(
    agent: &dyn CodingAgent,
    worktree_path: &Path,
    prompt: &str,
    session_name: &str,
) -> Result<(), AgentError> {
    // Build the command using the agent's trait method
    let cmd = agent.build_launch_command(worktree_path, prompt);

    // Create tmux session in detached mode
    let status = Command::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            session_name,
            "-x",
            "200",
            "-y",
            "50",
            "bash",
            "-c",
            &cmd,
        ])
        .status()
        .map_err(|e| AgentError::ProcessError(format!("Failed to launch tmux: {}", e)))?;

    if !status.success() {
        return Err(AgentError::ProcessError(
            "Failed to create tmux session".to_string(),
        ));
    }

    Ok(())
}

/// Get the tmux session name for an issue.
pub fn tmux_session_name(project: &str, issue_number: u64) -> String {
    format!("{}-issue-{}", project, issue_number)
}

/// Check if a tmux session exists and is running.
pub fn is_tmux_session_running(session_name: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", session_name])
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// List all tmux sessions for the assistant.
pub fn list_tmux_sessions() -> Vec<String> {
    let output = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter(|s| s.contains("-issue-"))
                .map(|s| s.to_string())
                .collect()
        }
        _ => vec![],
    }
}

/// Attach to a tmux session (returns the command to run).
pub fn attach_tmux_command(session_name: &str) -> String {
    format!("tmux attach -t {}", session_name)
}

/// Kill a tmux session.
pub fn kill_tmux_session(session_name: &str) -> Result<(), AgentError> {
    let status = Command::new("tmux")
        .args(["kill-session", "-t", session_name])
        .status()
        .map_err(|e| AgentError::ProcessError(format!("Failed to kill tmux session: {}", e)))?;

    if !status.success() {
        return Err(AgentError::ProcessError(
            "Failed to kill tmux session".to_string(),
        ));
    }

    Ok(())
}

/// Capture the content of a tmux pane.
fn capture_tmux_pane(session_name: &str) -> Option<String> {
    let output = Command::new("tmux")
        .args(["capture-pane", "-t", session_name, "-p", "-S", "-50"])
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        None
    }
}

/// Check if Claude Code is idle (waiting for input).
/// Returns true if the last lines indicate Claude is waiting for user input.
fn is_claude_idle(pane_content: &str) -> bool {
    let lines: Vec<&str> = pane_content.lines().collect();

    // Skip empty lines from the end to find the last meaningful line
    let last_lines: Vec<&str> = lines
        .iter()
        .rev()
        .filter(|l| !l.trim().is_empty())
        .take(5)
        .copied()
        .collect();

    for line in &last_lines {
        // Trim only leading whitespace to preserve trailing context
        let trimmed = line.trim_start();

        // Claude Code shows ">" prompt when waiting for input
        if trimmed == ">" || trimmed.starts_with("> ") {
            return true;
        }

        // Claude Code shows selection dialog when asking a question
        // Pattern: "Enter to select · Tab/Arrow keys to navigate · Esc to cancel"
        if trimmed.contains("Enter to select") {
            return true;
        }

        // Claude Code shows authorization prompt
        // Pattern: "Esc to cancel" at the end of permission dialogs
        if trimmed == "Esc to cancel" {
            return true;
        }
    }

    false
}

/// Start a monitoring thread for the tmux session.
/// If `already_awaiting` is true, skip the first idle notification (used when resuming).
fn start_tmux_monitoring_with_state(
    session_id: String,
    tmux_name: String,
    worktree_path: std::path::PathBuf,
    already_awaiting: bool,
    agent_type: CodingAgentType,
) {
    use super::traits::get_agent;

    thread::spawn(move || {
        let agent = get_agent(&agent_type);
        let mut was_idle = already_awaiting;
        let mut idle_notified = already_awaiting;

        loop {
            thread::sleep(Duration::from_secs(5));

            // Update stats from git diff
            let (lines_added, lines_deleted, files_changed) = get_diff_stats(&worktree_path);

            let stats = AgentStats {
                lines_output: 0, // We don't track output lines with tmux
                lines_added,
                lines_deleted,
                files_changed,
            };

            // Keep a copy for notification message
            let stats_copy = stats.clone();

            // Update session
            let mut manager = SessionManager::load();
            manager.update_stats(&session_id, stats);
            let _ = manager.save();

            // Check if tmux session is still running
            if !is_tmux_session_running(&tmux_name) {
                // Session ended - mark as completed
                let new_status = AgentStatus::Completed { exit_code: 0 };

                let mut manager = SessionManager::load();
                manager.update_status(&session_id, new_status);
                let _ = manager.save();

                // Send notification using agent name
                if let Some(session) = manager.get(&session_id) {
                    let title = agent.name();
                    let message = format!("Session ended for issue #{}", session.issue_number);
                    send_notification(title, &message);
                }

                break;
            }

            // Check if agent is idle (waiting for user input)
            if let Some(pane_content) = capture_tmux_pane(&tmux_name) {
                let is_idle = agent.is_idle(&pane_content);

                if is_idle && !was_idle {
                    // Agent just became idle - update status to Awaiting
                    let mut manager = SessionManager::load();
                    manager.update_status(&session_id, AgentStatus::Awaiting);
                    let _ = manager.save();

                    // Send notification only once
                    if !idle_notified {
                        if let Some(session) = manager.get(&session_id) {
                            let title = agent.name();
                            let message = format!(
                                "Awaiting input for issue #{} (+{} -{})",
                                session.issue_number,
                                stats_copy.lines_added,
                                stats_copy.lines_deleted
                            );
                            send_notification(title, &message);
                        }
                        idle_notified = true;
                    }
                } else if !is_idle && was_idle {
                    // Agent started working again - update status to Running
                    let mut manager = SessionManager::load();
                    manager.update_status(&session_id, AgentStatus::Running);
                    let _ = manager.save();
                    // Reset notification flag so we can notify again when idle
                    idle_notified = false;
                }

                was_idle = is_idle;
            }
        }
    });
}

/// Start a monitoring thread for a new tmux session.
fn start_tmux_monitoring(
    session_id: String,
    tmux_name: String,
    worktree_path: std::path::PathBuf,
    agent_type: CodingAgentType,
) {
    start_tmux_monitoring_with_state(session_id, tmux_name, worktree_path, false, agent_type);
}

/// Resume monitoring threads for all running sessions.
///
/// This should be called when the TUI starts to ensure stats are updated
/// for sessions that were started in a previous process.
pub fn resume_monitoring_for_running_sessions() {
    let manager = SessionManager::load();

    for session in manager.running() {
        let tmux_name = tmux_session_name(&session.project, session.issue_number);

        // Only start monitoring if tmux session is actually running
        if is_tmux_session_running(&tmux_name) {
            // Pass the current awaiting state to avoid duplicate notifications
            let already_awaiting = session.is_awaiting();
            start_tmux_monitoring_with_state(
                session.id.clone(),
                tmux_name,
                session.worktree_path.clone(),
                already_awaiting,
                session.agent_type.clone(),
            );
        }
    }
}

/// Kill an agent by session ID (kills the tmux session).
pub fn kill_agent(session_id: &str) -> Result<(), AgentError> {
    let manager = SessionManager::load();

    if let Some(session) = manager.get(session_id)
        && session.is_running()
    {
        // Build tmux session name and kill it
        let tmux_name = tmux_session_name(&session.project, session.issue_number);
        let _ = kill_tmux_session(&tmux_name);

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
        assert!(prompt.contains("Implement GitHub issue #123"));
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
        assert!(prompt.contains("Implement GitHub issue #456"));
        assert!(prompt.contains("Another issue"));
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
        assert!(prompt.contains("Implement GitHub issue #789"));
        assert!(prompt.contains("\"quotes\""));
        assert!(prompt.contains("`backticks`"));
        assert!(prompt.contains("<>&"));
    }

    #[test]
    fn idle_detection_simple_prompt() {
        // Simple prompt on its own line
        assert!(is_claude_idle("Some output\n>\n"));
        assert!(is_claude_idle("Some output\n> \n"));
        assert!(is_claude_idle("Some output\n>"));
    }

    #[test]
    fn idle_detection_with_empty_lines() {
        // Prompt followed by empty lines (common in tmux capture)
        assert!(is_claude_idle("Some output\n>\n\n\n"));
        assert!(is_claude_idle("Some output\n> \n\n"));
    }

    #[test]
    fn idle_detection_with_leading_space() {
        // Prompt with leading whitespace
        assert!(is_claude_idle("Some output\n  >\n"));
        assert!(is_claude_idle("Some output\n\t> \n"));
    }

    #[test]
    fn idle_detection_not_idle() {
        // Working output (no prompt)
        assert!(!is_claude_idle("Processing files...\nDone"));
        assert!(!is_claude_idle("Some output without prompt"));
    }

    #[test]
    fn idle_detection_prompt_in_output() {
        // Prompt character in middle of text should still trigger
        // because we check last non-empty lines
        assert!(is_claude_idle("Some > text\n>\n"));
    }

    #[test]
    fn idle_detection_question_dialog() {
        // Claude Code selection dialog
        let content = "Quel type de fichier?\n1. JSON\n2. YAML\nEnter to select · Tab/Arrow keys to navigate · Esc to cancel\n";
        assert!(is_claude_idle(content));
    }

    #[test]
    fn idle_detection_authorization_prompt() {
        // Claude Code authorization/permission dialog
        let content = "Bash command\nuv run python --version\nDo you want to proceed?\n1. Yes\n2. Yes, and don't ask again\nEsc to cancel\n";
        assert!(is_claude_idle(content));
    }
}
