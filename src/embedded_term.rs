//! Embedded terminal for displaying PTY sessions in the TUI.

use std::sync::Arc;

use ratatui::style::Color;

use crate::agents::pty_manager::pty_manager;
use crate::agents::pty_session::PtySession;

/// A styled cell from the terminal.
#[derive(Clone)]
pub struct StyledCell {
    pub content: String,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub underline: bool,
    pub inverse: bool,
}

/// Embedded terminal that wraps a PTY session for display in the TUI.
pub struct EmbeddedTerminal {
    /// Reference to the PTY session.
    session: Arc<PtySession>,
    /// Current terminal height (for resize).
    rows: u16,
    /// Current terminal width (for resize).
    cols: u16,
    /// Session name for reference.
    pub session_name: String,
}

impl EmbeddedTerminal {
    /// Attach to an existing PTY session.
    ///
    /// This is the primary constructor - it attaches to a session managed by PtyManager.
    pub fn attach(session_name: &str, rows: u16, cols: u16) -> Result<Self, String> {
        let session = pty_manager()
            .get_session(session_name)
            .ok_or_else(|| format!("Session '{}' not found", session_name))?;

        // Resize the session to match our terminal size
        session.resize(rows, cols);

        Ok(Self {
            session,
            rows,
            cols,
            session_name: session_name.to_string(),
        })
    }

    /// Create a new embedded terminal (alias for attach for backward compatibility).
    pub fn new(session_name: &str, rows: u16, cols: u16) -> Result<Self, String> {
        Self::attach(session_name, rows, cols)
    }

    /// Get the current screen as styled cells for rendering.
    pub fn get_screen(&self) -> Vec<Vec<StyledCell>> {
        self.session.get_screen()
    }

    /// Check if the session is still running.
    pub fn is_running(&self) -> bool {
        self.session.is_running()
    }

    /// Send input bytes to the terminal.
    pub fn send_input(&self, data: &[u8]) {
        self.session.send_input(data);
    }

    /// Send a key to the terminal.
    pub fn send_key(&self, key: crossterm::event::KeyCode) {
        self.session.send_key(key);
    }

    /// Send a key with modifiers.
    pub fn send_key_with_modifiers(
        &self,
        key: crossterm::event::KeyCode,
        modifiers: crossterm::event::KeyModifiers,
    ) {
        self.session.send_key_with_modifiers(key, modifiers);
    }

    /// Resize the terminal.
    pub fn resize(&mut self, rows: u16, cols: u16) {
        if self.rows != rows || self.cols != cols {
            self.rows = rows;
            self.cols = cols;
            self.session.resize(rows, cols);
        }
    }

    /// Get the PTY session ID.
    pub fn session_id(&self) -> &str {
        &self.session.id
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_styled_cell_clone() {
        use super::StyledCell;
        use ratatui::style::Color;

        let cell = StyledCell {
            content: "x".to_string(),
            fg: Color::White,
            bg: Color::Black,
            bold: false,
            underline: false,
            inverse: false,
        };
        let cloned = cell.clone();
        assert_eq!(cloned.content, "x");
    }
}
