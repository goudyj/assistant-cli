//! Trait definitions for coding agents.

use std::path::Path;

use crate::config::CodingAgentType;

use super::claude::ClaudeCodeAgent;
use super::opencode::OpencodeAgent;

/// Trait for coding agents that can process GitHub issues.
pub trait CodingAgent: Send + Sync {
    /// Returns the display name of this agent (e.g., "Claude Code", "Opencode")
    fn name(&self) -> &'static str;

    /// Returns the CLI command to invoke this agent (e.g., "claude", "opencode")
    fn cli_command(&self) -> &'static str;

    /// Check if the agent is idle (waiting for user input) based on tmux pane content.
    fn is_idle(&self, pane_content: &str) -> bool;

    /// Build the shell command to launch the agent with a prompt.
    fn build_launch_command(&self, worktree_path: &Path, prompt: &str) -> String {
        let escaped_prompt = prompt.replace('\'', "'\\''");
        format!(
            "cd '{}' && {} '{}'",
            worktree_path.display(),
            self.cli_command(),
            escaped_prompt
        )
    }
}

/// Factory function to get the appropriate agent based on configuration.
pub fn get_agent(agent_type: &CodingAgentType) -> Box<dyn CodingAgent> {
    match agent_type {
        CodingAgentType::Claude => Box::new(ClaudeCodeAgent),
        CodingAgentType::Opencode => Box::new(OpencodeAgent),
    }
}
