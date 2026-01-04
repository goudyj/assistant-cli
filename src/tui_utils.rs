//! TUI utility functions.

use std::io;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

/// Format a date string to just the date part (YYYY-MM-DD)
pub fn format_date(date_str: &str) -> String {
    if date_str.len() >= 10 {
        date_str[..10].to_string()
    } else {
        date_str.to_string()
    }
}

/// Open a URL in the default browser
pub fn open_url(url: &str) {
    let _ = open::that(url);
}

/// Attach to a tmux session, temporarily exiting the TUI
#[allow(dead_code)]
pub fn attach_to_tmux_session(session_name: &str) -> io::Result<()> {
    // Exit raw mode and alternate screen
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;

    // Run tmux attach interactively
    let status = std::process::Command::new("tmux")
        .args(["attach", "-t", session_name])
        .status()?;

    // Re-enter alternate screen and raw mode
    execute!(io::stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;

    if !status.success() {
        return Err(io::Error::other(
            "tmux attach failed",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_date_full_iso() {
        assert_eq!(format_date("2024-01-15T10:30:00Z"), "2024-01-15");
    }

    #[test]
    fn format_date_short_string() {
        assert_eq!(format_date("2024-01"), "2024-01");
    }

    #[test]
    fn format_date_exactly_ten_chars() {
        assert_eq!(format_date("2024-01-15"), "2024-01-15");
    }
}
