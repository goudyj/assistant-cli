//! Project selection screen TUI.

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::io;

/// Run project selection screen
/// Returns the selected project name or None if cancelled
pub async fn run_project_select(projects: Vec<String>) -> io::Result<Option<String>> {
    if projects.is_empty() {
        return Ok(None);
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut selected: usize = 0;
    let mut result: Option<String> = None;
    let mut should_quit = false;

    while !should_quit {
        terminal.draw(|f| {
            draw_project_select_screen(f, &projects, selected);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            should_quit = true;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if selected > 0 {
                                selected -= 1;
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if selected < projects.len() - 1 {
                                selected += 1;
                            }
                        }
                        KeyCode::Enter => {
                            result = Some(projects[selected].clone());
                            should_quit = true;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(result)
}

/// Draw the project selection screen
fn draw_project_select_screen(f: &mut Frame, projects: &[String], selected: usize) {
    let area = f.area();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Select Project ")
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let items: Vec<ListItem> = projects
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let style = if i == selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let prefix = if i == selected { "> " } else { "  " };
            ListItem::new(Line::from(format!("{}{}", prefix, name))).style(style)
        })
        .collect();

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    // Add instructions at the bottom
    let chunks = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(inner);

    f.render_widget(list, chunks[0]);

    let help = Paragraph::new("↑↓ navigate │ Enter select │ q quit")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(help, chunks[1]);
}
