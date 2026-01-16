//! Worktree management views event handling.

use crate::agents::WorktreeInfo;
use crate::tui::IssueBrowser;
use crate::tui_types::TuiView;
use crossterm::event::KeyCode;
use std::path::PathBuf;

pub fn handle_worktree_list_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    worktrees: &[WorktreeInfo],
    selected: &mut usize,
) {
    match key {
        KeyCode::Esc | KeyCode::Char('q') => {
            browser.view = TuiView::List;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if *selected > 0 {
                *selected -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if *selected < worktrees.len().saturating_sub(1) {
                *selected += 1;
            }
        }
        KeyCode::Char('o') => {
            // Open selected worktree in IDE
            if let Some(wt) = worktrees.get(*selected) {
                let ide_cmd = browser.ide_command.as_deref();
                match crate::agents::open_in_ide(&wt.path, ide_cmd) {
                    Ok(_) => {
                        browser.status_message = Some(format!("Opened {} in IDE", wt.name));
                    }
                    Err(e) => {
                        browser.status_message = Some(format!("Failed to open IDE: {}", e));
                    }
                }
            }
        }
        KeyCode::Char('p') => {
            // Create PR for selected worktree
            if let Some(wt) = worktrees.get(*selected) {
                if let Some(issue_num) = wt.issue_number {
                    let manager = crate::agents::SessionManager::load();
                    if let Some(session) = manager.get_by_issue(&wt.project, issue_num) {
                        if session.is_running() {
                            browser.status_message = Some("Agent is still running".to_string());
                        } else if session.pr_url.is_some() {
                            browser.status_message = Some("PR already created".to_string());
                        } else {
                            match crate::agents::create_pr(session, browser.base_branch.as_deref())
                            {
                                Ok(url) => {
                                    browser.status_message = Some(format!("PR created: {}", url));
                                }
                                Err(e) => {
                                    browser.status_message =
                                        Some(format!("Failed to create PR: {}", e));
                                }
                            }
                        }
                    } else {
                        browser.status_message = Some("No session for this worktree".to_string());
                    }
                }
            }
        }
        KeyCode::Char('d') | KeyCode::Delete => {
            // Show confirmation before deleting
            let selected_idx = *selected;
            if let Some(wt) = worktrees.get(selected_idx).cloned() {
                if wt.has_tmux {
                    browser.status_message =
                        Some("Tmux session still running. Kill it first (K).".to_string());
                } else {
                    browser.view = TuiView::ConfirmDeleteWorktree {
                        worktree: wt,
                        return_index: selected_idx,
                    };
                }
            }
        }
        KeyCode::Char('K') => {
            // Kill tmux session for selected worktree
            let selected_idx = *selected;
            if let Some(wt) = worktrees.get(selected_idx).cloned() {
                // Determine tmux session name based on worktree type
                let tmux_name = if let Some(issue_num) = wt.issue_number {
                    crate::agents::tmux_session_name(&wt.project, issue_num)
                } else {
                    // Standalone worktree: session name matches worktree name
                    wt.name.clone()
                };

                if crate::agents::is_tmux_session_running(&tmux_name) {
                    if let Some(issue_num) = wt.issue_number {
                        let manager = crate::agents::SessionManager::load();
                        if let Some(session) = manager.get_by_issue(&wt.project, issue_num) {
                            // Session exists in manager, use kill_agent to update status
                            let _ = crate::agents::kill_agent(&session.id);
                        } else {
                            // Orphaned: no session but tmux running, kill directly
                            let _ = crate::agents::kill_tmux_session(&tmux_name);
                        }
                    } else {
                        // Standalone worktree without issue: kill tmux directly
                        let _ = crate::agents::kill_tmux_session(&tmux_name);
                    }
                    browser.status_message = Some(format!("Killed tmux session: {}", tmux_name));
                    // Refresh session cache to update issue list indicators
                    browser.refresh_sessions(&wt.project);
                    // Refresh the list
                    let new_worktrees = browser.build_worktree_list();
                    let new_selected = selected_idx.min(new_worktrees.len().saturating_sub(1));
                    browser.view = TuiView::WorktreeList {
                        worktrees: new_worktrees,
                        selected: new_selected,
                    };
                } else {
                    browser.status_message = Some("No tmux session running".to_string());
                }
            }
        }
        KeyCode::Char('n') => {
            // Create new worktree
            if browser.local_path.is_none() {
                browser.status_message =
                    Some("No local_path configured for this project.".to_string());
            } else {
                browser.view = TuiView::CreateWorktree {
                    input: String::new(),
                };
            }
        }
        KeyCode::Char('t') => {
            // Open tmux session for selected worktree
            if let Some(wt) = worktrees.get(*selected) {
                // Determine tmux session name based on worktree type
                let session_name = if let Some(issue_num) = wt.issue_number {
                    crate::agents::tmux_session_name(&wt.project, issue_num)
                } else {
                    // Standalone worktree: session name matches worktree name
                    wt.name.clone()
                };

                if crate::agents::is_tmux_session_running(&session_name) {
                    let all_sessions = crate::agents::list_all_tmux_sessions();
                    let current_index = all_sessions
                        .iter()
                        .position(|s| s == &session_name)
                        .unwrap_or(0);
                    // Create embedded terminal to attach to tmux session
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
                        }
                    }
                } else {
                    browser.status_message =
                        Some("No tmux session running for this worktree".to_string());
                }
            }
        }
        KeyCode::Char('a') => {
            // Start agent on selected worktree
            if let Some(wt) = worktrees.get(*selected) {
                // Check if tmux session is already running
                let session_name = if let Some(issue_num) = wt.issue_number {
                    crate::agents::tmux_session_name(&wt.project, issue_num)
                } else {
                    wt.name.clone()
                };

                if crate::agents::is_tmux_session_running(&session_name) {
                    browser.status_message =
                        Some("Tmux session already running. Open it with 't'.".to_string());
                } else {
                    // Extract branch name from worktree name
                    let branch_name = wt.name.clone();
                    browser.view = TuiView::WorktreeAgentInstructions {
                        worktree_path: wt.path.clone(),
                        branch_name,
                        input: String::new(),
                    };
                }
            }
        }
        _ => {}
    }
}

pub fn handle_create_worktree_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    input: &mut String,
) {
    match key {
        KeyCode::Esc => {
            // Return to worktree list
            let worktrees = browser.build_worktree_list();
            if worktrees.is_empty() {
                browser.view = TuiView::List;
            } else {
                browser.view = TuiView::WorktreeList {
                    worktrees,
                    selected: 0,
                };
            }
        }
        KeyCode::Enter => {
            if !input.is_empty() {
                let branch_name = input.clone();
                if let Some(local_path) = &browser.local_path {
                    let project_name = browser.project_name.clone().unwrap_or_default();

                    match crate::agents::create_worktree_with_branch(
                        local_path,
                        &project_name,
                        &branch_name,
                        browser.base_branch.as_deref(),
                    ) {
                        Ok((worktree_path, branch)) => {
                            browser.view = TuiView::PostWorktreeCreate {
                                worktree_path,
                                branch_name: branch,
                            };
                        }
                        Err(e) => {
                            browser.status_message = Some(format!("Failed: {}", e));
                            let worktrees = browser.build_worktree_list();
                            browser.view = TuiView::WorktreeList {
                                worktrees,
                                selected: 0,
                            };
                        }
                    }
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

pub fn handle_post_worktree_create_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    worktree_path: &PathBuf,
    branch_name: &str,
) {
    match key {
        KeyCode::Esc => {
            // Return to worktree list
            if let Some(project) = browser.project_name.clone() {
                browser.refresh_sessions(&project);
            }
            let worktrees = browser.build_worktree_list();
            browser.view = TuiView::WorktreeList {
                worktrees,
                selected: 0,
            };
        }
        KeyCode::Char('o') => {
            // Open in IDE
            let ide_cmd = browser.ide_command.as_deref();
            match crate::agents::open_in_ide(worktree_path, ide_cmd) {
                Ok(_) => {
                    browser.status_message = Some(format!("Opened {} in IDE", branch_name));
                }
                Err(e) => {
                    browser.status_message = Some(format!("Failed to open IDE: {}", e));
                }
            }
            let worktrees = browser.build_worktree_list();
            browser.view = TuiView::WorktreeList {
                worktrees,
                selected: 0,
            };
        }
        KeyCode::Char('a') => {
            // Show instructions popup before starting agent
            browser.view = TuiView::WorktreeAgentInstructions {
                worktree_path: worktree_path.clone(),
                branch_name: branch_name.to_string(),
                input: String::new(),
            };
        }
        _ => {}
    }
}

pub fn handle_confirm_prune_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    orphaned: &[WorktreeInfo],
) {
    match key {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            let results = crate::agents::prune_worktrees(orphaned);
            let success_count = results.iter().filter(|(_, r)| r.is_ok()).count();
            let fail_count = results.len() - success_count;

            // Also remove sessions for pruned worktrees
            let mut manager = crate::agents::SessionManager::load();
            for wt in orphaned.iter() {
                if let Some(issue_num) = wt.issue_number {
                    if let Some(session) = manager.get_by_issue(&wt.project, issue_num) {
                        let session_id = session.id.clone();
                        manager.remove(&session_id);
                    }
                }
            }
            let _ = manager.save();

            if fail_count > 0 {
                browser.status_message = Some(format!(
                    "Pruned {} worktrees ({} failed)",
                    success_count, fail_count
                ));
            } else {
                browser.status_message = Some(format!("Pruned {} worktrees", success_count));
            }
            browser.view = TuiView::List;
            if let Some(project) = browser.project_name.clone() {
                browser.refresh_sessions(&project);
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            browser.view = TuiView::List;
        }
        _ => {}
    }
}

pub fn handle_confirm_delete_worktree_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    worktree: &WorktreeInfo,
    return_index: usize,
) {
    match key {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            let wt = worktree.clone();
            let results = crate::agents::prune_worktrees(&[wt.clone()]);
            if let Some((name, result)) = results.first() {
                match result {
                    Ok(_) => {
                        browser.status_message = Some(format!("Deleted worktree: {}", name));
                        // Remove from session manager if exists
                        if let Some(issue_num) = wt.issue_number {
                            let mut manager = crate::agents::SessionManager::load();
                            if let Some(session) = manager.get_by_issue(&wt.project, issue_num) {
                                let session_id = session.id.clone();
                                manager.remove(&session_id);
                                let _ = manager.save();
                            }
                        }
                        // Refresh session cache to update issue list indicators
                        browser.refresh_sessions(&wt.project);
                    }
                    Err(e) => {
                        browser.status_message = Some(format!("Failed to delete: {}", e));
                    }
                }
            }
            // Return to worktree list
            let new_worktrees = browser.build_worktree_list();
            if new_worktrees.is_empty() {
                browser.view = TuiView::List;
            } else {
                let new_selected = return_index.min(new_worktrees.len().saturating_sub(1));
                browser.view = TuiView::WorktreeList {
                    worktrees: new_worktrees,
                    selected: new_selected,
                };
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            // Return to worktree list without deleting
            let worktrees = browser.build_worktree_list();
            if worktrees.is_empty() {
                browser.view = TuiView::List;
            } else {
                let new_selected = return_index.min(worktrees.len().saturating_sub(1));
                browser.view = TuiView::WorktreeList {
                    worktrees,
                    selected: new_selected,
                };
            }
        }
        _ => {}
    }
}
