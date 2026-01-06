//! Opencode integration for dispatching issues.

use std::path::Path;

use super::traits::CodingAgent;

/// Opencode agent for processing GitHub issues.
pub struct OpencodeAgent;

impl CodingAgent for OpencodeAgent {
    fn name(&self) -> &'static str {
        "Opencode"
    }

    fn cli_command(&self) -> &'static str {
        "opencode"
    }

    fn is_idle(&self, pane_content: &str) -> bool {
        is_opencode_idle(pane_content)
    }

    fn build_launch_command(&self, worktree_path: &Path, prompt: &str) -> String {
        // Opencode uses --prompt flag
        let escaped_prompt = prompt.replace('\'', "'\\''");
        format!(
            "cd '{}' && opencode --prompt '{}'",
            worktree_path.display(),
            escaped_prompt
        )
    }
}

/// Check if Opencode is idle (waiting for input).
/// Returns true if the last lines indicate Opencode is waiting for user input.
fn is_opencode_idle(pane_content: &str) -> bool {
    let lines: Vec<&str> = pane_content.lines().collect();

    // Get last non-empty lines (Opencode may have more footer content)
    let last_lines: Vec<&str> = lines
        .iter()
        .rev()
        .filter(|l| !l.trim().is_empty())
        .take(10)
        .copied()
        .collect();

    for line in &last_lines {
        let trimmed = line.trim();

        // Opencode footer when idle: "tab switch agent" or "ctrl+p command"
        if trimmed.contains("tab switch agent") || trimmed.contains("ctrl+p command") {
            return true;
        }

        // Opencode permission prompt
        if trimmed.contains("Permission required to run this tool:") {
            return true;
        }

        // Opencode permission dialog footer
        if trimmed.contains("enter accept") && trimmed.contains("a accept always") {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_detection_footer() {
        let agent = OpencodeAgent;
        let content = "Some output\n  tab switch agent   ctrl+p command\n";
        assert!(agent.is_idle(content));
    }

    #[test]
    fn idle_detection_ctrl_p() {
        let content = "Response from AI\nctrl+p command\n";
        assert!(is_opencode_idle(content));
    }

    #[test]
    fn idle_detection_permission_prompt() {
        let content = "# List running Docker containers\n$ docker ps\nPermission required to run this tool:\nenter accept  a accept always  d deny\n";
        assert!(is_opencode_idle(content));
    }

    #[test]
    fn idle_detection_permission_footer() {
        let content = "Some command\nenter accept  a accept always  d deny\n";
        assert!(is_opencode_idle(content));
    }

    #[test]
    fn not_idle_when_working() {
        let content = "Processing files...\nAnalyzing code...\n";
        assert!(!is_opencode_idle(content));
    }

    #[test]
    fn not_idle_response_in_progress() {
        let content = "Oui, ça va bien, merci ! Comment puis-je t'aider avec ton projet aujourd'hui ?\nBuild · claude-opus-4-5-20251101 · 3.0s\n";
        assert!(!is_opencode_idle(content));
    }
}
