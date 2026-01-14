//! TUI event handling functions.

use crate::auth;
use crate::issues::IssueContent;
use crate::llm;
use crate::tui::format_comment_with_llm;
use crate::tui::IssueBrowser;
use crate::tui_image::display_image;
use crate::tui_types::{CommandSuggestion, CreateStage, TuiView};
use crate::tui_utils::open_url;

use crossterm::event::{KeyCode, KeyModifiers};

/// Handle keyboard events
pub async fn handle_key_event(browser: &mut IssueBrowser, key: KeyCode, modifiers: KeyModifiers) {
    // Clear status message on any keypress (except ESC for double-ESC logic)
    if key != KeyCode::Esc {
        browser.status_message = None;
        browser.last_esc_press = None;
    }

    match &mut browser.view {
        TuiView::List => match key {
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
            KeyCode::Down | KeyCode::Char('j') => {
                browser.next();
                if let Some(selected) = browser.list_state.selected()
                    && browser.has_next_page
                    && selected >= browser.issues.len().saturating_sub(5)
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
                    .list_issues(&browser.list_labels, &browser.list_state_filter, 50)
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
                if browser.local_path.is_none() {
                    browser.status_message =
                        Some("No local_path configured for this project.".to_string());
                } else if browser.selected_issues.is_empty() {
                    // Single issue dispatch - show instructions popup
                    if let Some(issue) = browser.selected_issue() {
                        let issue_number = issue.number;
                        let project_name = browser.project_name.clone().unwrap_or_default();

                        let tmux_name =
                            crate::agents::tmux_session_name(&project_name, issue_number);
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
                        let tmux_name =
                            crate::agents::tmux_session_name(&project_name, *issue_number);
                        if crate::agents::is_tmux_session_running(&tmux_name) {
                            skipped += 1;
                            continue;
                        }
                        if let Ok(detail) = browser.github.get_issue(*issue_number).await
                            && crate::agents::dispatch_to_agent(&detail, &local_path, &project_name, &browser.coding_agent, browser.base_branch.as_deref(), None)
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
            KeyCode::Char('t') => {
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
                                    browser.status_message =
                                        Some(format!("Failed to open terminal: {}", e));
                                }
                            }
                        } else {
                            browser.status_message =
                                Some("No active session for this issue".to_string());
                        }
                    } else {
                        browser.status_message = Some("No project selected".to_string());
                    }
                }
            }
            KeyCode::Char('T') => {
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
                            browser.status_message =
                                Some(format!("Failed to open terminal: {}", e));
                        }
                    }
                } else {
                    browser.status_message = Some("No tmux sessions available".to_string());
                }
            }
            KeyCode::Char('l') => {
                if let Some(issue) = browser.selected_issue() {
                    if let Some(session) = browser.session_cache.get(&issue.number) {
                        let content =
                            std::fs::read_to_string(&session.log_file).unwrap_or_default();
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
                                    browser.status_message =
                                        Some(format!("Failed to create PR: {}", e));
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
            KeyCode::Char('K') => {
                if let Some(issue) = browser.selected_issue() {
                    let issue_number = issue.number;
                    if let Some(session) = browser.session_cache.get(&issue_number) {
                        if session.is_running() {
                            let _ = crate::agents::kill_agent(&session.id);
                            browser.status_message =
                                Some(format!("Killed agent for #{}", issue_number));
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
            KeyCode::Char('o') => {
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
            KeyCode::Char('W') => {
                if let Some(issue) = browser.selected_issue() {
                    let issue_number = issue.number;
                    if let Some(session) = browser.session_cache.get(&issue_number).cloned() {
                        if session.is_running() {
                            browser.status_message = Some("Agent is still running".to_string());
                        } else if let Some(parent) = session.worktree_path.parent()
                            && let Some(grandparent) = parent.parent()
                        {
                            let _ = crate::agents::remove_worktree(
                                grandparent,
                                &session.worktree_path,
                                true,
                            );
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
            KeyCode::Char('?') => {
                browser.view = TuiView::Help;
            }
            _ => {}
        },
        TuiView::DispatchInstructions { issue, input } => match key {
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
        },
        TuiView::WorktreeAgentInstructions {
            worktree_path,
            branch_name,
            input,
        } => match key {
            KeyCode::Esc => {
                // Return to PostWorktreeCreate
                browser.view = TuiView::PostWorktreeCreate {
                    worktree_path: worktree_path.clone(),
                    branch_name: branch_name.clone(),
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
        },
        TuiView::Help => match key {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                browser.view = TuiView::List;
            }
            _ => {}
        },
        TuiView::Search { input } => match key {
            KeyCode::Esc => {
                browser.view = TuiView::List;
            }
            KeyCode::Enter => {
                let query = input.clone();
                if query.is_empty() {
                    // Empty query: reload all issues
                    browser.search_query = None;
                    browser.status_message = Some("Loading issues...".to_string());
                    browser.view = TuiView::List;
                    if let Ok(issues) = browser
                        .github
                        .list_issues(&browser.list_labels, &browser.list_state_filter, 50)
                        .await
                    {
                        browser.all_issues = issues.clone();
                        browser.issues = issues;
                        browser.list_state.select(Some(0));
                        browser.status_message = None;
                    }
                } else {
                    // Search GitHub
                    browser.status_message = Some(format!("Searching '{}'...", query));
                    browser.view = TuiView::List;
                    match browser.github.search_issues(&query).await {
                        Ok(results) => {
                            browser.search_query = Some(query);
                            browser.issues = results;
                            browser.list_state.select(if browser.issues.is_empty() {
                                None
                            } else {
                                Some(0)
                            });
                            browser.status_message = Some(format!(
                                "Found {} issue{}",
                                browser.issues.len(),
                                if browser.issues.len() == 1 { "" } else { "s" }
                            ));
                        }
                        Err(e) => {
                            browser.status_message = Some(format!("Search error: {}", e));
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
        },
        TuiView::Detail(issue) => match key {
            KeyCode::Esc | KeyCode::Char('q') => {
                browser.view = TuiView::List;
                browser.scroll_offset = 0;
                browser.current_images.clear();
            }
            KeyCode::Down | KeyCode::Char('j') => browser.scroll_offset += 1,
            KeyCode::Up | KeyCode::Char('k') => {
                browser.scroll_offset = browser.scroll_offset.saturating_sub(1);
            }
            KeyCode::Char('c') => {
                let issue_clone = issue.clone();
                browser.view = TuiView::AddComment {
                    issue: issue_clone,
                    input: String::new(),
                };
                browser.status_message = None;
            }
            KeyCode::Char('o') => {
                open_url(&issue.html_url);
                browser.status_message = Some("Opened in browser".to_string());
            }
            KeyCode::Char('O') => {
                if !browser.current_images.is_empty() {
                    let url = &browser.current_images[browser.current_image_index];
                    open_url(url);
                    browser.status_message = Some("Image opened in browser".to_string());
                    browser.current_image_index =
                        (browser.current_image_index + 1) % browser.current_images.len();
                } else {
                    browser.status_message = Some("No images".to_string());
                }
            }
            KeyCode::Char('i') => {
                if !browser.current_images.is_empty() {
                    let url = browser.current_images[browser.current_image_index].clone();
                    let token = browser.github_token.clone();
                    if let Err(e) = display_image(&url, token.as_deref()).await {
                        browser.status_message = Some(format!("Image error: {}", e));
                    }
                    browser.current_image_index =
                        (browser.current_image_index + 1) % browser.current_images.len();
                } else {
                    browser.status_message = Some("No images in this issue".to_string());
                }
            }
            KeyCode::Char('x') => {
                if issue.state == "Open" {
                    let issue_clone = issue.clone();
                    browser.view = TuiView::ConfirmClose { issue: issue_clone };
                } else {
                    browser.status_message = Some("Issue is already closed".to_string());
                }
            }
            KeyCode::Char('X') => {
                if issue.state == "Closed" {
                    let issue_clone = issue.clone();
                    browser.view = TuiView::ConfirmReopen { issue: issue_clone };
                } else {
                    browser.status_message = Some("Issue is already open".to_string());
                }
            }
            KeyCode::Char('a') => {
                let issue_clone = issue.clone();
                browser.load_assignees().await;
                let suggestions = browser.get_assignee_suggestions("");
                browser.view = TuiView::AssignUser {
                    issue: issue_clone,
                    input: String::new(),
                    suggestions,
                    selected: 0,
                };
            }
            KeyCode::Char('d') => {
                if let Some(project) = browser.project_name.clone() {
                    let tmux_name = crate::agents::tmux_session_name(&project, issue.number);
                    if crate::agents::is_tmux_session_running(&tmux_name) {
                        browser.status_message = Some(format!(
                            "Session already running for #{}. Use 't' to open tmux or 'K' to kill it.",
                            issue.number
                        ));
                    } else {
                        let issue_clone = issue.clone();
                        browser.view = TuiView::ConfirmDispatch { issue: issue_clone };
                    }
                } else {
                    browser.status_message = Some("No project selected".to_string());
                }
            }
            _ => {}
        },
        TuiView::AddComment { issue, input } => match key {
            KeyCode::Esc => {
                let number = issue.number;
                if let Ok(detail) = browser.github.get_issue(number).await {
                    browser.view = TuiView::Detail(detail);
                } else {
                    browser.view = TuiView::List;
                }
                browser.status_message = None;
            }
            KeyCode::Enter => {
                if !input.is_empty() {
                    let comment_body = if browser.auto_format {
                        browser.status_message = Some("Formatting...".to_string());
                        format_comment_with_llm(input, &browser.llm_endpoint)
                            .await
                            .unwrap_or_else(|_| input.clone())
                    } else {
                        input.clone()
                    };

                    browser.status_message = Some("Sending...".to_string());
                    let number = issue.number;
                    if browser.github.add_comment(number, &comment_body).await.is_ok() {
                        if let Ok(detail) = browser.github.get_issue(number).await {
                            browser.view = TuiView::Detail(detail);
                        } else {
                            browser.view = TuiView::List;
                        }
                    } else {
                        browser.view = TuiView::List;
                    }
                    browser.status_message = None;
                }
            }
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Char(c) => {
                input.push(c);
            }
            _ => {}
        },
        TuiView::ConfirmClose { issue } => match key {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let number = issue.number;
                if browser.github.close_issue(number).await.is_ok() {
                    browser.status_message = Some(format!("Issue #{} closed", number));
                    if let Some(pos) = browser.issues.iter().position(|i| i.number == number) {
                        browser.issues[pos].state = "Closed".to_string();
                    }
                    browser.view = TuiView::List;
                } else {
                    browser.status_message = Some("Failed to close issue".to_string());
                    if let Ok(detail) = browser.github.get_issue(number).await {
                        browser.view = TuiView::Detail(detail);
                    } else {
                        browser.view = TuiView::List;
                    }
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                let number = issue.number;
                if let Ok(detail) = browser.github.get_issue(number).await {
                    browser.view = TuiView::Detail(detail);
                } else {
                    browser.view = TuiView::List;
                }
            }
            _ => {}
        },
        TuiView::ConfirmReopen { issue } => match key {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let number = issue.number;
                if browser.github.reopen_issue(number).await.is_ok() {
                    browser.status_message = Some(format!("Issue #{} reopened", number));
                    if let Some(pos) = browser.issues.iter().position(|i| i.number == number) {
                        browser.issues[pos].state = "Open".to_string();
                    }
                    if let Ok(detail) = browser.github.get_issue(number).await {
                        browser.view = TuiView::Detail(detail);
                    } else {
                        browser.view = TuiView::List;
                    }
                } else {
                    browser.status_message = Some("Failed to reopen issue".to_string());
                    if let Ok(detail) = browser.github.get_issue(number).await {
                        browser.view = TuiView::Detail(detail);
                    } else {
                        browser.view = TuiView::List;
                    }
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                let number = issue.number;
                if let Ok(detail) = browser.github.get_issue(number).await {
                    browser.view = TuiView::Detail(detail);
                } else {
                    browser.view = TuiView::List;
                }
            }
            _ => {}
        },
        TuiView::AssignUser {
            issue,
            input,
            suggestions,
            selected,
        } => {
            let number = issue.number;
            let current_assignees = issue.assignees.clone();
            let input_str = input.clone();
            let sel = *selected;

            match key {
                KeyCode::Esc => {
                    if let Ok(detail) = browser.github.get_issue(number).await {
                        browser.view = TuiView::Detail(detail);
                    } else {
                        browser.view = TuiView::List;
                    }
                }
                KeyCode::Up => {
                    if let TuiView::AssignUser { selected, .. } = &mut browser.view
                        && *selected > 0
                    {
                        *selected -= 1;
                    }
                }
                KeyCode::Down => {
                    let sugg_len = suggestions.len();
                    if let TuiView::AssignUser { selected, .. } = &mut browser.view
                        && *selected < sugg_len.saturating_sub(1)
                    {
                        *selected += 1;
                    }
                }
                KeyCode::Enter => {
                    if let Some(user) = suggestions.get(sel) {
                        let user_to_assign = user.clone();

                        if current_assignees.contains(&user_to_assign) {
                            browser.status_message =
                                Some(format!("{} is already assigned", user_to_assign));
                        } else if browser
                            .github
                            .assign_issue(number, std::slice::from_ref(&user_to_assign))
                            .await
                            .is_ok()
                        {
                            browser.status_message =
                                Some(format!("Assigned {} to #{}", user_to_assign, number));
                            if let Some(pos) = browser.issues.iter().position(|i| i.number == number)
                            {
                                browser.issues[pos].assignees.push(user_to_assign);
                            }
                        } else {
                            browser.status_message = Some("Failed to assign user".to_string());
                        }

                        if let Ok(detail) = browser.github.get_issue(number).await {
                            browser.view = TuiView::Detail(detail);
                        } else {
                            browser.view = TuiView::List;
                        }
                    }
                }
                KeyCode::Char('-') => {
                    if !current_assignees.is_empty() {
                        let user_to_remove = current_assignees[0].clone();

                        if browser
                            .github
                            .unassign_issue(number, std::slice::from_ref(&user_to_remove))
                            .await
                            .is_ok()
                        {
                            browser.status_message =
                                Some(format!("Unassigned {} from #{}", user_to_remove, number));
                            if let Some(pos) = browser.issues.iter().position(|i| i.number == number)
                            {
                                browser.issues[pos]
                                    .assignees
                                    .retain(|u| u != &user_to_remove);
                            }
                        } else {
                            browser.status_message = Some("Failed to unassign user".to_string());
                        }

                        if let Ok(detail) = browser.github.get_issue(number).await {
                            let new_suggestions = browser.get_assignee_suggestions(&input_str);
                            browser.view = TuiView::AssignUser {
                                issue: detail,
                                input: input_str,
                                suggestions: new_suggestions,
                                selected: 0,
                            };
                        } else {
                            browser.view = TuiView::List;
                        }
                    }
                }
                KeyCode::Backspace => {
                    let mut new_input = input_str.clone();
                    new_input.pop();
                    let new_suggestions = browser.get_assignee_suggestions(&new_input);
                    if let TuiView::AssignUser {
                        input: ref mut inp,
                        suggestions: ref mut sug,
                        selected: ref mut sel,
                        ..
                    } = browser.view
                    {
                        *inp = new_input;
                        *sug = new_suggestions;
                        *sel = 0;
                    }
                }
                KeyCode::Char(c) => {
                    let mut new_input = input_str.clone();
                    new_input.push(c);
                    let new_suggestions = browser.get_assignee_suggestions(&new_input);
                    if let TuiView::AssignUser {
                        input: ref mut inp,
                        suggestions: ref mut sug,
                        selected: ref mut sel,
                        ..
                    } = browser.view
                    {
                        *inp = new_input;
                        *sug = new_suggestions;
                        *sel = 0;
                    }
                }
                _ => {}
            }
        }
        TuiView::ConfirmDispatch { issue } => match key {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let number = issue.number;

                if let (Some(project), Some(local_path)) =
                    (&browser.project_name, &browser.local_path)
                {
                    let tmux_name = crate::agents::tmux_session_name(project, number);
                    if crate::agents::is_tmux_session_running(&tmux_name) {
                        browser.status_message = Some(format!(
                            "Session already running for #{}. Use 't' to open tmux or 'K' to kill it.",
                            number
                        ));
                    } else {
                        let agent = crate::agents::get_agent(&browser.coding_agent);
                        match crate::agents::dispatch_to_agent(issue, local_path, project, &browser.coding_agent, browser.base_branch.as_deref(), None).await {
                            Ok(session) => {
                                browser.status_message = Some(format!(
                                    "Dispatched #{} to {} (session {})",
                                    number,
                                    agent.name(),
                                    &session.id[..8]
                                ));
                            }
                            Err(e) => {
                                browser.status_message = Some(format!("Failed to dispatch: {}", e));
                            }
                        }
                    }
                } else {
                    browser.status_message = Some(
                        "No local_path configured for this project. Add it to assistant.json"
                            .to_string(),
                    );
                }

                if let Ok(detail) = browser.github.get_issue(number).await {
                    browser.view = TuiView::Detail(detail);
                } else {
                    browser.view = TuiView::List;
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                let number = issue.number;
                if let Ok(detail) = browser.github.get_issue(number).await {
                    browser.view = TuiView::Detail(detail);
                } else {
                    browser.view = TuiView::List;
                }
            }
            _ => {}
        },
        TuiView::AgentLogs { scroll, .. } => match key {
            KeyCode::Esc | KeyCode::Char('q') => {
                browser.view = TuiView::List;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                *scroll = scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                *scroll += 1;
            }
            KeyCode::PageUp => {
                *scroll = scroll.saturating_sub(20);
            }
            KeyCode::PageDown => {
                *scroll += 20;
            }
            _ => {}
        },
        TuiView::EmbeddedTmux {
            available_sessions,
            current_index,
            return_to_worktrees,
        } => {
            let has_modifier =
                modifiers.contains(KeyModifiers::CONTROL) || modifiers.contains(KeyModifiers::SUPER);

            if key == KeyCode::Char('q') && modifiers.contains(KeyModifiers::CONTROL) {
                // Ctrl+Q to exit embedded terminal
                browser.embedded_term = None;
                browser.last_esc_press = None;
                if *return_to_worktrees {
                    let worktrees = browser.build_worktree_list();
                    browser.view = TuiView::WorktreeList {
                        worktrees,
                        selected: 0,
                    };
                } else {
                    browser.view = TuiView::List;
                }
                if let Some(project) = browser.project_name.clone() {
                    browser.refresh_sessions_with_fresh_stats(&project);
                }
                return;
            } else if key == KeyCode::Esc {
                // Single ESC passes through to tmux
                if let Some(ref term) = browser.embedded_term {
                    term.send_input(&[0x1b]); // ESC byte
                }
            } else if key == KeyCode::Left && has_modifier {
                // Ctrl+Left or CMD+Left: switch to previous session
                if !available_sessions.is_empty() && *current_index > 0 {
                    *current_index -= 1;
                    let session_name = &available_sessions[*current_index];
                    let area = crossterm::terminal::size().unwrap_or((80, 24));
                    if let Ok(term) = crate::embedded_term::EmbeddedTerminal::new(
                        session_name,
                        area.1.saturating_sub(1),
                        area.0,
                    ) {
                        browser.embedded_term = Some(term);
                    }
                }
            } else if key == KeyCode::Right && has_modifier {
                // Ctrl+Right or CMD+Right: switch to next session
                if !available_sessions.is_empty()
                    && *current_index < available_sessions.len() - 1
                {
                    *current_index += 1;
                    let session_name = &available_sessions[*current_index];
                    let area = crossterm::terminal::size().unwrap_or((80, 24));
                    if let Ok(term) = crate::embedded_term::EmbeddedTerminal::new(
                        session_name,
                        area.1.saturating_sub(1),
                        area.0,
                    ) {
                        browser.embedded_term = Some(term);
                    }
                }
            } else {
                // All other keys pass through to terminal
                if let Some(ref term) = browser.embedded_term {
                    term.send_key_with_modifiers(key, modifiers);
                }
            }
        }
        TuiView::ProjectSelect { projects, selected } => match key {
            KeyCode::Esc => {
                browser.view = TuiView::List;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if *selected > 0 {
                    *selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if *selected < projects.len().saturating_sub(1) {
                    *selected += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(project_name) = projects.get(*selected).cloned() {
                    // Find the project config
                    if let Some((_, project_config)) = browser
                        .available_projects
                        .iter()
                        .find(|(name, _)| name == &project_name)
                    {
                        let project_config = project_config.clone();
                        let token = browser.github_token.clone().unwrap_or_default();
                        browser.status_message =
                            Some(format!("Switching to {}...", project_name));
                        browser.view = TuiView::List;
                        browser
                            .switch_project(&project_name, &project_config, &token)
                            .await;
                        browser.status_message =
                            Some(format!("Switched to {}", project_name));
                    } else {
                        browser.view = TuiView::List;
                        browser.status_message = Some("Project not found".to_string());
                    }
                } else {
                    browser.view = TuiView::List;
                }
            }
            _ => {}
        },
        TuiView::AgentSelect { selected } => match key {
            KeyCode::Esc => {
                browser.view = TuiView::List;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if *selected > 0 {
                    *selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if *selected < 1 {
                    *selected += 1;
                }
            }
            KeyCode::Enter => {
                let new_agent = if *selected == 0 {
                    crate::config::CodingAgentType::Claude
                } else {
                    crate::config::CodingAgentType::Opencode
                };
                let agent_name = crate::agents::get_agent(&new_agent).name();
                browser.coding_agent = new_agent;
                browser.view = TuiView::List;
                browser.status_message = Some(format!("Dispatch agent set to {}.", agent_name));
            }
            _ => {}
        },
        TuiView::Command {
            input,
            suggestions,
            selected,
        } => match key {
            KeyCode::Esc => {
                browser.view = TuiView::List;
            }
            KeyCode::Up => {
                if *selected > 0 {
                    *selected -= 1;
                }
            }
            KeyCode::Down => {
                if *selected < suggestions.len().saturating_sub(1) {
                    *selected += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(cmd) = suggestions.get(*selected) {
                    let cmd_name = cmd.name.clone();
                    let labels = cmd.labels.clone();
                    browser.view = TuiView::List;

                    match cmd_name.as_str() {
                        "all" => {
                            browser.list_labels.clear();
                            browser.status_message = Some("Loading all issues...".to_string());
                            browser.reload_issues().await;
                            browser.status_message = Some("Showing all issues".to_string());
                        }
                        "logout" => {
                            let _ = auth::delete_token();
                            browser.status_message = Some("Logged out.".to_string());
                            browser.should_quit = true;
                        }
                        "repository" | "repo" => {
                            let mut projects: Vec<String> = browser
                                .available_projects
                                .iter()
                                .map(|(name, _)| name.clone())
                                .collect();
                            projects.sort();
                            if projects.is_empty() {
                                browser.status_message =
                                    Some("No projects configured.".to_string());
                            } else {
                                browser.view = TuiView::ProjectSelect {
                                    projects,
                                    selected: 0,
                                };
                            }
                        }
                        "worktrees" => {
                            let worktrees = browser.build_worktree_list();
                            browser.view = TuiView::WorktreeList {
                                worktrees,
                                selected: 0,
                            };
                        }
                        "prune" => {
                            let orphaned = browser.get_orphaned_worktrees();
                            if orphaned.is_empty() {
                                browser.status_message =
                                    Some("No orphaned worktrees to clean up.".to_string());
                            } else {
                                browser.view = TuiView::ConfirmPrune { orphaned };
                            }
                        }
                        "agent" => {
                            browser.view = TuiView::AgentSelect { selected: 0 };
                        }
                        _ => {
                            if let Some(filter_labels) = labels {
                                browser.list_labels = filter_labels.clone();
                                browser.status_message =
                                    Some(format!("Loading /{} filter...", cmd_name));
                                browser.reload_issues().await;
                                browser.status_message =
                                    Some(format!("Filter applied: /{}", cmd_name));
                            }
                        }
                    }
                } else {
                    browser.view = TuiView::List;
                }
            }
            KeyCode::Backspace => {
                input.pop();
                let input_clone = input.clone();
                let available = browser.available_commands.clone();
                *suggestions = filter_commands(&available, &input_clone);
                *selected = 0;
            }
            KeyCode::Char(c) => {
                input.push(c);
                let input_clone = input.clone();
                let available = browser.available_commands.clone();
                *suggestions = filter_commands(&available, &input_clone);
                *selected = 0;
            }
            _ => {}
        },
        TuiView::CreateIssue { input, stage } => match key {
            KeyCode::Esc => {
                browser.view = TuiView::List;
            }
            KeyCode::Enter => {
                if matches!(stage, CreateStage::Description) && !input.is_empty() {
                    let description = input.clone();
                    let labels = browser.project_labels.clone();
                    let endpoint = browser.llm_endpoint.clone();

                    *stage = CreateStage::Generating;

                    match crate::issues::generate_issue_with_labels(&description, &labels, &endpoint)
                        .await
                    {
                        Ok((issue, messages)) => {
                            browser.view = TuiView::PreviewIssue {
                                issue,
                                messages,
                                feedback_input: String::new(),
                                scroll: 0,
                            };
                        }
                        Err(e) => {
                            browser.status_message = Some(format!("Generation failed: {}", e));
                            browser.view = TuiView::List;
                        }
                    }
                }
            }
            KeyCode::Backspace => {
                if matches!(stage, CreateStage::Description) {
                    input.pop();
                }
            }
            KeyCode::Char(c) => {
                if matches!(stage, CreateStage::Description) {
                    input.push(c);
                }
            }
            _ => {}
        },
        TuiView::PreviewIssue {
            issue,
            messages,
            feedback_input,
            scroll,
        } => match key {
            KeyCode::Esc => {
                browser.view = TuiView::List;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                *scroll = scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                *scroll += 1;
            }
            KeyCode::Enter => {
                if feedback_input.is_empty() {
                    let issue_clone = issue.clone();
                    match browser.github.create_issue(&issue_clone).await {
                        Ok((url, new_issue)) => {
                            browser.status_message = Some(format!("Issue created: {}", url));
                            browser.all_issues.insert(0, new_issue.clone());
                            browser.issues.insert(0, new_issue);
                            *browser.list_state.offset_mut() = 0;
                            browser.list_state.select(Some(0));
                            browser.view = TuiView::List;
                        }
                        Err(e) => {
                            browser.status_message = Some(format!("Failed to create: {}", e));
                        }
                    }
                } else {
                    let feedback = feedback_input.clone();
                    let endpoint = browser.llm_endpoint.clone();

                    messages.push(llm::Message {
                        role: "user".to_string(),
                        content: feedback,
                    });

                    match llm::generate_response(messages, &endpoint).await {
                        Ok(response) => {
                            if let Ok(updated_issue) =
                                serde_json::from_str::<IssueContent>(&response.message.content)
                            {
                                messages.push(llm::Message {
                                    role: "assistant".to_string(),
                                    content: serde_json::to_string(&updated_issue).unwrap_or_default(),
                                });
                                *issue = updated_issue;
                                *feedback_input = String::new();
                                *scroll = 0;
                            } else {
                                browser.status_message =
                                    Some("Failed to parse updated issue.".to_string());
                            }
                        }
                        Err(e) => {
                            browser.status_message = Some(format!("Refinement failed: {}", e));
                        }
                    }
                }
            }
            KeyCode::Backspace => {
                feedback_input.pop();
            }
            KeyCode::Char(c) => {
                feedback_input.push(c);
            }
            _ => {}
        },
        TuiView::DirectIssue {
            title,
            body,
            editing_body,
        } => {
            match key {
                KeyCode::Esc => {
                    browser.view = TuiView::List;
                }
                KeyCode::Tab => {
                    *editing_body = !*editing_body;
                }
                KeyCode::Enter if modifiers.contains(KeyModifiers::SHIFT) => {
                    if title.is_empty() {
                        browser.status_message = Some("Title cannot be empty".to_string());
                    } else {
                        let issue = IssueContent {
                            type_: "task".to_string(),
                            title: title.clone(),
                            body: body.clone(),
                            labels: Vec::new(),
                        };
                        match browser.github.create_issue(&issue).await {
                            Ok((url, new_issue)) => {
                                browser.status_message = Some(format!("Issue created: {}", url));
                                browser.all_issues.insert(0, new_issue.clone());
                                browser.issues.insert(0, new_issue);
                                *browser.list_state.offset_mut() = 0;
                                browser.list_state.select(Some(0));
                                browser.view = TuiView::List;
                            }
                            Err(e) => {
                                browser.status_message = Some(format!("Failed to create: {}", e));
                            }
                        }
                    }
                }
                KeyCode::Char('s') | KeyCode::Char('j')
                    if modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    if title.is_empty() {
                        browser.status_message = Some("Title cannot be empty".to_string());
                    } else {
                        let issue = IssueContent {
                            type_: "task".to_string(),
                            title: title.clone(),
                            body: body.clone(),
                            labels: Vec::new(),
                        };
                        match browser.github.create_issue(&issue).await {
                            Ok((url, new_issue)) => {
                                browser.status_message = Some(format!("Issue created: {}", url));
                                browser.all_issues.insert(0, new_issue.clone());
                                browser.issues.insert(0, new_issue);
                                *browser.list_state.offset_mut() = 0;
                                browser.list_state.select(Some(0));
                                browser.view = TuiView::List;
                            }
                            Err(e) => {
                                browser.status_message = Some(format!("Failed to create: {}", e));
                            }
                        }
                    }
                }
                KeyCode::Enter => {
                    if *editing_body {
                        body.push('\n');
                    } else {
                        *editing_body = true;
                    }
                }
                KeyCode::Backspace => {
                    if *editing_body {
                        body.pop();
                    } else {
                        title.pop();
                    }
                }
                KeyCode::Char(c) => {
                    if *editing_body {
                        body.push(c);
                    } else {
                        title.push(c);
                    }
                }
                _ => {}
            }
        }
        TuiView::WorktreeList {
            worktrees,
            selected,
        } => match key {
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
                            browser.status_message =
                                Some(format!("Opened {} in IDE", wt.name));
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
                                match crate::agents::create_pr(session, browser.base_branch.as_deref()) {
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
                        browser.status_message = Some("Tmux session still running. Kill it first (K).".to_string());
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
                        browser.status_message = Some("No tmux session running for this worktree".to_string());
                    }
                }
            }
            _ => {}
        },
        TuiView::CreateWorktree { input } => match key {
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
        },
        TuiView::PostWorktreeCreate {
            worktree_path,
            branch_name,
        } => match key {
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
                    branch_name: branch_name.clone(),
                    input: String::new(),
                };
            }
            _ => {}
        },
        TuiView::ConfirmPrune { orphaned } => match key {
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
        },
        TuiView::ConfirmDeleteWorktree {
            worktree,
            return_index,
        } => match key {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let wt = worktree.clone();
                let selected_idx = *return_index;
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
                    let new_selected = selected_idx.min(new_worktrees.len().saturating_sub(1));
                    browser.view = TuiView::WorktreeList {
                        worktrees: new_worktrees,
                        selected: new_selected,
                    };
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                // Return to worktree list without deleting
                let selected_idx = *return_index;
                let worktrees = browser.build_worktree_list();
                if worktrees.is_empty() {
                    browser.view = TuiView::List;
                } else {
                    let new_selected = selected_idx.min(worktrees.len().saturating_sub(1));
                    browser.view = TuiView::WorktreeList {
                        worktrees,
                        selected: new_selected,
                    };
                }
            }
            _ => {}
        },
    }
}

/// Filter commands based on input
fn filter_commands(commands: &[CommandSuggestion], input: &str) -> Vec<CommandSuggestion> {
    if input.is_empty() {
        commands.to_vec()
    } else {
        let input_lower = input.to_lowercase();
        commands
            .iter()
            .filter(|cmd| cmd.name.to_lowercase().contains(&input_lower))
            .cloned()
            .collect()
    }
}

/// Handle pasted content into input fields
pub fn handle_paste(browser: &mut IssueBrowser, content: &str) {
    let clean_content = content.replace('\r', "");

    match &mut browser.view {
        TuiView::Search { input } => {
            input.push_str(&clean_content.replace('\n', " "));
        }
        TuiView::Command {
            input,
            suggestions,
            selected,
        } => {
            input.push_str(&clean_content.replace('\n', " "));
            let input_clone = input.clone();
            let available = browser.available_commands.clone();
            *suggestions = filter_commands(&available, &input_clone);
            *selected = 0;
        }
        TuiView::CreateIssue { input, stage } => {
            if matches!(stage, CreateStage::Description) {
                input.push_str(&clean_content);
            }
        }
        TuiView::PreviewIssue { feedback_input, .. } => {
            feedback_input.push_str(&clean_content);
        }
        TuiView::DirectIssue {
            title,
            body,
            editing_body,
        } => {
            if *editing_body {
                body.push_str(&clean_content);
            } else {
                title.push_str(&clean_content.replace('\n', " "));
            }
        }
        TuiView::AddComment { input, .. } => {
            input.push_str(&clean_content);
        }
        TuiView::AssignUser { input, .. } => {
            input.push_str(&clean_content.replace('\n', " "));
        }
        TuiView::DispatchInstructions { input, .. } => {
            input.push_str(&clean_content);
        }
        TuiView::WorktreeAgentInstructions { input, .. } => {
            input.push_str(&clean_content);
        }
        _ => {}
    }
}
