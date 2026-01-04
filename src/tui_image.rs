//! TUI image display functions.

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use image::ImageReader;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Color, Style},
    widgets::Paragraph,
    Terminal,
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, StatefulImage};
use std::io::{self, Cursor};

/// Display an image in the terminal using ratatui-image
pub async fn display_image(
    url: &str,
    github_token: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let mut request = client.get(url);
    if (url.contains("github.com") || url.contains("githubusercontent.com"))
        && let Some(token) = github_token
    {
        request = request.header("Authorization", format!("Bearer {}", token));
    }

    let response = request.send().await?;

    if !response.status().is_success() {
        return Err(format!("Failed to download: HTTP {}", response.status()).into());
    }

    let bytes = response.bytes().await?;

    let img = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()?
        .decode()?;

    // Temporarily exit raw mode for protocol detection
    disable_raw_mode()?;
    let picker = Picker::from_query_stdio()?;
    enable_raw_mode()?;

    let mut image_state = picker.new_resize_protocol(img);

    show_image_view(&mut image_state, url)?;

    Ok(())
}

/// Show image in a fullscreen ratatui view
fn show_image_view(
    image_state: &mut StatefulProtocol,
    url: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut stdout = io::stdout();

    let backend = CrosstermBackend::new(&mut stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    loop {
        terminal.draw(|f| {
            let area = f.area();

            let chunks =
                Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(area);

            let header = Paragraph::new(format!(
                "{}\n\nPress any key to return, 'b' to open in browser",
                url
            ))
            .style(Style::default().fg(Color::DarkGray));
            f.render_widget(header, chunks[0]);

            let image_widget = StatefulImage::default();
            f.render_stateful_widget(image_widget, chunks[1], image_state);
        })?;

        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            if key.code == KeyCode::Char('b') {
                let _ = open::that(url);
            }
            break;
        }
    }

    terminal.clear()?;

    Ok(())
}
