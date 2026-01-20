//! PTY session for running agents directly without tmux.

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use vt100::Parser;

use crate::embedded_term::StyledCell;

/// Size of the circular buffer for recent output (for idle detection).
const OUTPUT_BUFFER_SIZE: usize = 4096;

/// A PTY session running an agent directly.
pub struct PtySession {
    /// Unique session identifier (e.g., "project-issue-42")
    pub id: String,
    /// Terminal parser for screen state
    parser: Arc<Mutex<Parser>>,
    /// Channel to send input to the PTY
    input_tx: Sender<Vec<u8>>,
    /// Circular buffer for recent output (for idle detection)
    output_buffer: Arc<RwLock<VecDeque<u8>>>,
    /// Whether the session is still running
    running: Arc<Mutex<bool>>,
    /// Child process handle
    _child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
    /// Master PTY (kept for resize operations)
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
}

impl PtySession {
    /// Spawn a new PTY session running a command.
    ///
    /// # Arguments
    /// * `id` - Unique session identifier
    /// * `program` - The program to run (e.g., "claude" or "opencode")
    /// * `args` - Arguments to pass to the program
    /// * `working_dir` - Working directory for the command
    /// * `rows` - Terminal height in rows
    /// * `cols` - Terminal width in columns
    pub fn spawn(
        id: &str,
        program: &str,
        args: &[&str],
        working_dir: &Path,
        rows: u16,
        cols: u16,
    ) -> Result<Self, String> {
        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to open PTY: {}", e))?;

        // Build the command
        let mut cmd = CommandBuilder::new(program);
        cmd.args(args);
        cmd.cwd(working_dir);

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to spawn command: {}", e))?;

        // Create the terminal parser
        let parser = Arc::new(Mutex::new(Parser::new(rows, cols, 1000))); // 1000 lines scrollback
        let parser_clone = Arc::clone(&parser);

        // Create circular output buffer
        let output_buffer = Arc::new(RwLock::new(VecDeque::with_capacity(OUTPUT_BUFFER_SIZE)));
        let output_buffer_clone = Arc::clone(&output_buffer);

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
                        let data = &buf[..n];

                        // Update vt100 parser
                        if let Ok(mut parser) = parser_clone.lock() {
                            parser.process(data);
                        }

                        // Append to circular buffer
                        if let Ok(mut buffer) = output_buffer_clone.write() {
                            for byte in data {
                                if buffer.len() >= OUTPUT_BUFFER_SIZE {
                                    buffer.pop_front();
                                }
                                buffer.push_back(*byte);
                            }
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
            id: id.to_string(),
            parser,
            input_tx,
            output_buffer,
            running: running_clone,
            _child: Arc::new(Mutex::new(child)),
            master: Arc::new(Mutex::new(pair.master)),
        })
    }

    /// Check if the session is still running.
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
            KeyCode::BackTab => b"\x1b[Z".to_vec(),
            KeyCode::Up => b"\x1b[A".to_vec(),
            KeyCode::Down => b"\x1b[B".to_vec(),
            KeyCode::Right => b"\x1b[C".to_vec(),
            KeyCode::Left => b"\x1b[D".to_vec(),
            KeyCode::Home => b"\x1b[H".to_vec(),
            KeyCode::End => b"\x1b[F".to_vec(),
            KeyCode::PageUp => b"\x1b[5~".to_vec(),
            KeyCode::PageDown => b"\x1b[6~".to_vec(),
            KeyCode::Delete => b"\x1b[3~".to_vec(),
            KeyCode::Esc => vec![0x1b],
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
                let ctrl_code = (c.to_ascii_lowercase() as u8).wrapping_sub(b'a').wrapping_add(1);
                if ctrl_code <= 26 {
                    self.send_input(&[ctrl_code]);
                    return;
                }
            }
        }

        // Handle Alt+key combinations (ESC prefix)
        if modifiers.contains(KeyModifiers::ALT) {
            match key {
                KeyCode::Backspace => {
                    self.send_input(&[0x1b, 127]);
                    return;
                }
                KeyCode::Char(c) => {
                    self.send_input(&[0x1b, c as u8]);
                    return;
                }
                _ => {}
            }
        }

        // Handle Super (CMD on macOS) combinations
        if modifiers.contains(KeyModifiers::SUPER) {
            if let KeyCode::Backspace = key {
                self.send_input(&[21]); // Ctrl+U
                return;
            }
        }

        // Default: send the key without modifiers
        self.send_key(key);
    }

    /// Get the current screen contents as styled cells.
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

    /// Get recent output as a string (for idle detection).
    pub fn get_recent_output(&self) -> String {
        let buffer = self.output_buffer.read().unwrap();
        let bytes: Vec<u8> = buffer.iter().copied().collect();
        String::from_utf8_lossy(&bytes).to_string()
    }

    /// Resize the terminal.
    pub fn resize(&self, rows: u16, cols: u16) {
        // Resize the vt100 parser
        if let Ok(mut parser) = self.parser.lock() {
            parser.screen_mut().set_size(rows, cols);
        }

        // Resize the actual PTY
        if let Ok(master) = self.master.lock() {
            let _ = master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }
    }

    /// Kill the session.
    pub fn kill(&self) {
        *self.running.lock().unwrap() = false;
        // The child process will be killed when dropped
    }
}

/// Convert vt100 color to ratatui color.
fn convert_color(color: vt100::Color) -> ratatui::style::Color {
    use ratatui::style::Color;

    match color {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
        vt100::Color::Idx(n) => match n {
            0 => Color::Black,
            1 => Color::Red,
            2 => Color::Green,
            3 => Color::Yellow,
            4 => Color::Blue,
            5 => Color::Magenta,
            6 => Color::Cyan,
            7 => Color::Gray,
            8 => Color::DarkGray,
            9 => Color::LightRed,
            10 => Color::LightGreen,
            11 => Color::LightYellow,
            12 => Color::LightBlue,
            13 => Color::LightMagenta,
            14 => Color::LightCyan,
            15 => Color::White,
            _ => Color::Indexed(n),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_buffer_circular() {
        let buffer: VecDeque<u8> = VecDeque::with_capacity(10);
        assert_eq!(buffer.capacity(), 10);
    }
}
