//! Popup widget for centered modal dialogs.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Widget},
};

/// A centered popup container that clears the background and draws a bordered box.
///
/// # Example
/// ```
/// use assistant::widgets::Popup;
/// use ratatui::style::Color;
///
/// let popup = Popup::new(" Search ")
///     .percent(50, 30)
///     .border_color(Color::Yellow);
/// ```
#[derive(Debug, Clone)]
pub struct Popup<'a> {
    title: &'a str,
    percent_x: u16,
    percent_y: u16,
    border_color: Color,
    min_width: u16,
    min_height: u16,
}

impl<'a> Popup<'a> {
    /// Create a new popup with the given title.
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            percent_x: 50,
            percent_y: 30,
            border_color: Color::Cyan,
            min_width: 20,
            min_height: 5,
        }
    }

    /// Set the popup size as a percentage of the parent area.
    pub fn percent(mut self, x: u16, y: u16) -> Self {
        self.percent_x = x;
        self.percent_y = y;
        self
    }

    /// Set the border color.
    pub fn border_color(mut self, color: Color) -> Self {
        self.border_color = color;
        self
    }

    /// Set minimum dimensions.
    pub fn min_size(mut self, width: u16, height: u16) -> Self {
        self.min_width = width;
        self.min_height = height;
        self
    }

    /// Calculate the centered popup area within the given outer area.
    pub fn area(&self, outer: Rect) -> Rect {
        let popup_width = (outer.width * self.percent_x / 100)
            .max(self.min_width)
            .min(outer.width.saturating_sub(4));
        let popup_height = (outer.height * self.percent_y / 100)
            .max(self.min_height)
            .min(outer.height.saturating_sub(4));
        let popup_x = (outer.width.saturating_sub(popup_width)) / 2;
        let popup_y = (outer.height.saturating_sub(popup_height)) / 2;

        Rect::new(popup_x, popup_y, popup_width, popup_height)
    }

    /// Get the inner area (inside the borders) for content rendering.
    pub fn inner(&self, outer: Rect) -> Rect {
        let popup_area = self.area(outer);
        let block = Block::default().borders(Borders::ALL);
        block.inner(popup_area)
    }
}

impl Widget for Popup<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let popup_area = self.area(area);

        // Clear the background
        Clear.render(popup_area, buf);

        // Draw the bordered box
        let block = Block::default()
            .borders(Borders::ALL)
            .title(self.title)
            .border_style(Style::default().fg(self.border_color))
            .style(Style::default().bg(Color::Black));

        block.render(popup_area, buf);
    }
}

/// Helper to render a popup and return its inner area for content.
#[allow(dead_code)]
pub fn render_popup(
    f: &mut ratatui::Frame,
    title: &str,
    percent_x: u16,
    percent_y: u16,
) -> Rect {
    let popup = Popup::new(title).percent(percent_x, percent_y);
    let area = f.area();
    let inner = popup.inner(area);
    f.render_widget(popup, area);
    inner
}

/// Render a popup with custom border color and return its inner area.
#[allow(dead_code)]
pub fn render_popup_colored(
    f: &mut ratatui::Frame,
    title: &str,
    percent_x: u16,
    percent_y: u16,
    border_color: Color,
) -> Rect {
    let popup = Popup::new(title)
        .percent(percent_x, percent_y)
        .border_color(border_color);
    let area = f.area();
    let inner = popup.inner(area);
    f.render_widget(popup, area);
    inner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_popup_area_calculation() {
        let popup = Popup::new("Test").percent(50, 50);
        let outer = Rect::new(0, 0, 100, 50);
        let area = popup.area(outer);

        assert_eq!(area.width, 50);
        assert_eq!(area.height, 25);
        assert_eq!(area.x, 25);
        assert_eq!(area.y, 12);
    }

    #[test]
    fn test_popup_min_size() {
        let popup = Popup::new("Test").percent(10, 10).min_size(30, 10);
        let outer = Rect::new(0, 0, 100, 50);
        let area = popup.area(outer);

        // Should use min_size since 10% is smaller
        assert_eq!(area.width, 30);
        assert_eq!(area.height, 10);
    }
}
