//! Filter popup widget for issues and pull requests.

use std::collections::HashSet;

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

/// Focus area in filter popup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterFocus {
    Status,
    Author,
}

/// A filter option with a label and checked state.
pub struct FilterOption {
    pub label: String,
    pub checked: bool,
}

/// Configuration for the filter popup.
pub struct FilterPopupConfig<'a> {
    pub title: &'a str,
    pub status_options: Vec<FilterOption>,
    pub author_suggestions: &'a [String],
    pub author_filter: &'a HashSet<String>,
    pub author_input: &'a str,
    pub focus: FilterFocus,
    pub selected_status: usize,
    pub selected_author: usize,
    pub status_height: u16,
}

impl<'a> FilterPopupConfig<'a> {
    pub fn new(
        title: &'a str,
        author_suggestions: &'a [String],
        author_filter: &'a HashSet<String>,
        author_input: &'a str,
    ) -> Self {
        Self {
            title,
            status_options: Vec::new(),
            author_suggestions,
            author_filter,
            author_input,
            focus: FilterFocus::Status,
            selected_status: 0,
            selected_author: 0,
            status_height: 6,
        }
    }

    pub fn status_options(mut self, options: Vec<FilterOption>) -> Self {
        self.status_options = options;
        self
    }

    pub fn focus(mut self, focus: FilterFocus, selected_status: usize, selected_author: usize) -> Self {
        self.focus = focus;
        self.selected_status = selected_status;
        self.selected_author = selected_author;
        self
    }

    pub fn status_height(mut self, height: u16) -> Self {
        self.status_height = height;
        self
    }
}

/// Render a filter popup with the given configuration.
pub fn render_filter_popup(f: &mut Frame, config: &FilterPopupConfig) {
    let area = centered_rect(50, 60, f.area());

    // Render popup container
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", config.title))
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);

    // Layout sections
    let chunks = Layout::vertical([
        Constraint::Length(1),                    // Status header
        Constraint::Length(config.status_height), // Status options
        Constraint::Length(1),                    // Author header
        Constraint::Length(1),                    // Author input
        Constraint::Min(5),                       // Author list
        Constraint::Length(1),                    // Hint
    ])
    .split(inner);

    // Status section header
    let status_header_style = if config.focus == FilterFocus::Status {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    f.render_widget(
        Paragraph::new("Status:").style(status_header_style),
        chunks[0],
    );

    // Status options
    let status_items: Vec<ListItem> = config
        .status_options
        .iter()
        .enumerate()
        .map(|(i, opt)| {
            let checked = if opt.checked { "[x]" } else { "[ ]" };
            let style = if config.focus == FilterFocus::Status && i == config.selected_status {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };
            ListItem::new(format!("{} {}", checked, opt.label)).style(style)
        })
        .collect();
    f.render_widget(List::new(status_items), chunks[1]);

    // Author section header
    let author_header_style = if config.focus == FilterFocus::Author {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    f.render_widget(
        Paragraph::new("Author:").style(author_header_style),
        chunks[2],
    );

    // Author input field
    let input_text = if config.author_input.is_empty() && config.focus == FilterFocus::Author {
        "Type to search or add author...".to_string()
    } else if config.author_input.is_empty() {
        String::new()
    } else {
        format!("@{}_", config.author_input)
    };

    let input_style = if config.author_input.is_empty() && config.focus == FilterFocus::Author {
        Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)
    } else if config.focus == FilterFocus::Author {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    f.render_widget(Paragraph::new(input_text).style(input_style), chunks[3]);

    // Author list
    let author_items: Vec<ListItem> = if config.author_suggestions.is_empty() {
        vec![ListItem::new("No authors available").style(Style::default().fg(Color::DarkGray))]
    } else {
        config
            .author_suggestions
            .iter()
            .enumerate()
            .map(|(i, author)| {
                let checked = if config.author_filter.contains(author) {
                    "[x]"
                } else {
                    "[ ]"
                };
                let style = if config.focus == FilterFocus::Author && i == config.selected_author {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                };
                ListItem::new(format!("{} @{}", checked, author)).style(style)
            })
            .collect()
    };
    f.render_widget(List::new(author_items), chunks[4]);

    // Hint
    f.render_widget(
        Paragraph::new("Tab: switch │ Space: toggle │ Enter: add/apply │ Esc: cancel")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        chunks[5],
    );
}

/// Calculate centered rectangle for popups.
fn centered_rect(percent_x: u16, percent_y: u16, outer: Rect) -> Rect {
    let popup_width = (outer.width * percent_x / 100)
        .max(20)
        .min(outer.width.saturating_sub(4));
    let popup_height = (outer.height * percent_y / 100)
        .max(5)
        .min(outer.height.saturating_sub(4));
    let popup_x = (outer.width.saturating_sub(popup_width)) / 2;
    let popup_y = (outer.height.saturating_sub(popup_height)) / 2;
    Rect::new(popup_x, popup_y, popup_width, popup_height)
}
