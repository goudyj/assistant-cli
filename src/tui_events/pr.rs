//! Pull request views event handling.

use crate::github::PullRequestDetail;
use crate::tui::IssueBrowser;
use crate::tui_types::{PrFilterFocus, PrStatus, TuiView};
use crate::tui_utils::open_url;
use crossterm::event::KeyCode;

pub async fn handle_pr_list_key(browser: &mut IssueBrowser, key: KeyCode) {
    match key {
        KeyCode::Esc | KeyCode::Char('q') => {
            browser.view = TuiView::List;
        }
        KeyCode::Tab => {
            browser.view = TuiView::List;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            browser.pr_next();
            if let Some(selected) = browser.pr_list_state.selected()
                && browser.pr_has_next_page
                && selected >= browser.pull_requests.len().saturating_sub(10)
            {
                browser.load_next_pr_page().await;
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            browser.pr_previous();
        }
        KeyCode::Enter => {
            if let Some(pr) = browser.selected_pr() {
                let number = pr.number;
                if let Ok(detail) = browser.github.get_pull_request(number).await {
                    browser.view = TuiView::PullRequestDetail(detail);
                    browser.scroll_offset = 0;
                }
            }
        }
        KeyCode::Char('o') => {
            if let Some(pr) = browser.selected_pr() {
                open_url(&pr.html_url);
            }
        }
        KeyCode::Char('c') => {
            // Checkout PR branch as worktree
            if let Some(pr) = browser.selected_pr() {
                if browser.local_path.is_none() {
                    browser.status_message =
                        Some("No local_path configured for this project.".to_string());
                    return;
                }
                let branch = pr.head_ref.clone();
                browser.status_message =
                    Some(format!("Creating worktree for branch: {}", branch));
                // Reuse existing worktree creation logic
                match browser.create_worktree_for_branch(&branch) {
                    Ok((path, _)) => {
                        browser.status_message =
                            Some(format!("Worktree created at: {}", path.display()));
                    }
                    Err(e) => {
                        browser.status_message =
                            Some(format!("Failed to create worktree: {}", e));
                    }
                }
            }
        }
        KeyCode::Char('r') => {
            // Review PR with agent
            if let Some(pr) = browser.selected_pr() {
                let number = pr.number;
                if let Ok(detail) = browser.github.get_pull_request(number).await {
                    browser.view = TuiView::DispatchPrReview {
                        pr: detail,
                        input: String::new(),
                    };
                }
            }
        }
        KeyCode::Char('m') => {
            // Merge PR
            if let Some(pr) = browser.selected_pr() {
                let number = pr.number;
                if let Ok(detail) = browser.github.get_pull_request(number).await {
                    if detail.mergeable == Some(false) {
                        browser.status_message = Some("PR is not mergeable".to_string());
                    } else {
                        browser.view = TuiView::ConfirmMerge { pr: detail };
                    }
                }
            }
        }
        KeyCode::Char('f') => {
            // Open filters
            let mut status_filter = browser.pr_status_filter.clone();
            if status_filter.is_empty() {
                status_filter.insert(PrStatus::Open);
            }
            let available_authors = browser.available_pr_authors.clone();
            // Find index of first selected author to position cursor correctly
            let selected_author = available_authors
                .iter()
                .position(|a| browser.pr_author_filter.contains(a))
                .unwrap_or(0);
            browser.view = TuiView::PrFilters {
                status_filter,
                author_filter: browser.pr_author_filter.clone(),
                available_authors: available_authors.clone(),
                focus: PrFilterFocus::Status,
                selected_status: 0,
                selected_author,
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

pub fn handle_pr_detail_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    pr: &PullRequestDetail,
) {
    match key {
        KeyCode::Esc | KeyCode::Char('q') => {
            browser.view = TuiView::PullRequestList;
            browser.scroll_offset = 0;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            browser.scroll_offset = browser.scroll_offset.saturating_add(1);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            browser.scroll_offset = browser.scroll_offset.saturating_sub(1);
        }
        KeyCode::Char('o') => {
            open_url(&pr.html_url);
        }
        KeyCode::Char('m') => {
            if pr.mergeable == Some(false) {
                browser.status_message = Some("PR is not mergeable".to_string());
            } else {
                browser.view = TuiView::ConfirmMerge { pr: pr.clone() };
            }
        }
        KeyCode::Char('r') => {
            browser.view = TuiView::DispatchPrReview {
                pr: pr.clone(),
                input: String::new(),
            };
        }
        _ => {}
    }
}

pub async fn handle_confirm_merge_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    pr: &PullRequestDetail,
) {
    match key {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            browser.status_message = Some("Merging PR...".to_string());
            match browser.github.merge_pull_request(pr.number, None).await {
                Ok(()) => {
                    browser.status_message = Some(format!("PR #{} merged!", pr.number));
                    // Reload PR list
                    browser.reload_pull_requests().await;
                    browser.view = TuiView::PullRequestList;
                }
                Err(e) => {
                    browser.status_message = Some(format!("Failed to merge: {}", e));
                    browser.view = TuiView::PullRequestDetail(pr.clone());
                }
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            browser.view = TuiView::PullRequestDetail(pr.clone());
        }
        _ => {}
    }
}

pub async fn handle_dispatch_pr_review_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    pr: &PullRequestDetail,
    input: &mut String,
) {
    match key {
        KeyCode::Esc => {
            browser.view = TuiView::PullRequestDetail(pr.clone());
        }
        KeyCode::Enter => {
            // Start agent review
            if browser.local_path.is_none() {
                browser.status_message =
                    Some("No local_path configured for this project.".to_string());
                browser.view = TuiView::PullRequestDetail(pr.clone());
                return;
            }

            // Create worktree for PR branch
            let branch = pr.head_ref.clone();
            match browser.create_worktree_for_branch(&branch) {
                Ok((worktree_path, _)) => {
                    // Build review prompt
                    let mut review_prompt =
                        format!("Review this PR #{}: \"{}\"\n\n", pr.number, pr.title);
                    if let Some(body) = &pr.body {
                        review_prompt.push_str(&format!("Description:\n{}\n\n", body));
                    }
                    review_prompt.push_str(
                        "Please review the code changes, check for:\n\
                        - Code quality and best practices\n\
                        - Potential bugs or edge cases\n\
                        - Performance concerns\n\
                        - Security issues\n\n\
                        Provide a summary of your findings.",
                    );
                    if !input.is_empty() {
                        review_prompt
                            .push_str(&format!("\n\nAdditional instructions:\n{}", input));
                    }

                    // Launch agent
                    let project_name = browser.current_project.clone();
                    match browser.dispatch_agent_for_worktree(&worktree_path, &review_prompt) {
                        Ok(session_name) => {
                            browser.status_message =
                                Some(format!("Started PR review session: {}", session_name));
                            browser.refresh_sessions(&project_name);

                            // Open embedded terminal to show the agent
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
                                        return_to_worktrees: false,
                                    };
                                }
                                Err(e) => {
                                    browser.status_message =
                                        Some(format!("Failed to open terminal: {}", e));
                                    browser.view = TuiView::PullRequestList;
                                }
                            }
                        }
                        Err(e) => {
                            browser.status_message =
                                Some(format!("Failed to start agent: {}", e));
                            browser.view = TuiView::PullRequestList;
                        }
                    }
                }
                Err(e) => {
                    browser.status_message = Some(format!("Failed to create worktree: {}", e));
                    browser.view = TuiView::PullRequestDetail(pr.clone());
                }
            }
        }
        KeyCode::Char(c) => {
            input.push(c);
        }
        KeyCode::Backspace => {
            input.pop();
        }
        _ => {}
    }
}
