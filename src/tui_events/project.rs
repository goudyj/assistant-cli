//! Project selection view event handling.

use crate::tui::IssueBrowser;
use crate::tui_types::TuiView;
use crossterm::event::KeyCode;

pub async fn handle_project_select_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    projects: &[String],
    selected: &mut usize,
) {
    match key {
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
                    browser.status_message = Some(format!("Switching to {}...", project_name));
                    browser.view = TuiView::List;
                    browser
                        .switch_project(&project_name, &project_config, &token)
                        .await;
                    browser.status_message = Some(format!("Switched to {}", project_name));
                } else {
                    browser.view = TuiView::List;
                    browser.status_message = Some("Project not found".to_string());
                }
            } else {
                browser.view = TuiView::List;
            }
        }
        _ => {}
    }
}
