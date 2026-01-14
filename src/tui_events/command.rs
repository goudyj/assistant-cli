//! Command palette event handling.

use crate::auth;
use crate::tui::IssueBrowser;
use crate::tui_events::common::filter_commands;
use crate::tui_types::{CommandSuggestion, TuiView};
use crossterm::event::KeyCode;

pub async fn handle_command_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    input: &mut String,
    suggestions: &mut Vec<CommandSuggestion>,
    selected: &mut usize,
) {
    match key {
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
                    "issues" => {
                        browser.view = TuiView::List;
                    }
                    "prs" => {
                        browser.load_pull_requests().await;
                        browser.view = TuiView::PullRequestList;
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
                            browser.status_message = Some("No projects configured.".to_string());
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
                            browser.status_message = Some(format!("Filter applied: /{}", cmd_name));
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
    }
}
