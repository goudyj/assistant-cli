//! Status bar widget for displaying messages at the bottom of the screen.

use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// A status bar widget that displays messages with automatic color coding.
///
/// # Example
/// ```
/// use assistant::widgets::StatusBar;
///
/// let status = StatusBar::new("Operation successful");
/// // Renders as a green bordered box with centered text
///
/// let error = StatusBar::new("Failed to connect");
/// // Automatically renders as red because message contains "Failed"
/// ```
#[derive(Debug, Clone)]
pub struct StatusBar<'a> {
    message: &'a str,
    style: Option<Style>,
}

impl<'a> StatusBar<'a> {
    /// Create a new status bar with the given message.
    /// Color is automatically determined based on message content.
    pub fn new(message: &'a str) -> Self {
        Self {
            message,
            style: None,
        }
    }

    /// Override the automatic color with a custom style.
    pub fn style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
    }

    /// Determine the appropriate color based on message content.
    fn detect_color(&self) -> Color {
        if self.message.contains("Failed")
            || self.message.contains("No ")
            || self.message.contains("error")
            || self.message.contains("Error")
        {
            Color::Red
        } else if self.message.contains("Loading") || self.message.contains("...") {
            Color::Yellow
        } else {
            Color::Green
        }
    }
}

impl Widget for StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let color = self
            .style
            .map(|s| s.fg.unwrap_or(Color::Green))
            .unwrap_or_else(|| self.detect_color());

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(color));

        let paragraph = Paragraph::new(self.message)
            .block(block)
            .style(Style::default().fg(color))
            .alignment(Alignment::Center);

        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_detection() {
        let status = StatusBar::new("Failed to load");
        assert_eq!(status.detect_color(), Color::Red);

        let status = StatusBar::new("No issues found");
        assert_eq!(status.detect_color(), Color::Red);
    }

    #[test]
    fn test_success_detection() {
        let status = StatusBar::new("Issues loaded successfully");
        assert_eq!(status.detect_color(), Color::Green);
    }

    #[test]
    fn test_loading_detection() {
        let status = StatusBar::new("Loading...");
        assert_eq!(status.detect_color(), Color::Yellow);
    }
}
