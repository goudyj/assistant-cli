//! Issue creation views event handling.

use crate::issues::IssueContent;
use crate::llm;
use crate::tui::IssueBrowser;
use crate::tui_types::{CreateStage, TuiView};
use crossterm::event::{KeyCode, KeyModifiers};

pub async fn handle_create_issue_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    input: &mut String,
    stage: &mut CreateStage,
) {
    match key {
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
    }
}

pub async fn handle_preview_issue_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    issue: &mut IssueContent,
    messages: &mut Vec<llm::Message>,
    feedback_input: &mut String,
    scroll: &mut u16,
) {
    match key {
        KeyCode::Esc => {
            browser.view = TuiView::List;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            *scroll = scroll.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            *scroll = scroll.saturating_add(1);
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
    }
}

pub async fn handle_direct_issue_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    modifiers: KeyModifiers,
    title: &mut String,
    body: &mut String,
    editing_body: &mut bool,
) {
    match key {
        KeyCode::Esc => {
            browser.view = TuiView::List;
        }
        KeyCode::Tab => {
            *editing_body = !*editing_body;
        }
        KeyCode::Enter if modifiers.contains(KeyModifiers::SHIFT) => {
            submit_direct_issue(browser, title, body).await;
        }
        KeyCode::Char('s') | KeyCode::Char('j') if modifiers.contains(KeyModifiers::CONTROL) => {
            submit_direct_issue(browser, title, body).await;
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

async fn submit_direct_issue(browser: &mut IssueBrowser, title: &str, body: &str) {
    if title.is_empty() {
        browser.status_message = Some("Title cannot be empty".to_string());
    } else {
        let issue = IssueContent {
            type_: "task".to_string(),
            title: title.to_string(),
            body: body.to_string(),
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
