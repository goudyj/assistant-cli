//! Input field widget for text entry.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

/// A text input field with placeholder support and cursor display.
///
/// # Example
/// ```
/// use assistant::widgets::InputField;
///
/// let input = InputField::new("search_term")
///     .placeholder("Type to search...")
///     .title(" Search ");
/// ```
#[derive(Debug, Clone)]
pub struct InputField<'a> {
    value: &'a str,
    placeholder: Option<&'a str>,
    title: Option<&'a str>,
    border_color: Color,
    show_cursor: bool,
    focused: bool,
    prefix: Option<&'a str>,
}

impl<'a> InputField<'a> {
    /// Create a new input field with the current value.
    pub fn new(value: &'a str) -> Self {
        Self {
            value,
            placeholder: None,
            title: None,
            border_color: Color::Yellow,
            show_cursor: true,
            focused: true,
            prefix: None,
        }
    }

    /// Set the placeholder text shown when the field is empty.
    pub fn placeholder(mut self, text: &'a str) -> Self {
        self.placeholder = Some(text);
        self
    }

    /// Set the title displayed in the border.
    pub fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// Set the border color.
    pub fn border_color(mut self, color: Color) -> Self {
        self.border_color = color;
        self
    }

    /// Set whether to show the cursor.
    pub fn show_cursor(mut self, show: bool) -> Self {
        self.show_cursor = show;
        self
    }

    /// Set whether the field is focused.
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Set a prefix to display before the input (e.g., "/" for commands, "@" for usernames).
    pub fn prefix(mut self, prefix: &'a str) -> Self {
        self.prefix = Some(prefix);
        self
    }
}

impl Widget for InputField<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let border_style = if self.focused {
            Style::default().fg(self.border_color)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style);

        if let Some(title) = self.title {
            block = block.title(title);
        }

        let (display_text, text_style) = if self.value.is_empty() {
            let text = self.placeholder.unwrap_or("");
            (text.to_string(), Style::default().fg(Color::DarkGray))
        } else {
            let prefix = self.prefix.unwrap_or("");
            let cursor = if self.show_cursor && self.focused {
                "_"
            } else {
                ""
            };
            (
                format!("{}{}{}", prefix, self.value, cursor),
                Style::default().fg(Color::White),
            )
        };

        let paragraph = Paragraph::new(display_text)
            .block(block)
            .style(text_style)
            .wrap(Wrap { trim: false });

        paragraph.render(area, buf);
    }
}

/// A simpler inline input (no borders) for use inside other widgets.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct InlineInput<'a> {
    value: &'a str,
    placeholder: Option<&'a str>,
    prefix: Option<&'a str>,
    show_cursor: bool,
}

#[allow(dead_code)]
impl<'a> InlineInput<'a> {
    pub fn new(value: &'a str) -> Self {
        Self {
            value,
            placeholder: None,
            prefix: None,
            show_cursor: true,
        }
    }

    pub fn placeholder(mut self, text: &'a str) -> Self {
        self.placeholder = Some(text);
        self
    }

    pub fn prefix(mut self, prefix: &'a str) -> Self {
        self.prefix = Some(prefix);
        self
    }

    pub fn show_cursor(mut self, show: bool) -> Self {
        self.show_cursor = show;
        self
    }
}

impl Widget for InlineInput<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let (text, style) = if self.value.is_empty() {
            (
                self.placeholder.unwrap_or("").to_string(),
                Style::default().fg(Color::DarkGray),
            )
        } else {
            let prefix = self.prefix.unwrap_or("");
            let cursor = if self.show_cursor { "_" } else { "" };
            (
                format!("{}{}{}", prefix, self.value, cursor),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
        };

        let paragraph = Paragraph::new(text).style(style);
        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_field_creation() {
        let input = InputField::new("test")
            .placeholder("Enter text")
            .title(" Input ");
        assert_eq!(input.value, "test");
        assert_eq!(input.placeholder, Some("Enter text"));
    }

    #[test]
    fn test_input_with_prefix() {
        let input = InputField::new("search").prefix("/");
        assert_eq!(input.prefix, Some("/"));
    }
}
