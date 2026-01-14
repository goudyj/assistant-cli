//! Issue list view event handling.

use crate::tui::IssueBrowser;
use crate::tui_types::{CreateStage, IssueFilterFocus, TuiView};
use crossterm::event::KeyCode;

pub async fn handle_list_key(browser: &mut IssueBrowser, key: KeyCode) {
    match key {
        KeyCode::Esc => {
            if let Some(last_press) = browser.last_esc_press
                && last_press.elapsed() < std::time::Duration::from_secs(2)
            {
                browser.should_quit = true;
                return;
            }
            browser.last_esc_press = Some(std::time::Instant::now());
            browser.status_message = Some("Press ESC again to quit".to_string());
        }
        KeyCode::Char('q') => browser.should_quit = true,
        KeyCode::Tab => {
            // Switch to PR list
            browser.load_pull_requests().await;
            browser.view = TuiView::PullRequestList;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            browser.next();
            if let Some(selected) = browser.list_state.selected()
                && browser.has_next_page
                && selected >= browser.issues.len().saturating_sub(10)
            {
                browser.load_next_page().await;
            }
        }
        KeyCode::Up | KeyCode::Char('k') => browser.previous(),
        KeyCode::Enter => {
            if let Some(issue) = browser.selected_issue() {
                let number = issue.number;
                if let Ok(detail) = browser.github.get_issue(number).await {
                    browser.extract_images_from_issue(&detail);
                    browser.view = TuiView::Detail(detail);
                    browser.scroll_offset = 0;
                }
            }
        }
        KeyCode::Char('s') => {
            browser.view = TuiView::Search {
                input: String::new(),
            };
        }
        KeyCode::Char('/') => {
            let suggestions = browser.available_commands.clone();
            browser.view = TuiView::Command {
                input: String::new(),
                suggestions,
                selected: 0,
            };
        }
        KeyCode::Char('c') => {
            // Clear search and reload issues from GitHub
            browser.search_query = None;
            browser.status_message = Some("Reloading issues...".to_string());
            if let Ok(issues) = browser
                .github
                .list_issues(&browser.list_labels, &browser.list_state_filter, 100)
                .await
            {
                browser.all_issues = issues.clone();
                browser.issues = issues;
                browser.list_state.select(if browser.issues.is_empty() {
                    None
                } else {
                    Some(0)
                });
                browser.status_message = Some("Search cleared".to_string());
            }
        }
        KeyCode::Char(' ') => {
            if let Some(issue) = browser.selected_issue() {
                let number = issue.number;
                if browser.selected_issues.contains(&number) {
                    browser.selected_issues.remove(&number);
                } else {
                    browser.selected_issues.insert(number);
                }
            }
        }
        KeyCode::Char('d') => {
            handle_dispatch(browser).await;
        }
        KeyCode::Char('t') => {
            handle_open_tmux(browser);
        }
        KeyCode::Char('T') => {
            handle_open_any_tmux(browser);
        }
        KeyCode::Char('l') => {
            if let Some(issue) = browser.selected_issue() {
                if let Some(session) = browser.session_cache.get(&issue.number) {
                    let content = std::fs::read_to_string(&session.log_file).unwrap_or_default();
                    browser.view = TuiView::AgentLogs {
                        session_id: session.id.clone(),
                        content,
                        scroll: 0,
                    };
                } else {
                    browser.status_message = Some("No agent session for this issue".to_string());
                }
            }
        }
        KeyCode::Char('p') => {
            handle_create_pr(browser);
        }
        KeyCode::Char('K') => {
            handle_kill_agent(browser);
        }
        KeyCode::Char('o') => {
            handle_open_ide(browser);
        }
        KeyCode::Char('W') => {
            handle_cleanup_worktree(browser);
        }
        KeyCode::Char('C') => {
            if browser.project_labels.is_empty() {
                browser.status_message = Some("No project labels configured.".to_string());
            } else {
                browser.view = TuiView::CreateIssue {
                    input: String::new(),
                    stage: CreateStage::Description,
                };
            }
        }
        KeyCode::Char('N') => {
            browser.view = TuiView::DirectIssue {
                title: String::new(),
                body: String::new(),
                editing_body: false,
            };
        }
        KeyCode::Char('R') => {
            browser.status_message = Some("Refreshing...".to_string());
            browser.reload_issues().await;
            browser.status_message = Some("Refreshed".to_string());
        }
        KeyCode::Char('f') => {
            // Open issue filters popup
            let status_filter = browser.issue_status_filter.clone();
            let available_authors = browser.available_issue_authors.clone();
            browser.view = TuiView::IssueFilters {
                status_filter,
                author_filter: browser.issue_author_filter.clone(),
                available_authors: available_authors.clone(),
                focus: IssueFilterFocus::Status,
                selected_status: 0,
                selected_author: 0,
                author_input: String::new(),
                author_suggestions: available_authors,
            };
        }
        KeyCode::Char('?') => {
            browser.view = TuiView::Help;
        }
        _ => {}
    }
}

async fn handle_dispatch(browser: &mut IssueBrowser) {
    if browser.local_path.is_none() {
        browser.status_message = Some("No local_path configured for this project.".to_string());
    } else if browser.selected_issues.is_empty() {
        // Single issue dispatch - show instructions popup
        if let Some(issue) = browser.selected_issue() {
            let issue_number = issue.number;
            let project_name = browser.project_name.clone().unwrap_or_default();

            let tmux_name = crate::agents::tmux_session_name(&project_name, issue_number);
            if crate::agents::is_tmux_session_running(&tmux_name) {
                browser.status_message = Some(format!(
                    "Session already running for #{}. Use 't' to open tmux or 'K' to kill it.",
                    issue_number
                ));
            } else if let Ok(detail) = browser.github.get_issue(issue_number).await {
                // Open instructions popup instead of dispatching directly
                browser.view = TuiView::DispatchInstructions {
                    issue: detail,
                    input: String::new(),
                };
            }
        }
    } else {
        // Batch dispatch - no instructions popup, dispatch directly
        let project_name = browser.project_name.clone().unwrap_or_default();
        let local_path = browser.local_path.clone().unwrap();
        let mut dispatched = 0;
        let mut skipped = 0;

        let agent = crate::agents::get_agent(&browser.coding_agent);
        for issue_number in browser.selected_issues.iter() {
            let tmux_name = crate::agents::tmux_session_name(&project_name, *issue_number);
            if crate::agents::is_tmux_session_running(&tmux_name) {
                skipped += 1;
                continue;
            }
            if let Ok(detail) = browser.github.get_issue(*issue_number).await
                && crate::agents::dispatch_to_agent(
                    &detail,
                    &local_path,
                    &project_name,
                    &browser.coding_agent,
                    browser.base_branch.as_deref(),
                    None,
                )
                .await
                .is_ok()
            {
                dispatched += 1;
            }
        }

        if skipped > 0 {
            browser.status_message = Some(format!(
                "Dispatched {} issues ({} skipped, already running).",
                dispatched, skipped
            ));
        } else {
            browser.status_message =
                Some(format!("Dispatched {} issues to {}.", dispatched, agent.name()));
        }
        browser.selected_issues.clear();
        if let Some(project) = browser.project_name.clone() {
            browser.refresh_sessions(&project);
        }
    }
}

fn handle_open_tmux(browser: &mut IssueBrowser) {
    if let Some(issue) = browser.selected_issue() {
        let issue_number = issue.number;
        if let Some(project) = browser.project_name.clone() {
            let tmux_name = crate::agents::tmux_session_name(&project, issue_number);
            if crate::agents::is_tmux_session_running(&tmux_name) {
                let all_sessions = crate::agents::list_tmux_sessions();
                let current_idx = all_sessions
                    .iter()
                    .position(|s| s == &tmux_name)
                    .unwrap_or(0);

                let area = crossterm::terminal::size().unwrap_or((80, 24));
                match crate::embedded_term::EmbeddedTerminal::new(
                    &tmux_name,
                    area.1.saturating_sub(1),
                    area.0,
                ) {
                    Ok(term) => {
                        browser.embedded_term = Some(term);
                        browser.view = TuiView::EmbeddedTmux {
                            available_sessions: all_sessions,
                            current_index: current_idx,
                            return_to_worktrees: false,
                        };
                    }
                    Err(e) => {
                        browser.status_message = Some(format!("Failed to open terminal: {}", e));
                    }
                }
            } else {
                browser.status_message = Some("No active session for this issue".to_string());
            }
        } else {
            browser.status_message = Some("No project selected".to_string());
        }
    }
}

fn handle_open_any_tmux(browser: &mut IssueBrowser) {
    let all_sessions = crate::agents::list_tmux_sessions();
    if !all_sessions.is_empty() {
        let area = crossterm::terminal::size().unwrap_or((80, 24));
        match crate::embedded_term::EmbeddedTerminal::new(
            &all_sessions[0],
            area.1.saturating_sub(1),
            area.0,
        ) {
            Ok(term) => {
                browser.embedded_term = Some(term);
                browser.view = TuiView::EmbeddedTmux {
                    available_sessions: all_sessions,
                    current_index: 0,
                    return_to_worktrees: false,
                };
            }
            Err(e) => {
                browser.status_message = Some(format!("Failed to open terminal: {}", e));
            }
        }
    } else {
        browser.status_message = Some("No tmux sessions available".to_string());
    }
}

fn handle_create_pr(browser: &mut IssueBrowser) {
    if let Some(issue) = browser.selected_issue() {
        let issue_number = issue.number;
        if let Some(session) = browser.session_cache.get(&issue_number) {
            if session.is_running() {
                browser.status_message = Some("Agent is still running".to_string());
            } else if session.pr_url.is_some() {
                browser.status_message = Some("PR already created".to_string());
            } else {
                match crate::agents::create_pr(session, browser.base_branch.as_deref()) {
                    Ok(url) => {
                        browser.status_message = Some(format!("PR created: {}", url));
                    }
                    Err(e) => {
                        browser.status_message = Some(format!("Failed to create PR: {}", e));
                    }
                }
            }
        } else {
            browser.status_message = Some("No agent session for this issue".to_string());
        }
        if let Some(project) = browser.project_name.clone() {
            browser.refresh_sessions(&project);
        }
    }
}

fn handle_kill_agent(browser: &mut IssueBrowser) {
    if let Some(issue) = browser.selected_issue() {
        let issue_number = issue.number;
        if let Some(session) = browser.session_cache.get(&issue_number) {
            if session.is_running() {
                let _ = crate::agents::kill_agent(&session.id);
                browser.status_message = Some(format!("Killed agent for #{}", issue_number));
            } else {
                browser.status_message = Some("Agent is not running".to_string());
            }
        } else {
            browser.status_message = Some("No agent session for this issue".to_string());
        }
        if let Some(project) = browser.project_name.clone() {
            browser.refresh_sessions(&project);
        }
    }
}

fn handle_open_ide(browser: &mut IssueBrowser) {
    if let Some(issue) = browser.selected_issue() {
        let issue_number = issue.number;
        if let Some(session) = browser.session_cache.get(&issue_number) {
            let ide_cmd = browser.ide_command.as_deref();
            match crate::agents::open_in_ide(&session.worktree_path, ide_cmd) {
                Ok(_) => {
                    browser.status_message =
                        Some(format!("Opened worktree for #{} in IDE", issue_number));
                }
                Err(e) => {
                    browser.status_message = Some(format!("Failed to open IDE: {}", e));
                }
            }
        } else {
            browser.status_message = Some("No worktree for this issue".to_string());
        }
    }
}

fn handle_cleanup_worktree(browser: &mut IssueBrowser) {
    if let Some(issue) = browser.selected_issue() {
        let issue_number = issue.number;
        if let Some(session) = browser.session_cache.get(&issue_number).cloned() {
            if session.is_running() {
                browser.status_message = Some("Agent is still running".to_string());
            } else if let Some(parent) = session.worktree_path.parent()
                && let Some(grandparent) = parent.parent()
            {
                let _ =
                    crate::agents::remove_worktree(grandparent, &session.worktree_path, true);
                browser.status_message =
                    Some(format!("Cleaned up worktree for #{}", issue_number));

                let mut manager = crate::agents::SessionManager::load();
                manager.remove(&session.id);
                let _ = manager.save();
            }
        } else {
            browser.status_message = Some("No agent session for this issue".to_string());
        }
        if let Some(project) = browser.project_name.clone() {
            browser.refresh_sessions(&project);
        }
    }
}
