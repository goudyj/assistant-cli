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
    SavingToken,
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
    let mut poll_status = String::from("Waiting for authorization...");
    let mut last_poll = std::time::Instant::now() - std::time::Duration::from_secs(10);
    let mut poll_interval = std::time::Duration::from_secs(5);

    while !should_quit {
        terminal.draw(|f| {
            draw_login_screen(f, &state, &poll_status);
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

                // Only poll GitHub if enough time has passed
                if last_poll.elapsed() < poll_interval {
                    continue;
                }
                last_poll = std::time::Instant::now();

                let poll_result = tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    check_auth_once_debug(&auth.device_code, &auth.client_id),
                )
                .await;

                match poll_result {
                    Ok(Ok((Some(token), _))) => {
                        result = Some(token);
                        state = LoginState::SavingToken;
                    }
                    Ok(Ok((None, msg))) => {
                        if msg.contains("slow_down") {
                            poll_interval = std::time::Duration::from_secs(10);
                        }
                        poll_status = msg;
                    }
                    Ok(Err(e)) => {
                        state = LoginState::Error(format!("Auth failed: {}", e));
                    }
                    Err(_) => {
                        poll_status = "Timeout, retrying...".to_string();
                    }
                }
            }
            LoginState::SavingToken => {
                // Save the token to config file
                if let Some(ref token) = result {
                    if let Err(e) = auth::store_token(token) {
                        state = LoginState::Error(format!("Failed to save token: {}", e));
                        result = None;
                    } else {
                        should_quit = true;
                    }
                } else {
                    state = LoginState::Error("Token was lost".to_string());
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

/// Check auth status once (helper for polling) - returns (token, status_message)
async fn check_auth_once_debug(
    device_code: &str,
    client_id: &str,
) -> Result<(Option<String>, String), Box<dyn std::error::Error + Send + Sync>> {
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
        error_description: Option<String>,
    }

    let data: TokenResponse = serde_json::from_str(&response_text)
        .map_err(|e| format!("JSON parse error: {} - Response: {}", e, response_text))?;

    if let Some(token) = data.access_token {
        return Ok((Some(token), "Token received!".to_string()));
    }

    if let Some(error) = data.error {
        let desc = data.error_description.unwrap_or_default();
        match error.as_str() {
            "authorization_pending" => Ok((None, format!("Waiting... ({})", error))),
            "slow_down" => Ok((None, format!("Slowing down... ({})", error))),
            _ => Err(format!("{}: {}", error, desc).into()),
        }
    } else {
        Ok((None, format!("Unknown response: {}", response_text)))
    }
}

/// Draw the login screen
fn draw_login_screen(f: &mut Frame, state: &LoginState, poll_status: &str) {
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
                    poll_status.to_string(),
                    Style::default().fg(Color::DarkGray),
                ),
                Line::from(""),
                Line::styled("Press Esc to cancel.", Style::default().fg(Color::DarkGray)),
            ]
        }
        LoginState::SavingToken => {
            vec![
                Line::from(""),
                Line::styled(
                    "Authorization received!",
                    Style::default().fg(Color::Green),
                ),
                Line::from(""),
                Line::styled(
                    "Saving token...",
                    Style::default().fg(Color::Yellow),
                ),
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
