//! Claude Code integration for dispatching issues.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::pty_manager::pty_manager;
use super::traits::CodingAgent;
use super::{
    agents_log_dir, build_issue_prompt, create_worktree, get_diff_stats, new_session_id,
    send_notification, AgentError, AgentSession, AgentStats, AgentStatus, SessionManager,
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
/// This creates a git worktree, launches the agent in a PTY session,
/// and returns immediately with a session handle.
pub async fn dispatch_to_agent(
    issue: &IssueDetail,
    local_path: &Path,
    project: &str,
    agent_type: &CodingAgentType,
    base_branch: Option<&str>,
    additional_instructions: Option<&str>,
) -> Result<AgentSession, AgentError> {
    use super::traits::get_agent;

    let agent = get_agent(agent_type);
    let session_id = new_session_id();

    // Create the worktree
    let (worktree_path, branch_name) = create_worktree(local_path, project, issue.number, base_branch)?;

    // Ensure log directory exists
    let log_dir = agents_log_dir();
    fs::create_dir_all(&log_dir)?;

    // Create log file (for session metadata)
    let log_file = log_dir.join(format!("{}.log", session_id));

    // Build the prompt with optional additional instructions
    let prompt = build_issue_prompt(issue, additional_instructions);

    // Get PTY session name
    let pty_name = pty_session_name(project, issue.number);

    // Launch agent in PTY
    launch_agent_pty(&*agent, &worktree_path, &prompt, &pty_name)?;

    // Create session with agent type
    let session = AgentSession::new(
        session_id.clone(),
        issue.number,
        issue.title.clone(),
        project.to_string(),
        0, // No direct PID, we use PTY session name
        log_file.clone(),
        worktree_path.clone(),
        branch_name,
        agent_type.clone(),
    );

    // Save session
    let mut manager = SessionManager::load();
    manager.add(session.clone());
    manager.save()?;

    // Start monitoring thread for PTY session
    start_pty_monitoring(session_id, pty_name, worktree_path, agent_type.clone());

    Ok(session)
}

/// Dispatch an issue to Claude Code (backward compatibility wrapper).
pub async fn dispatch_to_claude(
    issue: &IssueDetail,
    local_path: &Path,
    project: &str,
    base_branch: Option<&str>,
) -> Result<AgentSession, AgentError> {
    dispatch_to_agent(issue, local_path, project, &CodingAgentType::Claude, base_branch, None).await
}

/// Launch a coding agent in a PTY session.
pub fn launch_agent_pty(
    agent: &dyn CodingAgent,
    worktree_path: &Path,
    prompt: &str,
    session_name: &str,
) -> Result<(), AgentError> {
    // Get terminal size
    let (cols, rows) = crossterm::terminal::size().unwrap_or((200, 50));

    // Build the command using the agent's trait method
    let cmd = agent.build_launch_command(worktree_path, prompt);

    // Spawn PTY session with bash -c to run the command
    pty_manager()
        .spawn_session(
            session_name,
            "bash",
            &["-c", &cmd],
            worktree_path,
            rows,
            cols,
        )
        .map_err(|e| AgentError::ProcessError(format!("Failed to launch PTY: {}", e)))?;

    Ok(())
}

/// Launch a coding agent interactively in a PTY session.
///
/// If `initial_prompt` is provided, the agent will be launched with that prompt.
/// Otherwise, it starts in interactive mode without initial context.
pub fn launch_agent_interactive(
    worktree_path: &Path,
    session_name: &str,
    agent_type: &CodingAgentType,
    initial_prompt: Option<&str>,
) -> Result<(), AgentError> {
    use super::traits::get_agent;

    // Verify worktree exists
    if !worktree_path.exists() {
        return Err(AgentError::ProcessError(format!(
            "Worktree path does not exist: {}",
            worktree_path.display()
        )));
    }

    // Check if session already exists
    if is_pty_session_running(session_name) {
        return Err(AgentError::ProcessError(format!(
            "PTY session '{}' already exists",
            session_name
        )));
    }

    let agent = get_agent(agent_type);
    let (cols, rows) = crossterm::terminal::size().unwrap_or((200, 50));

    if let Some(prompt) = initial_prompt {
        // With prompt - use build_launch_command
        let cmd = agent.build_launch_command(worktree_path, prompt);

        pty_manager()
            .spawn_session(
                session_name,
                "bash",
                &["-c", &cmd],
                worktree_path,
                rows,
                cols,
            )
            .map_err(|e| AgentError::ProcessError(format!("Failed to launch PTY: {}", e)))?;
    } else {
        // No prompt - start agent in interactive mode
        let agent_cmd = agent.cli_command();

        pty_manager()
            .spawn_session(session_name, agent_cmd, &[], worktree_path, rows, cols)
            .map_err(|e| AgentError::ProcessError(format!("Failed to launch PTY: {}", e)))?;
    }

    Ok(())
}

/// Get the PTY session name for an issue.
pub fn pty_session_name(project: &str, issue_number: u64) -> String {
    format!("{}-issue-{}", project, issue_number)
}

/// Check if a PTY session exists and is running.
pub fn is_pty_session_running(session_name: &str) -> bool {
    pty_manager().is_session_running(session_name)
}

/// List all PTY sessions for the assistant (issue-based only).
pub fn list_pty_sessions() -> Vec<String> {
    pty_manager()
        .list_running_sessions()
        .into_iter()
        .filter(|s| s.contains("-issue-"))
        .collect()
}

/// List all PTY sessions (no filtering).
pub fn list_all_pty_sessions() -> Vec<String> {
    pty_manager().list_running_sessions()
}

/// Get recent output from a PTY session (for idle detection).
fn get_pty_output(session_name: &str) -> Option<String> {
    pty_manager()
        .get_session(session_name)
        .map(|session| session.get_recent_output())
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
        if trimmed.contains("Enter to select") {
            return true;
        }

        // Claude Code shows authorization prompt
        if trimmed == "Esc to cancel" {
            return true;
        }
    }

    false
}

/// Start a monitoring thread for the PTY session.
fn start_pty_monitoring_with_state(
    session_id: String,
    pty_name: String,
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
                lines_output: 0,
                lines_added,
                lines_deleted,
                files_changed,
            };

            let stats_copy = stats.clone();

            // Update session
            let mut manager = SessionManager::load();
            manager.update_stats(&session_id, stats);
            let _ = manager.save();

            // Check if PTY session is still running
            if !is_pty_session_running(&pty_name) {
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

                // Clean up the session from the manager
                pty_manager().remove_session(&pty_name);

                break;
            }

            // Check if agent is idle (waiting for user input)
            if let Some(output) = get_pty_output(&pty_name) {
                let is_idle = agent.is_idle(&output);

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
                    idle_notified = false;
                }

                was_idle = is_idle;
            }
        }
    });
}

/// Start a monitoring thread for a new PTY session.
fn start_pty_monitoring(
    session_id: String,
    pty_name: String,
    worktree_path: std::path::PathBuf,
    agent_type: CodingAgentType,
) {
    start_pty_monitoring_with_state(session_id, pty_name, worktree_path, false, agent_type);
}

/// Resume monitoring threads for all running sessions.
///
/// This should be called when the TUI starts to ensure stats are updated
/// for sessions that were started in a previous process.
pub fn resume_monitoring_for_running_sessions() {
    let manager = SessionManager::load();

    for session in manager.running() {
        let pty_name = pty_session_name(&session.project, session.issue_number);

        // Only start monitoring if PTY session is actually running
        if is_pty_session_running(&pty_name) {
            let already_awaiting = session.is_awaiting();
            start_pty_monitoring_with_state(
                session.id.clone(),
                pty_name,
                session.worktree_path.clone(),
                already_awaiting,
                session.agent_type.clone(),
            );
        }
    }
}

/// Kill an agent by session ID.
pub fn kill_agent(session_id: &str) -> Result<(), AgentError> {
    let manager = SessionManager::load();

    if let Some(session) = manager.get(session_id)
        && session.is_running()
    {
        // Build PTY session name and kill it
        let pty_name = pty_session_name(&session.project, session.issue_number);
        pty_manager().kill_session(&pty_name);
        pty_manager().remove_session(&pty_name);

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
pub fn create_pr(session: &AgentSession, base_branch: Option<&str>) -> Result<String, AgentError> {
    use std::process::Command;

    let mut args = vec![
        "pr".to_string(),
        "create".to_string(),
        "--title".to_string(),
        format!("Fix #{}: {}", session.issue_number, session.issue_title),
        "--body".to_string(),
        format!(
            "Closes #{}\n\nAutomatically generated by Claude Code.",
            session.issue_number
        ),
    ];

    if let Some(base) = base_branch {
        args.push("--base".to_string());
        args.push(base.to_string());
    }

    let output = Command::new("gh")
        .current_dir(&session.worktree_path)
        .args(&args)
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

/// Get a PTY session by name (for embedded terminal).
pub fn get_pty_session(session_name: &str) -> Option<Arc<super::pty_session::PtySession>> {
    pty_manager().get_session(session_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_issue_prompt_with_body() {
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

        let prompt = build_issue_prompt(&issue, None);
        assert!(prompt.contains("Read and implement GitHub issue #123"));
        assert!(prompt.contains("Fix the bug"));
        assert!(prompt.contains("This is the description"));
    }

    #[test]
    fn build_issue_prompt_without_body() {
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

        let prompt = build_issue_prompt(&issue, None);
        assert!(prompt.contains("Read and implement GitHub issue #456"));
        assert!(prompt.contains("Another issue"));
    }

    #[test]
    fn build_issue_prompt_with_special_characters() {
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

        let prompt = build_issue_prompt(&issue, None);
        assert!(prompt.contains("Read and implement GitHub issue #789"));
        assert!(prompt.contains("\"quotes\""));
        assert!(prompt.contains("`backticks`"));
        assert!(prompt.contains("<>&"));
    }

    #[test]
    fn build_issue_prompt_with_additional_instructions() {
        let issue = IssueDetail {
            number: 100,
            title: "Test issue".to_string(),
            body: Some("Issue body".to_string()),
            html_url: "https://github.com/test/test/issues/100".to_string(),
            labels: vec![],
            state: "Open".to_string(),
            assignees: vec![],
            comments: vec![],
        };

        let prompt = build_issue_prompt(&issue, Some("Use TDD approach\nWrite tests first"));
        assert!(prompt.contains("Read and implement GitHub issue #100"));
        assert!(prompt.contains("Issue body"));
        assert!(prompt.contains("Additional instructions:"));
        assert!(prompt.contains("Use TDD approach"));
        assert!(prompt.contains("Write tests first"));
    }

    #[test]
    fn build_issue_prompt_with_empty_instructions() {
        let issue = IssueDetail {
            number: 101,
            title: "Test issue".to_string(),
            body: Some("Issue body".to_string()),
            html_url: "https://github.com/test/test/issues/101".to_string(),
            labels: vec![],
            state: "Open".to_string(),
            assignees: vec![],
            comments: vec![],
        };

        let prompt = build_issue_prompt(&issue, Some("   "));
        assert!(!prompt.contains("Additional instructions:"));
    }

    #[test]
    fn idle_detection_simple_prompt() {
        assert!(is_claude_idle("Some output\n>\n"));
        assert!(is_claude_idle("Some output\n> \n"));
        assert!(is_claude_idle("Some output\n>"));
    }

    #[test]
    fn idle_detection_with_empty_lines() {
        assert!(is_claude_idle("Some output\n>\n\n\n"));
        assert!(is_claude_idle("Some output\n> \n\n"));
    }

    #[test]
    fn idle_detection_with_leading_space() {
        assert!(is_claude_idle("Some output\n  >\n"));
        assert!(is_claude_idle("Some output\n\t> \n"));
    }

    #[test]
    fn idle_detection_not_idle() {
        assert!(!is_claude_idle("Processing files...\nDone"));
        assert!(!is_claude_idle("Some output without prompt"));
    }

    #[test]
    fn idle_detection_prompt_in_output() {
        assert!(is_claude_idle("Some > text\n>\n"));
    }

    #[test]
    fn idle_detection_question_dialog() {
        let content = "Quel type de fichier?\n1. JSON\n2. YAML\nEnter to select · Tab/Arrow keys to navigate · Esc to cancel\n";
        assert!(is_claude_idle(content));
    }

    #[test]
    fn idle_detection_authorization_prompt() {
        let content = "Bash command\nuv run python --version\nDo you want to proceed?\n1. Yes\n2. Yes, and don't ask again\nEsc to cancel\n";
        assert!(is_claude_idle(content));
    }
}
