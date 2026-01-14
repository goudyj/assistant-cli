//! Help view event handling.

use crate::tui::IssueBrowser;
use crate::tui_types::TuiView;
use crossterm::event::KeyCode;

pub fn handle_help_key(browser: &mut IssueBrowser, key: KeyCode) {
    match key {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
            browser.view = TuiView::List;
        }
        _ => {}
    }
}
