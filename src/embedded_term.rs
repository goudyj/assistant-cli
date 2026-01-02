//! Embedded terminal for running tmux sessions within the TUI.

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use vt100::Parser;

/// Embedded terminal that wraps a tmux session.
pub struct EmbeddedTerminal {
    /// Terminal parser that maintains the screen buffer
    parser: Arc<Mutex<Parser>>,
    /// Channel to send input to the PTY
    input_tx: Sender<Vec<u8>>,
    /// Whether the terminal is still running
    running: Arc<Mutex<bool>>,
    /// Current tmux session name
    pub session_name: String,
}

impl EmbeddedTerminal {
    /// Create a new embedded terminal attached to a tmux session.
    pub fn new(session_name: &str, rows: u16, cols: u16) -> Result<Self, String> {
        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to open PTY: {}", e))?;

        // Build the tmux attach command
        let mut cmd = CommandBuilder::new("tmux");
        cmd.args(["attach", "-t", session_name]);

        let _child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to spawn tmux: {}", e))?;

        // Create the terminal parser
        let parser = Arc::new(Mutex::new(Parser::new(rows, cols, 0)));
        let parser_clone = Arc::clone(&parser);

        // Create input channel
        let (input_tx, input_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::channel();

        // Running flag
        let running = Arc::new(Mutex::new(true));
        let running_clone = Arc::clone(&running);

        // Get reader and writer from PTY
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("Failed to clone reader: {}", e))?;

        let mut writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to take writer: {}", e))?;

        // Spawn reader thread
        let running_reader = Arc::clone(&running);
        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        // EOF - PTY closed
                        *running_reader.lock().unwrap() = false;
                        break;
                    }
                    Ok(n) => {
                        if let Ok(mut parser) = parser_clone.lock() {
                            parser.process(&buf[..n]);
                        }
                    }
                    Err(_) => {
                        *running_reader.lock().unwrap() = false;
                        break;
                    }
                }
            }
        });

        // Spawn writer thread
        thread::spawn(move || {
            while let Ok(data) = input_rx.recv() {
                if writer.write_all(&data).is_err() {
                    break;
                }
                let _ = writer.flush();
            }
        });

        Ok(Self {
            parser,
            input_tx,
            running: running_clone,
            session_name: session_name.to_string(),
        })
    }

    /// Check if the terminal is still running.
    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }

    /// Send input bytes to the terminal.
    pub fn send_input(&self, data: &[u8]) {
        let _ = self.input_tx.send(data.to_vec());
    }

    /// Send a key to the terminal.
    pub fn send_key(&self, key: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;

        let bytes: Vec<u8> = match key {
            KeyCode::Char(c) => vec![c as u8],
            KeyCode::Enter => vec![b'\r'],
            KeyCode::Backspace => vec![127],
            KeyCode::Tab => vec![b'\t'],
            KeyCode::Up => b"\x1b[A".to_vec(),
            KeyCode::Down => b"\x1b[B".to_vec(),
            KeyCode::Right => b"\x1b[C".to_vec(),
            KeyCode::Left => b"\x1b[D".to_vec(),
            KeyCode::Home => b"\x1b[H".to_vec(),
            KeyCode::End => b"\x1b[F".to_vec(),
            KeyCode::PageUp => b"\x1b[5~".to_vec(),
            KeyCode::PageDown => b"\x1b[6~".to_vec(),
            KeyCode::Delete => b"\x1b[3~".to_vec(),
            KeyCode::F(n) => match n {
                1 => b"\x1bOP".to_vec(),
                2 => b"\x1bOQ".to_vec(),
                3 => b"\x1bOR".to_vec(),
                4 => b"\x1bOS".to_vec(),
                5 => b"\x1b[15~".to_vec(),
                6 => b"\x1b[17~".to_vec(),
                7 => b"\x1b[18~".to_vec(),
                8 => b"\x1b[19~".to_vec(),
                9 => b"\x1b[20~".to_vec(),
                10 => b"\x1b[21~".to_vec(),
                11 => b"\x1b[23~".to_vec(),
                12 => b"\x1b[24~".to_vec(),
                _ => vec![],
            },
            _ => vec![],
        };

        if !bytes.is_empty() {
            self.send_input(&bytes);
        }
    }

    /// Send a key with modifiers.
    pub fn send_key_with_modifiers(
        &self,
        key: crossterm::event::KeyCode,
        modifiers: crossterm::event::KeyModifiers,
    ) {
        use crossterm::event::{KeyCode, KeyModifiers};

        // Handle Ctrl+key combinations
        if modifiers.contains(KeyModifiers::CONTROL) {
            if let KeyCode::Char(c) = key {
                // Ctrl+A = 1, Ctrl+B = 2, etc.
                let ctrl_code = (c.to_ascii_lowercase() as u8).wrapping_sub(b'a').wrapping_add(1);
                if ctrl_code <= 26 {
                    self.send_input(&[ctrl_code]);
                    return;
                }
            }
        }

        // Default: send the key without modifiers
        self.send_key(key);
    }

    /// Get the current screen contents as lines of styled spans.
    pub fn get_screen(&self) -> Vec<Vec<StyledCell>> {
        let parser = self.parser.lock().unwrap();
        let screen = parser.screen();
        let mut lines = Vec::new();

        for row in 0..screen.size().0 {
            let mut line = Vec::new();
            for col in 0..screen.size().1 {
                let cell = screen.cell(row, col).unwrap();
                line.push(StyledCell {
                    content: cell.contents().to_string(),
                    fg: convert_color(cell.fgcolor()),
                    bg: convert_color(cell.bgcolor()),
                    bold: cell.bold(),
                    underline: cell.underline(),
                    inverse: cell.inverse(),
                });
            }
            lines.push(line);
        }

        lines
    }

    /// Resize the terminal.
    pub fn resize(&self, rows: u16, cols: u16) {
        if let Ok(mut parser) = self.parser.lock() {
            parser.screen_mut().set_size(rows, cols);
        }
    }
}

/// A styled cell from the terminal.
#[derive(Clone)]
pub struct StyledCell {
    pub content: String,
    pub fg: ratatui::style::Color,
    pub bg: ratatui::style::Color,
    pub bold: bool,
    pub underline: bool,
    pub inverse: bool,
}

/// Convert vt100 color to ratatui color.
fn convert_color(color: vt100::Color) -> ratatui::style::Color {
    use ratatui::style::Color;

    match color {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(0) => Color::Black,
        vt100::Color::Idx(1) => Color::Red,
        vt100::Color::Idx(2) => Color::Green,
        vt100::Color::Idx(3) => Color::Yellow,
        vt100::Color::Idx(4) => Color::Blue,
        vt100::Color::Idx(5) => Color::Magenta,
        vt100::Color::Idx(6) => Color::Cyan,
        vt100::Color::Idx(7) => Color::Gray,
        vt100::Color::Idx(8) => Color::DarkGray,
        vt100::Color::Idx(9) => Color::LightRed,
        vt100::Color::Idx(10) => Color::LightGreen,
        vt100::Color::Idx(11) => Color::LightYellow,
        vt100::Color::Idx(12) => Color::LightBlue,
        vt100::Color::Idx(13) => Color::LightMagenta,
        vt100::Color::Idx(14) => Color::LightCyan,
        vt100::Color::Idx(15) => Color::White,
        vt100::Color::Idx(n) => Color::Indexed(n),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_conversion() {
        use ratatui::style::Color;

        assert_eq!(convert_color(vt100::Color::Default), Color::Reset);
        assert_eq!(convert_color(vt100::Color::Idx(1)), Color::Red);
        assert_eq!(convert_color(vt100::Color::Rgb(255, 0, 0)), Color::Rgb(255, 0, 0));
    }
}
