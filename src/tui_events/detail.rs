//! Issue detail views event handling.

use crate::github::IssueDetail;
use crate::tui::format_comment_with_llm;
use crate::tui::IssueBrowser;
use crate::tui_image::display_image;
use crate::tui_types::TuiView;
use crate::tui_utils::open_url;
use crossterm::event::KeyCode;

pub async fn handle_detail_key(browser: &mut IssueBrowser, key: KeyCode, issue: &IssueDetail) {
    match key {
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
    }
}

pub async fn handle_add_comment_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    issue: &IssueDetail,
    input: &mut String,
) {
    match key {
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
    }
}

pub async fn handle_confirm_close_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    issue: &IssueDetail,
) {
    match key {
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
    }
}

pub async fn handle_confirm_reopen_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    issue: &IssueDetail,
) {
    match key {
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
    }
}

pub async fn handle_assign_user_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    issue: &IssueDetail,
    input: &str,
    suggestions: &[String],
    selected: usize,
) {
    let number = issue.number;
    let current_assignees = issue.assignees.clone();
    let input_str = input.to_string();

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
            if let Some(user) = suggestions.get(selected) {
                let user_to_assign = user.clone();

                if current_assignees.contains(&user_to_assign) {
                    browser.status_message = Some(format!("{} is already assigned", user_to_assign));
                } else if browser
                    .github
                    .assign_issue(number, std::slice::from_ref(&user_to_assign))
                    .await
                    .is_ok()
                {
                    browser.status_message =
                        Some(format!("Assigned {} to #{}", user_to_assign, number));
                    if let Some(pos) = browser.issues.iter().position(|i| i.number == number) {
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
                    if let Some(pos) = browser.issues.iter().position(|i| i.number == number) {
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

pub async fn handle_confirm_dispatch_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    issue: &IssueDetail,
) {
    match key {
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
                    match crate::agents::dispatch_to_agent(
                        issue,
                        local_path,
                        project,
                        &browser.coding_agent,
                        browser.base_branch.as_deref(),
                        None,
                    )
                    .await
                    {
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
    }
}
