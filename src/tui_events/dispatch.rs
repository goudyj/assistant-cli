//! Dispatch instructions views event handling.

use crate::github::IssueDetail;
use crate::tui::IssueBrowser;
use crate::tui_types::TuiView;
use crossterm::event::{KeyCode, KeyModifiers};
use std::path::PathBuf;

pub async fn handle_dispatch_instructions_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    modifiers: KeyModifiers,
    issue: &IssueDetail,
    input: &mut String,
) {
    match key {
        KeyCode::Esc => {
            browser.view = TuiView::List;
        }
        KeyCode::Enter if modifiers.contains(KeyModifiers::SHIFT) => {
            input.push('\n');
        }
        KeyCode::Enter => {
            // Dispatch with instructions (or without if empty)
            if let Some(local_path) = browser.local_path.clone() {
                let project_name = browser.project_name.clone().unwrap_or_default();
                let instructions = if input.trim().is_empty() {
                    None
                } else {
                    Some(input.as_str())
                };
                let agent = crate::agents::get_agent(&browser.coding_agent);
                match crate::agents::dispatch_to_agent(
                    issue,
                    &local_path,
                    &project_name,
                    &browser.coding_agent,
                    browser.base_branch.as_deref(),
                    instructions,
                )
                .await
                {
                    Ok(_) => {
                        browser.status_message =
                            Some(format!("Dispatched #{} to {}.", issue.number, agent.name()));
                    }
                    Err(e) => {
                        browser.status_message = Some(format!("Failed to dispatch: {}", e));
                    }
                }
                if let Some(project) = browser.project_name.clone() {
                    browser.refresh_sessions(&project);
                }
            }
            browser.view = TuiView::List;
        }
        KeyCode::Backspace => {
            input.pop();
        }
        KeyCode::Char(c) => {
            input.push(c);
        }
        _ => {}
    }
}

pub async fn handle_worktree_agent_instructions_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    modifiers: KeyModifiers,
    worktree_path: &PathBuf,
    branch_name: &str,
    input: &mut String,
) {
    match key {
        KeyCode::Esc => {
            // Return to PostWorktreeCreate
            browser.view = TuiView::PostWorktreeCreate {
                worktree_path: worktree_path.clone(),
                branch_name: branch_name.to_string(),
            };
        }
        KeyCode::Enter if modifiers.contains(KeyModifiers::SHIFT) => {
            input.push('\n');
        }
        KeyCode::Enter => {
            // Start agent with optional prompt
            let project_name = browser.project_name.clone().unwrap_or_default();
            let sanitized_branch = branch_name.replace('/', "-");
            let session_name = format!("{}-{}", project_name, sanitized_branch);

            let initial_prompt = if input.trim().is_empty() {
                None
            } else {
                Some(input.as_str())
            };

            match crate::agents::launch_agent_interactive(
                worktree_path,
                &session_name,
                &browser.coding_agent,
                initial_prompt,
            ) {
                Ok(_) => {
                    // Enter the tmux session directly
                    let sessions = crate::agents::list_tmux_sessions();
                    let mut all_sessions = sessions;
                    if !all_sessions.contains(&session_name) {
                        all_sessions.push(session_name.clone());
                    }
                    let current_index = all_sessions
                        .iter()
                        .position(|s| s == &session_name)
                        .unwrap_or(0);
                    let area = crossterm::terminal::size().unwrap_or((80, 24));
                    match crate::embedded_term::EmbeddedTerminal::new(
                        &session_name,
                        area.1.saturating_sub(1),
                        area.0,
                    ) {
                        Ok(term) => {
                            browser.embedded_term = Some(term);
                            browser.view = TuiView::EmbeddedTmux {
                                available_sessions: all_sessions,
                                current_index,
                                return_to_worktrees: true,
                            };
                        }
                        Err(e) => {
                            browser.status_message =
                                Some(format!("Failed to open terminal: {}", e));
                            let worktrees = browser.build_worktree_list();
                            browser.view = TuiView::WorktreeList {
                                worktrees,
                                selected: 0,
                            };
                        }
                    }
                }
                Err(e) => {
                    browser.status_message =
                        Some(format!("Failed: {} (path: {})", e, worktree_path.display()));
                    let worktrees = browser.build_worktree_list();
                    browser.view = TuiView::WorktreeList {
                        worktrees,
                        selected: 0,
                    };
                }
            }
        }
        KeyCode::Backspace => {
            input.pop();
        }
        KeyCode::Char(c) => {
            input.push(c);
        }
        _ => {}
    }
}
