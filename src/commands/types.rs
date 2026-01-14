//! Command system types.

/// The view/context where a command or shortcut is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandContext {
    /// Available everywhere
    Global,
    /// Issue list view
    IssueList,
    /// Issue detail view
    IssueDetail,
    /// Worktree list view
    WorktreeList,
    /// Pull request list view
    PullRequestList,
    /// Pull request detail view
    PullRequestDetail,
    /// Embedded tmux terminal
    EmbeddedTmux,
}

/// Category for grouping shortcuts in help display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandCategory {
    Navigation,
    Issues,
    Agent,
    Tmux,
    PullRequests,
    Worktrees,
    Other,
}

impl CommandCategory {
    /// Display name for help screen headers.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Navigation => "NAVIGATION",
            Self::Issues => "ISSUES",
            Self::Agent => "AGENT / WORKTREE",
            Self::Tmux => "TMUX",
            Self::PullRequests => "PULL REQUESTS",
            Self::Worktrees => "WORKTREES",
            Self::Other => "OTHER",
        }
    }

    /// Order for help display (lower = first).
    pub fn order(&self) -> u8 {
        match self {
            Self::Navigation => 0,
            Self::Issues => 1,
            Self::Agent => 2,
            Self::Tmux => 3,
            Self::PullRequests => 4,
            Self::Worktrees => 5,
            Self::Other => 6,
        }
    }
}
