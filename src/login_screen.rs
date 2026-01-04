//! Login screen TUI for GitHub OAuth authentication.

use crate::auth::{self, DeviceFlowAuth};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::Alignment,
    style::{Color, Modifier, Style},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use std::io;

/// Login screen state
enum LoginState {
    Initial,
    WaitingForAuth { auth: DeviceFlowAuth },
    Error(String),
}

/// Run the login screen TUI
/// Returns Ok(Some(token)) on successful login, Ok(None) if user cancelled
pub async fn run_login_screen(client_id: &str) -> io::Result<Option<String>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = LoginState::Initial;
    let mut result: Option<String> = None;
    let mut should_quit = false;

    while !should_quit {
        terminal.draw(|f| {
            draw_login_screen(f, &state);
        })?;

        match &state {
            LoginState::Initial => {
                if event::poll(std::time::Duration::from_millis(100))?
                    && let Event::Key(key) = event::read()?
                        && key.kind == KeyEventKind::Press {
                            match key.code {
                                KeyCode::Enter => {
                                    // Start device flow
                                    state = LoginState::Error("Starting authentication...".to_string());
                                }
                                KeyCode::Esc => {
                                    should_quit = true;
                                }
                                _ => {}
                            }
                        }

                // Check if we need to start the auth flow
                if matches!(state, LoginState::Error(ref msg) if msg == "Starting authentication...") {
                    // Temporarily exit TUI to start auth
                    disable_raw_mode()?;
                    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

                    match DeviceFlowAuth::start(client_id).await {
                        Ok(auth) => {
                            let _ = auth.open_browser();
                            execute!(io::stdout(), EnterAlternateScreen)?;
                            enable_raw_mode()?;
                            state = LoginState::WaitingForAuth { auth };
                        }
                        Err(e) => {
                            execute!(io::stdout(), EnterAlternateScreen)?;
                            enable_raw_mode()?;
                            state = LoginState::Error(format!("Auth failed: {}", e));
                        }
                    }
                }
            }
            LoginState::WaitingForAuth { auth } => {
                // Poll for events while waiting
                if event::poll(std::time::Duration::from_millis(100))?
                    && let Event::Key(key) = event::read()?
                        && key.kind == KeyEventKind::Press && key.code == KeyCode::Esc {
                            should_quit = true;
                            continue;
                        }

                // We need to own the auth to poll it, so we'll try once
                // This is a bit tricky - we'll use a timeout approach
                let poll_result = tokio::time::timeout(
                    std::time::Duration::from_millis(500),
                    check_auth_once(&auth.device_code, &auth.client_id),
                )
                .await;

                match poll_result {
                    Ok(Ok(Some(token))) => {
                        if let Err(e) = auth::store_token(&token) {
                            state = LoginState::Error(format!("Failed to store token: {}", e));
                        } else {
                            result = Some(token);
                            should_quit = true;
                        }
                    }
                    Ok(Ok(None)) => {
                        // Still pending, continue waiting
                    }
                    Ok(Err(e)) => {
                        state = LoginState::Error(format!("Auth failed: {}", e));
                    }
                    Err(_) => {
                        // Timeout, continue waiting
                    }
                }
            }
            LoginState::Error(_) => {
                if event::poll(std::time::Duration::from_millis(100))?
                    && let Event::Key(key) = event::read()?
                        && key.kind == KeyEventKind::Press {
                            match key.code {
                                KeyCode::Enter => {
                                    state = LoginState::Initial;
                                }
                                KeyCode::Esc => {
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

/// Check auth status once (helper for polling)
async fn check_auth_once(
    device_code: &str,
    client_id: &str,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();

    let response = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", client_id),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await?;

    let response_text = response.text().await?;

    #[derive(serde::Deserialize)]
    struct TokenResponse {
        access_token: Option<String>,
        error: Option<String>,
    }

    let data: TokenResponse = serde_json::from_str(&response_text)?;

    if let Some(token) = data.access_token {
        return Ok(Some(token));
    }

    if let Some(error) = data.error {
        match error.as_str() {
            "authorization_pending" | "slow_down" => Ok(None),
            _ => Err(error.into()),
        }
    } else {
        Ok(None)
    }
}

/// Draw the login screen
fn draw_login_screen(f: &mut Frame, state: &LoginState) {
    let area = f.area();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" GitHub Login ")
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let content = match state {
        LoginState::Initial => {
            vec![
                Line::from(""),
                Line::from(""),
                Line::styled(
                    "No GitHub connection detected.",
                    Style::default().fg(Color::Yellow),
                ),
                Line::from(""),
                Line::styled(
                    "Press Enter to login...",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Line::styled("Press Esc to quit.", Style::default().fg(Color::DarkGray)),
            ]
        }
        LoginState::WaitingForAuth { auth } => {
            vec![
                Line::from(""),
                Line::styled(
                    "Open this URL in your browser:",
                    Style::default().fg(Color::White),
                ),
                Line::from(""),
                Line::styled(
                    format!("  {}", auth.verification_uri),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
                Line::from(""),
                Line::styled("Enter code:", Style::default().fg(Color::White)),
                Line::from(""),
                Line::styled(
                    format!("  {}", auth.user_code),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Line::from(""),
                Line::styled(
                    "Waiting for authorization...",
                    Style::default().fg(Color::DarkGray),
                ),
                Line::from(""),
                Line::styled("Press Esc to cancel.", Style::default().fg(Color::DarkGray)),
            ]
        }
        LoginState::Error(msg) => {
            vec![
                Line::from(""),
                Line::styled(msg.clone(), Style::default().fg(Color::Red)),
                Line::from(""),
                Line::styled(
                    "Press Enter to retry, Esc to quit.",
                    Style::default().fg(Color::DarkGray),
                ),
            ]
        }
    };

    let text = Text::from(content);
    let paragraph = Paragraph::new(text).alignment(Alignment::Center);

    f.render_widget(paragraph, inner);
}
