//! Confirmation dialog widget.

use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// A confirmation dialog widget with customizable message and hint.
///
/// # Example
/// ```
/// use assistant::widgets::ConfirmDialog;
///
/// let dialog = ConfirmDialog::new("Close issue #42?")
///     .hint("y/n");
/// ```
#[derive(Debug, Clone)]
pub struct ConfirmDialog<'a> {
    message: &'a str,
    hint: Option<&'a str>,
    border_color: Color,
}

impl<'a> ConfirmDialog<'a> {
    /// Create a new confirmation dialog with the given message.
    pub fn new(message: &'a str) -> Self {
        Self {
            message,
            hint: Some("y/n"),
            border_color: Color::Yellow,
        }
    }

    /// Set the hint text shown after the message.
    pub fn hint(mut self, hint: &'a str) -> Self {
        self.hint = Some(hint);
        self
    }

    /// Remove the hint text.
    pub fn no_hint(mut self) -> Self {
        self.hint = None;
        self
    }

    /// Set the border color.
    pub fn border_color(mut self, color: Color) -> Self {
        self.border_color = color;
        self
    }
}

impl Widget for ConfirmDialog<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.border_color));

        let text = if let Some(hint) = self.hint {
            format!("{} ({})", self.message, hint)
        } else {
            self.message.to_string()
        };

        let paragraph = Paragraph::new(text)
            .block(block)
            .style(
                Style::default()
                    .fg(self.border_color)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center);

        paragraph.render(area, buf);
    }
}

/// A more detailed confirmation dialog with title, body, and action hints.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DetailedConfirmDialog<'a> {
    title: &'a str,
    body_lines: Vec<&'a str>,
    confirm_key: &'a str,
    confirm_label: &'a str,
    cancel_key: &'a str,
    cancel_label: &'a str,
}

#[allow(dead_code)]
impl<'a> DetailedConfirmDialog<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            body_lines: Vec::new(),
            confirm_key: "y",
            confirm_label: "Yes",
            cancel_key: "n",
            cancel_label: "No",
        }
    }

    pub fn body(mut self, lines: Vec<&'a str>) -> Self {
        self.body_lines = lines;
        self
    }

    pub fn confirm(mut self, key: &'a str, label: &'a str) -> Self {
        self.confirm_key = key;
        self.confirm_label = label;
        self
    }

    pub fn cancel(mut self, key: &'a str, label: &'a str) -> Self {
        self.cancel_key = key;
        self.cancel_label = label;
        self
    }
}

impl Widget for DetailedConfirmDialog<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        use ratatui::text::{Line, Span};

        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", self.title))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(area);
        block.render(area, buf);

        let mut lines = vec![Line::from("")];

        for body_line in &self.body_lines {
            lines.push(Line::from(*body_line));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(""));

        // Action hints
        lines.push(Line::from(vec![
            Span::styled(
                self.confirm_key,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(": {} │ ", self.confirm_label)),
            Span::styled(
                self.cancel_key,
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(": {}", self.cancel_label)),
        ]));

        let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
        paragraph.render(inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confirm_dialog_creation() {
        let dialog = ConfirmDialog::new("Delete?").hint("y/n");
        assert_eq!(dialog.message, "Delete?");
        assert_eq!(dialog.hint, Some("y/n"));
    }

    #[test]
    fn test_detailed_dialog() {
        let dialog = DetailedConfirmDialog::new("Confirm")
            .body(vec!["Line 1", "Line 2"])
            .confirm("y", "Yes, do it")
            .cancel("n", "Cancel");
        assert_eq!(dialog.title, "Confirm");
        assert_eq!(dialog.body_lines.len(), 2);
    }
}
