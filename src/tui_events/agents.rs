//! Agent-related views event handling.

use crate::tui::IssueBrowser;
use crate::tui_types::TuiView;
use crossterm::event::KeyCode;

pub fn handle_agent_logs_key(browser: &mut IssueBrowser, key: KeyCode, scroll: &mut u16) {
    match key {
        KeyCode::Esc | KeyCode::Char('q') => {
            browser.view = TuiView::List;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            *scroll = scroll.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            *scroll = scroll.saturating_add(1);
        }
        KeyCode::PageUp => {
            *scroll = scroll.saturating_sub(20);
        }
        KeyCode::PageDown => {
            *scroll = scroll.saturating_add(20);
        }
        _ => {}
    }
}

pub fn handle_agent_select_key(browser: &mut IssueBrowser, key: KeyCode, selected: &mut usize) {
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
    }
}
