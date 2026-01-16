//! Embedded terminal (tmux) event handling.

use crate::tui::IssueBrowser;
use crate::tui_types::TuiView;
use crossterm::event::{KeyCode, KeyModifiers};

pub fn handle_embedded_tmux_key(
    browser: &mut IssueBrowser,
    key: KeyCode,
    modifiers: KeyModifiers,
    available_sessions: &mut Vec<String>,
    current_index: &mut usize,
    return_to_worktrees: bool,
) {
    let has_modifier = modifiers.contains(KeyModifiers::CONTROL)
        || modifiers.contains(KeyModifiers::SUPER)
        || modifiers.contains(KeyModifiers::SHIFT);

    if key == KeyCode::Char('q') && modifiers.contains(KeyModifiers::CONTROL) {
        // Ctrl+Q to exit embedded terminal
        browser.embedded_term = None;
        browser.last_esc_press = None;
        if return_to_worktrees {
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
    } else if key == KeyCode::Esc {
        // Single ESC passes through to tmux
        if let Some(ref term) = browser.embedded_term {
            term.send_input(&[0x1b]); // ESC byte
        }
    } else if key == KeyCode::Left && has_modifier {
        // Ctrl/Shift/CMD+Left: switch to previous session
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
        // Ctrl/Shift/CMD+Right: switch to next session
        if !available_sessions.is_empty() && *current_index < available_sessions.len() - 1 {
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
