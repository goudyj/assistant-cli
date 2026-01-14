//! Search view event handling.

use crate::tui::IssueBrowser;
use crate::tui_types::TuiView;
use crossterm::event::KeyCode;

pub async fn handle_search_key(browser: &mut IssueBrowser, key: KeyCode, input: &mut String) {
    match key {
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
    }
}
