//! Slash commands (command palette) definitions.

/// Built-in slash commands available in the command palette.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SlashCommand {
    /// Show all issues (clear filters)
    All,
    /// Show issues list
    Issues,
    /// Show pull requests list
    Prs,
    /// Logout from GitHub
    Logout,
    /// Switch repository
    Repository,
    /// Manage worktrees
    Worktrees,
    /// Clean up orphaned worktrees
    Prune,
    /// Select dispatch agent
    Agent,
    /// Custom filter command from config
    Custom { name: String, labels: Vec<String> },
}

impl SlashCommand {
    /// All built-in commands.
    pub fn builtins() -> Vec<Self> {
        vec![
            Self::All,
            Self::Issues,
            Self::Prs,
            Self::Logout,
            Self::Repository,
            Self::Worktrees,
            Self::Prune,
            Self::Agent,
        ]
    }

    /// Command name (without the leading /).
    pub fn name(&self) -> &str {
        match self {
            Self::All => "all",
            Self::Issues => "issues",
            Self::Prs => "prs",
            Self::Logout => "logout",
            Self::Repository => "repository",
            Self::Worktrees => "worktrees",
            Self::Prune => "prune",
            Self::Agent => "agent",
            Self::Custom { name, .. } => name,
        }
    }

    /// Description for the command palette.
    pub fn description(&self) -> String {
        match self {
            Self::All => "Show all issues (clear filters)".to_string(),
            Self::Issues => "Show issues list".to_string(),
            Self::Prs => "Show pull requests list".to_string(),
            Self::Logout => "Logout from GitHub".to_string(),
            Self::Repository => "Switch repository".to_string(),
            Self::Worktrees => "Manage worktrees (view, delete, open IDE)".to_string(),
            Self::Prune => "Clean up orphaned worktrees".to_string(),
            Self::Agent => "Select dispatch agent (Claude Code or Opencode)".to_string(),
            Self::Custom { labels, .. } => format!("Filter: {}", labels.join(", ")),
        }
    }

    /// Labels for custom filter commands.
    pub fn labels(&self) -> Option<&Vec<String>> {
        match self {
            Self::Custom { labels, .. } => Some(labels),
            _ => None,
        }
    }
}
