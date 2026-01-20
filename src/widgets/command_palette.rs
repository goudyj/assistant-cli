//! Command palette widget for slash commands.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Widget},
    Frame,
};

use crate::tui_types::CommandSuggestion;

/// A command suggestion item for the palette.
#[allow(dead_code)]
pub struct CommandItem<'a> {
    pub name: &'a str,
    pub description: &'a str,
    pub is_selected: bool,
}

#[allow(dead_code)]
impl<'a> CommandItem<'a> {
    pub fn new(name: &'a str, description: &'a str) -> Self {
        Self {
            name,
            description,
            is_selected: false,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.is_selected = selected;
        self
    }
}

/// Build a ListItem for a command suggestion.
pub fn build_command_item(cmd: &CommandSuggestion, is_selected: bool) -> ListItem<'_> {
    let prefix = if is_selected { "▸ " } else { "  " };
    let style = if is_selected {
        Style::default().bg(Color::DarkGray)
    } else {
        Style::default()
    };

    let line = Line::from(vec![
        Span::raw(prefix),
        Span::styled(
            format!("/{}", cmd.name),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(&cmd.description, Style::default().fg(Color::White)),
    ]);

    ListItem::new(line).style(style)
}

/// Configuration for the command palette.
pub struct CommandPaletteConfig<'a> {
    pub input: &'a str,
    pub suggestions: &'a [CommandSuggestion],
    pub selected: usize,
}

impl<'a> CommandPaletteConfig<'a> {
    pub fn new(input: &'a str, suggestions: &'a [CommandSuggestion], selected: usize) -> Self {
        Self {
            input,
            suggestions,
            selected,
        }
    }
}

/// Render the command palette panel (without background).
/// Returns the area used for the palette.
pub fn render_command_palette(f: &mut Frame, area: Rect, config: &CommandPaletteConfig) {
    let title = if config.suggestions.is_empty() {
        " Commands (no match) │ Esc cancel "
    } else {
        " Commands │ ↑↓ navigate │ Enter execute │ Esc cancel "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(1), // Input
        Constraint::Length(1), // Separator
        Constraint::Min(1),    // Suggestions
    ])
    .split(inner);

    // Input field with cursor
    let input_text = format!("/{}_", config.input);
    let input_para = Paragraph::new(input_text)
        .style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
    f.render_widget(input_para, chunks[0]);

    // Separator
    let sep = Paragraph::new("─".repeat(chunks[1].width as usize))
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(sep, chunks[1]);

    // Suggestions list
    let items: Vec<ListItem> = config
        .suggestions
        .iter()
        .enumerate()
        .map(|(i, cmd)| build_command_item(cmd, i == config.selected))
        .collect();

    f.render_widget(List::new(items), chunks[2]);
}

/// A simple separator line widget.
pub struct Separator {
    char: char,
    color: Color,
}

impl Separator {
    pub fn new() -> Self {
        Self {
            char: '─',
            color: Color::DarkGray,
        }
    }

    #[allow(dead_code)]
    pub fn char(mut self, c: char) -> Self {
        self.char = c;
        self
    }

    #[allow(dead_code)]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}

impl Default for Separator {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for Separator {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let line = self.char.to_string().repeat(area.width as usize);
        let paragraph = Paragraph::new(line).style(Style::default().fg(self.color));
        paragraph.render(area, buf);
    }
}
