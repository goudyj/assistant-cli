//! Clipboard utilities for the TUI.

/// Try to get clipboard content (platform-specific)
#[allow(unreachable_code)]
pub fn get_clipboard_content() -> Result<String, Box<dyn std::error::Error>> {
    // Try using pbpaste on macOS
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("pbpaste")
            .output()?;
        if output.status.success() {
            return Ok(String::from_utf8(output.stdout)?);
        }
    }

    // Try using xclip on Linux
    #[cfg(target_os = "linux")]
    {
        let output = std::process::Command::new("xclip")
            .args(["-selection", "clipboard", "-o"])
            .output();
        if let Ok(out) = output {
            if out.status.success() {
                return Ok(String::from_utf8(out.stdout)?);
            }
        }
        // Fallback to xsel
        let output = std::process::Command::new("xsel")
            .args(["--clipboard", "--output"])
            .output();
        if let Ok(out) = output {
            if out.status.success() {
                return Ok(String::from_utf8(out.stdout)?);
            }
        }
    }

    Err("Clipboard not available".into())
}

#[cfg(test)]
mod tests {
    // Platform-specific tests would be hard to write reliably
}
