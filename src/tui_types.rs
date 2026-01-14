//! TUI type definitions.

use std::path::PathBuf;

use std::collections::HashSet;

use crate::agents::WorktreeInfo;
use crate::github::{IssueDetail, PullRequestDetail, PullRequestSummary};
use crate::issues::IssueContent;
use crate::llm;

/// View state for the TUI
pub enum TuiView {
    List,
    Search { input: String },
    Detail(IssueDetail),
    AddComment { issue: IssueDetail, input: String },
    ConfirmClose { issue: IssueDetail },
    ConfirmReopen { issue: IssueDetail },
    AssignUser {
        issue: IssueDetail,
        input: String,
        suggestions: Vec<String>,
        selected: usize,
    },
    ConfirmDispatch { issue: IssueDetail },
    AgentLogs {
        session_id: String,
        content: String,
        scroll: u16,
    },
    EmbeddedTmux {
        /// Available tmux sessions for switching
        available_sessions: Vec<String>,
        /// Current session index
        current_index: usize,
        /// Return to worktree list instead of issue list
        return_to_worktrees: bool,
    },
    /// Project selection screen
    ProjectSelect {
        projects: Vec<String>,
        selected: usize,
    },
    /// Agent selection screen (claude/opencode)
    AgentSelect {
        selected: usize,
    },
    /// Command palette for custom commands
    Command {
        input: String,
        suggestions: Vec<CommandSuggestion>,
        selected: usize,
    },
    /// Issue creation flow
    CreateIssue {
        input: String,
        stage: CreateStage,
    },
    /// Preview generated issue before creation
    PreviewIssue {
        issue: IssueContent,
        messages: Vec<llm::Message>,
        feedback_input: String,
        scroll: u16,
    },
    /// Direct issue creation (no AI)
    DirectIssue {
        title: String,
        body: String,
        editing_body: bool,
    },
    /// Worktree management list
    WorktreeList {
        worktrees: Vec<WorktreeInfo>,
        selected: usize,
    },
    /// Confirm prune of orphaned worktrees
    ConfirmPrune {
        orphaned: Vec<WorktreeInfo>,
    },
    /// Confirm deletion of a single worktree
    ConfirmDeleteWorktree {
        worktree: WorktreeInfo,
        /// Index to return to in the worktree list
        return_index: usize,
    },
    /// Create new worktree with custom branch
    CreateWorktree {
        input: String,
    },
    /// Post worktree creation choice
    PostWorktreeCreate {
        worktree_path: PathBuf,
        branch_name: String,
    },
    /// Dispatch issue with optional additional instructions
    DispatchInstructions {
        issue: IssueDetail,
        input: String,
    },
    /// Start agent on worktree with optional instructions
    WorktreeAgentInstructions {
        worktree_path: PathBuf,
        branch_name: String,
        input: String,
    },
    /// Help screen showing all shortcuts
    Help,
    /// Pull request list view
    PullRequestList,
    /// Pull request detail view
    PullRequestDetail(PullRequestDetail),
    /// Confirm merge of a pull request
    ConfirmMerge {
        pr: PullRequestDetail,
    },
    /// Dispatch agent for PR review
    DispatchPrReview {
        pr: PullRequestDetail,
        input: String,
    },
    /// PR filters popup
    PrFilters {
        status_filter: HashSet<PrStatus>,
        author_filter: HashSet<String>,
        available_authors: Vec<String>,
        focus: PrFilterFocus,
        selected_status: usize,
        selected_author: usize,
        author_input: String,
        author_suggestions: Vec<String>,
    },
    /// Issue filters popup
    IssueFilters {
        status_filter: HashSet<IssueStatus>,
        author_filter: HashSet<String>,
        available_authors: Vec<String>,
        focus: IssueFilterFocus,
        selected_status: usize,
        selected_author: usize,
        author_input: String,
        author_suggestions: Vec<String>,
    },
}

/// Stages of issue creation
#[derive(Clone)]
pub enum CreateStage {
    /// User typing description
    Description,
    /// Waiting for LLM
    Generating,
}

/// Command suggestion for the command palette
#[derive(Clone)]
pub struct CommandSuggestion {
    pub name: String,
    pub description: String,
    pub labels: Option<Vec<String>>,
}

/// Pull request status for filtering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrStatus {
    Draft,
    Open,
    Merged,
    Closed,
}

impl PrStatus {
    pub fn all() -> Vec<PrStatus> {
        vec![PrStatus::Draft, PrStatus::Open, PrStatus::Merged, PrStatus::Closed]
    }

    pub fn label(&self) -> &'static str {
        match self {
            PrStatus::Draft => "Draft",
            PrStatus::Open => "Open",
            PrStatus::Merged => "Merged",
            PrStatus::Closed => "Closed",
        }
    }

    /// Check if a PR matches this status
    pub fn matches(&self, pr: &PullRequestSummary) -> bool {
        match self {
            PrStatus::Draft => pr.draft,
            PrStatus::Open => !pr.draft && pr.state.to_lowercase().contains("open"),
            PrStatus::Merged => pr.state.to_lowercase().contains("merged"),
            PrStatus::Closed => !pr.state.to_lowercase().contains("merged")
                && pr.state.to_lowercase().contains("closed"),
        }
    }
}

/// Focus area in PR filters popup
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrFilterFocus {
    Status,
    Author,
}

/// Issue status for filtering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IssueStatus {
    Open,
    Closed,
}

impl IssueStatus {
    pub fn all() -> Vec<IssueStatus> {
        vec![IssueStatus::Open, IssueStatus::Closed]
    }

    pub fn label(&self) -> &'static str {
        match self {
            IssueStatus::Open => "Open",
            IssueStatus::Closed => "Closed",
        }
    }

    /// Check if an issue matches this status
    pub fn matches(&self, issue: &crate::github::IssueSummary) -> bool {
        match self {
            IssueStatus::Open => issue.state.to_lowercase().contains("open"),
            IssueStatus::Closed => issue.state.to_lowercase().contains("closed"),
        }
    }
}

/// Focus area in Issue filters popup
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueFilterFocus {
    Status,
    Author,
}
