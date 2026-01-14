//! Filter views event handling.

use crate::tui::IssueBrowser;
use crate::tui_types::{IssueFilterFocus, IssueStatus, PrFilterFocus, PrStatus, TuiView};
use crossterm::event::KeyCode;
use std::collections::HashSet;

fn update_suggestions(input: &str, available: &[String]) -> Vec<String> {
    if input.is_empty() {
        available.to_vec()
    } else {
        let input_lower = input.to_lowercase();
        available
            .iter()
            .filter(|a| a.to_lowercase().contains(&input_lower))
            .cloned()
            .collect()
    }
}

pub fn handle_pr_filters_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    status_filter: &mut HashSet<PrStatus>,
    author_filter: &mut HashSet<String>,
    available_authors: &[String],
    focus: &mut PrFilterFocus,
    selected_status: &mut usize,
    selected_author: &mut usize,
    author_input: &mut String,
    author_suggestions: &mut Vec<String>,
) {
    let build_view = |sf: &HashSet<PrStatus>,
                      af: &HashSet<String>,
                      aa: &[String],
                      f: PrFilterFocus,
                      ss: usize,
                      sa: usize,
                      ai: String,
                      asg: Vec<String>| {
        TuiView::PrFilters {
            status_filter: sf.clone(),
            author_filter: af.clone(),
            available_authors: aa.to_vec(),
            focus: f,
            selected_status: ss,
            selected_author: sa,
            author_input: ai,
            author_suggestions: asg,
        }
    };

    match key {
        KeyCode::Esc => {
            browser.view = TuiView::PullRequestList;
        }
        KeyCode::Enter => {
            if *focus == PrFilterFocus::Author && !author_input.is_empty() {
                let author_to_add = if !author_suggestions.is_empty()
                    && *selected_author < author_suggestions.len()
                {
                    author_suggestions[*selected_author].clone()
                } else {
                    author_input.clone()
                };
                author_filter.insert(author_to_add);
                author_input.clear();
                *author_suggestions = available_authors.to_vec();
                *selected_author = 0;
                browser.view = build_view(
                    status_filter,
                    author_filter,
                    available_authors,
                    *focus,
                    *selected_status,
                    *selected_author,
                    author_input.clone(),
                    author_suggestions.clone(),
                );
            } else {
                browser.pr_status_filter = status_filter.clone();
                browser.pr_author_filter = author_filter.clone();
                browser.apply_pr_filters();
                browser.view = TuiView::PullRequestList;
            }
        }
        KeyCode::Tab => {
            let new_focus = match focus {
                PrFilterFocus::Status => PrFilterFocus::Author,
                PrFilterFocus::Author => PrFilterFocus::Status,
            };
            browser.view = build_view(
                status_filter,
                author_filter,
                available_authors,
                new_focus,
                *selected_status,
                *selected_author,
                author_input.clone(),
                author_suggestions.clone(),
            );
        }
        KeyCode::Down => {
            match focus {
                PrFilterFocus::Status => {
                    let max = PrStatus::all().len().saturating_sub(1);
                    *selected_status = (*selected_status + 1).min(max);
                }
                PrFilterFocus::Author => {
                    let max = author_suggestions.len().saturating_sub(1);
                    *selected_author = (*selected_author + 1).min(max);
                }
            }
            browser.view = build_view(
                status_filter,
                author_filter,
                available_authors,
                *focus,
                *selected_status,
                *selected_author,
                author_input.clone(),
                author_suggestions.clone(),
            );
        }
        KeyCode::Up => {
            match focus {
                PrFilterFocus::Status => {
                    *selected_status = selected_status.saturating_sub(1);
                }
                PrFilterFocus::Author => {
                    *selected_author = selected_author.saturating_sub(1);
                }
            }
            browser.view = build_view(
                status_filter,
                author_filter,
                available_authors,
                *focus,
                *selected_status,
                *selected_author,
                author_input.clone(),
                author_suggestions.clone(),
            );
        }
        KeyCode::Char(' ') if *focus == PrFilterFocus::Status => {
            let statuses = PrStatus::all();
            if let Some(status) = statuses.get(*selected_status) {
                if status_filter.contains(status) {
                    status_filter.remove(status);
                } else {
                    status_filter.insert(*status);
                }
            }
            browser.view = build_view(
                status_filter,
                author_filter,
                available_authors,
                *focus,
                *selected_status,
                *selected_author,
                author_input.clone(),
                author_suggestions.clone(),
            );
        }
        KeyCode::Char(' ') if *focus == PrFilterFocus::Author && author_input.is_empty() => {
            if let Some(author) = author_suggestions.get(*selected_author) {
                if author_filter.contains(author) {
                    author_filter.remove(author);
                } else {
                    author_filter.insert(author.clone());
                }
            }
            browser.view = build_view(
                status_filter,
                author_filter,
                available_authors,
                *focus,
                *selected_status,
                *selected_author,
                author_input.clone(),
                author_suggestions.clone(),
            );
        }
        KeyCode::Char(c) if *focus == PrFilterFocus::Author => {
            author_input.push(c);
            *author_suggestions = update_suggestions(author_input, available_authors);
            *selected_author = 0;
            browser.view = build_view(
                status_filter,
                author_filter,
                available_authors,
                *focus,
                *selected_status,
                *selected_author,
                author_input.clone(),
                author_suggestions.clone(),
            );
        }
        KeyCode::Backspace if *focus == PrFilterFocus::Author => {
            author_input.pop();
            *author_suggestions = update_suggestions(author_input, available_authors);
            *selected_author = 0;
            browser.view = build_view(
                status_filter,
                author_filter,
                available_authors,
                *focus,
                *selected_status,
                *selected_author,
                author_input.clone(),
                author_suggestions.clone(),
            );
        }
        _ => {}
    }
}

pub fn handle_issue_filters_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    status_filter: &mut HashSet<IssueStatus>,
    author_filter: &mut HashSet<String>,
    available_authors: &[String],
    focus: &mut IssueFilterFocus,
    selected_status: &mut usize,
    selected_author: &mut usize,
    author_input: &mut String,
    author_suggestions: &mut Vec<String>,
) {
    let build_view = |sf: &HashSet<IssueStatus>,
                      af: &HashSet<String>,
                      aa: &[String],
                      f: IssueFilterFocus,
                      ss: usize,
                      sa: usize,
                      ai: String,
                      asg: Vec<String>| {
        TuiView::IssueFilters {
            status_filter: sf.clone(),
            author_filter: af.clone(),
            available_authors: aa.to_vec(),
            focus: f,
            selected_status: ss,
            selected_author: sa,
            author_input: ai,
            author_suggestions: asg,
        }
    };

    match key {
        KeyCode::Esc => {
            browser.view = TuiView::List;
        }
        KeyCode::Enter => {
            if *focus == IssueFilterFocus::Author && !author_input.is_empty() {
                let author_to_add = if !author_suggestions.is_empty()
                    && *selected_author < author_suggestions.len()
                {
                    author_suggestions[*selected_author].clone()
                } else {
                    author_input.clone()
                };
                author_filter.insert(author_to_add);
                author_input.clear();
                *author_suggestions = available_authors.to_vec();
                *selected_author = 0;
                browser.view = build_view(
                    status_filter,
                    author_filter,
                    available_authors,
                    *focus,
                    *selected_status,
                    *selected_author,
                    author_input.clone(),
                    author_suggestions.clone(),
                );
            } else {
                browser.issue_status_filter = status_filter.clone();
                browser.issue_author_filter = author_filter.clone();
                browser.apply_issue_filters();
                browser.view = TuiView::List;
            }
        }
        KeyCode::Tab => {
            let new_focus = match focus {
                IssueFilterFocus::Status => IssueFilterFocus::Author,
                IssueFilterFocus::Author => IssueFilterFocus::Status,
            };
            browser.view = build_view(
                status_filter,
                author_filter,
                available_authors,
                new_focus,
                *selected_status,
                *selected_author,
                author_input.clone(),
                author_suggestions.clone(),
            );
        }
        KeyCode::Down => {
            match focus {
                IssueFilterFocus::Status => {
                    let max = IssueStatus::all().len().saturating_sub(1);
                    *selected_status = (*selected_status + 1).min(max);
                }
                IssueFilterFocus::Author => {
                    let max = author_suggestions.len().saturating_sub(1);
                    *selected_author = (*selected_author + 1).min(max);
                }
            }
            browser.view = build_view(
                status_filter,
                author_filter,
                available_authors,
                *focus,
                *selected_status,
                *selected_author,
                author_input.clone(),
                author_suggestions.clone(),
            );
        }
        KeyCode::Up => {
            match focus {
                IssueFilterFocus::Status => {
                    *selected_status = selected_status.saturating_sub(1);
                }
                IssueFilterFocus::Author => {
                    *selected_author = selected_author.saturating_sub(1);
                }
            }
            browser.view = build_view(
                status_filter,
                author_filter,
                available_authors,
                *focus,
                *selected_status,
                *selected_author,
                author_input.clone(),
                author_suggestions.clone(),
            );
        }
        KeyCode::Char(' ') if *focus == IssueFilterFocus::Status => {
            let statuses = IssueStatus::all();
            if let Some(status) = statuses.get(*selected_status) {
                if status_filter.contains(status) {
                    status_filter.remove(status);
                } else {
                    status_filter.insert(*status);
                }
            }
            browser.view = build_view(
                status_filter,
                author_filter,
                available_authors,
                *focus,
                *selected_status,
                *selected_author,
                author_input.clone(),
                author_suggestions.clone(),
            );
        }
        KeyCode::Char(' ') if *focus == IssueFilterFocus::Author && author_input.is_empty() => {
            if let Some(author) = author_suggestions.get(*selected_author) {
                if author_filter.contains(author) {
                    author_filter.remove(author);
                } else {
                    author_filter.insert(author.clone());
                }
            }
            browser.view = build_view(
                status_filter,
                author_filter,
                available_authors,
                *focus,
                *selected_status,
                *selected_author,
                author_input.clone(),
                author_suggestions.clone(),
            );
        }
        KeyCode::Char(c) if *focus == IssueFilterFocus::Author => {
            author_input.push(c);
            *author_suggestions = update_suggestions(author_input, available_authors);
            *selected_author = 0;
            browser.view = build_view(
                status_filter,
                author_filter,
                available_authors,
                *focus,
                *selected_status,
                *selected_author,
                author_input.clone(),
                author_suggestions.clone(),
            );
        }
        KeyCode::Backspace if *focus == IssueFilterFocus::Author => {
            author_input.pop();
            *author_suggestions = update_suggestions(author_input, available_authors);
            *selected_author = 0;
            browser.view = build_view(
                status_filter,
                author_filter,
                available_authors,
                *focus,
                *selected_status,
                *selected_author,
                author_input.clone(),
                author_suggestions.clone(),
            );
        }
        _ => {}
    }
}
