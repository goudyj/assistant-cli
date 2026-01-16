//! Keyboard shortcuts definitions.

use super::types::{CommandCategory, CommandContext};

/// All keyboard shortcuts in the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Shortcut {
    // Navigation
    MoveDown,
    MoveUp,
    OpenDetail,
    GoBack,
    Quit,

    // Issue List
    Search,
    OpenCommandPalette,
    ClearReload,
    SelectIssue,
    Refresh,
    OpenFilters,
    OpenHelp,
    SwitchToPRs,

    // Issue actions
    CreateIssueAI,
    CreateIssueDirect,
    AddComment,
    OpenInBrowser,
    OpenImage,
    DisplayImage,
    CloseIssue,
    ReopenIssue,
    AssignUser,

    // Agent / Worktree
    DispatchAgent,
    StartAgent,
    OpenTmux,
    OpenAnyTmux,
    ViewLogs,
    CreatePR,
    KillAgent,
    OpenIDE,
    DeleteWorktree,
    CreateWorktree,

    // PR actions
    SwitchToIssues,
    CheckoutBranch,
    ReviewPR,
    MergePR,

    // Embedded terminal
    ExitTerminal,
    PrevSession,
    NextSession,
}

impl Shortcut {
    /// All shortcuts.
    pub fn all() -> Vec<Self> {
        vec![
            Self::MoveDown,
            Self::MoveUp,
            Self::OpenDetail,
            Self::GoBack,
            Self::Quit,
            Self::Search,
            Self::OpenCommandPalette,
            Self::ClearReload,
            Self::SelectIssue,
            Self::Refresh,
            Self::OpenFilters,
            Self::OpenHelp,
            Self::SwitchToPRs,
            Self::CreateIssueAI,
            Self::CreateIssueDirect,
            Self::AddComment,
            Self::OpenInBrowser,
            Self::OpenImage,
            Self::DisplayImage,
            Self::CloseIssue,
            Self::ReopenIssue,
            Self::AssignUser,
            Self::DispatchAgent,
            Self::StartAgent,
            Self::OpenTmux,
            Self::OpenAnyTmux,
            Self::ViewLogs,
            Self::CreatePR,
            Self::KillAgent,
            Self::OpenIDE,
            Self::DeleteWorktree,
            Self::CreateWorktree,
            Self::SwitchToIssues,
            Self::CheckoutBranch,
            Self::ReviewPR,
            Self::MergePR,
            Self::ExitTerminal,
            Self::PrevSession,
            Self::NextSession,
        ]
    }

    /// Human-readable key representation for help display.
    pub fn key_display(&self) -> &'static str {
        match self {
            Self::MoveDown => "j/\u{2193}",
            Self::MoveUp => "k/\u{2191}",
            Self::OpenDetail => "Enter",
            Self::GoBack => "Esc",
            Self::Quit => "q",
            Self::Search => "s",
            Self::OpenCommandPalette => "/",
            Self::ClearReload => "c",
            Self::SelectIssue => "Space",
            Self::Refresh => "R",
            Self::OpenFilters => "f",
            Self::OpenHelp => "?",
            Self::SwitchToPRs => "Tab",
            Self::CreateIssueAI => "C",
            Self::CreateIssueDirect => "N",
            Self::AddComment => "c",
            Self::OpenInBrowser => "o",
            Self::OpenImage => "O",
            Self::DisplayImage => "i",
            Self::CloseIssue => "x",
            Self::ReopenIssue => "X",
            Self::AssignUser => "a",
            Self::DispatchAgent => "d",
            Self::StartAgent => "a",
            Self::OpenTmux => "t",
            Self::OpenAnyTmux => "T",
            Self::ViewLogs => "l",
            Self::CreatePR => "p",
            Self::KillAgent => "K",
            Self::OpenIDE => "o",
            Self::DeleteWorktree => "d/W",
            Self::CreateWorktree => "n",
            Self::SwitchToIssues => "Tab",
            Self::CheckoutBranch => "c",
            Self::ReviewPR => "r",
            Self::MergePR => "m",
            Self::ExitTerminal => "Ctrl+Q",
            Self::PrevSession => "Ctrl+\u{2190}",
            Self::NextSession => "Ctrl+\u{2192}",
        }
    }

    /// Description for help text.
    pub fn description(&self) -> &'static str {
        match self {
            Self::MoveDown => "Move down",
            Self::MoveUp => "Move up",
            Self::OpenDetail => "Open details",
            Self::GoBack => "Go back",
            Self::Quit => "Quit",
            Self::Search => "Search GitHub",
            Self::OpenCommandPalette => "Open command palette",
            Self::ClearReload => "Clear search / Reload",
            Self::SelectIssue => "Select / Deselect issue",
            Self::Refresh => "Refresh list",
            Self::OpenFilters => "Open filters",
            Self::OpenHelp => "Show help",
            Self::SwitchToPRs => "Switch to PRs",
            Self::CreateIssueAI => "Create issue (AI)",
            Self::CreateIssueDirect => "Create issue (direct)",
            Self::AddComment => "Add comment",
            Self::OpenInBrowser => "Open in browser",
            Self::OpenImage => "Open image in browser",
            Self::DisplayImage => "Display image inline",
            Self::CloseIssue => "Close issue",
            Self::ReopenIssue => "Reopen issue",
            Self::AssignUser => "Assign user",
            Self::DispatchAgent => "Dispatch agent",
            Self::StartAgent => "Start agent",
            Self::OpenTmux => "Open tmux session",
            Self::OpenAnyTmux => "Open any tmux session",
            Self::ViewLogs => "View agent logs",
            Self::CreatePR => "Create pull request",
            Self::KillAgent => "Kill agent",
            Self::OpenIDE => "Open in IDE",
            Self::DeleteWorktree => "Delete worktree",
            Self::CreateWorktree => "Create worktree",
            Self::SwitchToIssues => "Switch to issues",
            Self::CheckoutBranch => "Checkout as worktree",
            Self::ReviewPR => "Review with agent",
            Self::MergePR => "Merge PR",
            Self::ExitTerminal => "Exit terminal",
            Self::PrevSession => "Previous session",
            Self::NextSession => "Next session",
        }
    }

    /// Short description for status bar.
    pub fn short_desc(&self) -> &'static str {
        match self {
            Self::CreateIssueAI => "ai",
            Self::CreateIssueDirect => "new",
            Self::DispatchAgent => "dispatch",
            Self::StartAgent => "agent",
            Self::OpenIDE => "ide",
            Self::CreatePR => "pr",
            Self::OpenTmux => "tmux",
            Self::Refresh => "refresh",
            Self::OpenCommandPalette => "cmd",
            Self::OpenHelp => "help",
            Self::Quit => "quit",
            Self::Search => "search",
            Self::OpenFilters => "filter",
            Self::SelectIssue => "select",
            Self::ViewLogs => "logs",
            Self::KillAgent => "kill",
            Self::SwitchToPRs => "prs",
            Self::SwitchToIssues => "issues",
            Self::GoBack => "back",
            Self::AddComment => "comment",
            Self::AssignUser => "assign",
            Self::CloseIssue => "close",
            Self::MergePR => "merge",
            Self::ReviewPR => "review",
            Self::CheckoutBranch => "checkout",
            _ => self.description(),
        }
    }

    /// Category for help grouping.
    pub fn category(&self) -> CommandCategory {
        match self {
            Self::MoveDown | Self::MoveUp | Self::OpenDetail | Self::GoBack | Self::Quit => {
                CommandCategory::Navigation
            }

            Self::Search
            | Self::OpenCommandPalette
            | Self::ClearReload
            | Self::SelectIssue
            | Self::Refresh
            | Self::OpenFilters
            | Self::OpenHelp
            | Self::CreateIssueAI
            | Self::CreateIssueDirect
            | Self::AddComment
            | Self::OpenInBrowser
            | Self::OpenImage
            | Self::DisplayImage
            | Self::CloseIssue
            | Self::ReopenIssue
            | Self::AssignUser => CommandCategory::Issues,

            Self::DispatchAgent
            | Self::StartAgent
            | Self::ViewLogs
            | Self::KillAgent
            | Self::OpenIDE
            | Self::DeleteWorktree
            | Self::CreateWorktree
            | Self::CreatePR => CommandCategory::Agent,

            Self::OpenTmux
            | Self::OpenAnyTmux
            | Self::ExitTerminal
            | Self::PrevSession
            | Self::NextSession => CommandCategory::Tmux,

            Self::SwitchToPRs
            | Self::SwitchToIssues
            | Self::CheckoutBranch
            | Self::ReviewPR
            | Self::MergePR => CommandCategory::PullRequests,
        }
    }

    /// Contexts where this shortcut is available.
    pub fn contexts(&self) -> &'static [CommandContext] {
        match self {
            // Navigation - most views
            Self::MoveDown | Self::MoveUp => &[
                CommandContext::IssueList,
                CommandContext::IssueDetail,
                CommandContext::WorktreeList,
                CommandContext::PullRequestList,
                CommandContext::PullRequestDetail,
            ],
            Self::OpenDetail => &[
                CommandContext::IssueList,
                CommandContext::PullRequestList,
            ],
            Self::GoBack => &[
                CommandContext::IssueDetail,
                CommandContext::WorktreeList,
                CommandContext::PullRequestList,
                CommandContext::PullRequestDetail,
            ],
            Self::Quit => &[CommandContext::Global],

            // Issue list specific
            Self::Search
            | Self::OpenCommandPalette
            | Self::ClearReload
            | Self::SelectIssue
            | Self::CreateIssueAI
            | Self::CreateIssueDirect
            | Self::OpenAnyTmux => &[CommandContext::IssueList],

            Self::Refresh | Self::OpenFilters => &[
                CommandContext::IssueList,
                CommandContext::PullRequestList,
            ],

            Self::OpenHelp => &[
                CommandContext::IssueList,
                CommandContext::PullRequestList,
            ],

            Self::SwitchToPRs => &[CommandContext::IssueList],

            // Issue detail
            Self::AddComment
            | Self::OpenImage
            | Self::DisplayImage
            | Self::CloseIssue
            | Self::ReopenIssue
            | Self::AssignUser => &[CommandContext::IssueDetail],

            // Agent actions in multiple views
            Self::DispatchAgent => &[
                CommandContext::IssueList,
                CommandContext::IssueDetail,
            ],
            Self::OpenTmux => &[
                CommandContext::IssueList,
                CommandContext::WorktreeList,
            ],
            Self::ViewLogs | Self::KillAgent => &[CommandContext::IssueList],
            Self::OpenIDE => &[
                CommandContext::IssueList,
                CommandContext::WorktreeList,
            ],
            Self::DeleteWorktree => &[
                CommandContext::IssueList,
                CommandContext::WorktreeList,
            ],
            Self::CreatePR => &[
                CommandContext::IssueList,
                CommandContext::WorktreeList,
            ],
            Self::CreateWorktree => &[CommandContext::WorktreeList],
            Self::StartAgent => &[CommandContext::WorktreeList],

            // Browser open
            Self::OpenInBrowser => &[
                CommandContext::IssueDetail,
                CommandContext::WorktreeList,
                CommandContext::PullRequestList,
                CommandContext::PullRequestDetail,
            ],

            // PR specific
            Self::SwitchToIssues => &[CommandContext::PullRequestList],
            Self::CheckoutBranch => &[CommandContext::PullRequestList],
            Self::ReviewPR => &[
                CommandContext::PullRequestList,
                CommandContext::PullRequestDetail,
            ],
            Self::MergePR => &[
                CommandContext::PullRequestList,
                CommandContext::PullRequestDetail,
            ],

            // Embedded terminal
            Self::ExitTerminal | Self::PrevSession | Self::NextSession => {
                &[CommandContext::EmbeddedTmux]
            }
        }
    }
}
