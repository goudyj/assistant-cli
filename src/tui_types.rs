//! TUI type definitions.

use std::path::PathBuf;

use crate::agents::WorktreeInfo;
use crate::github::IssueDetail;
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
    AgentDiff {
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
    /// Create new worktree with custom branch
    CreateWorktree {
        input: String,
    },
    /// Post worktree creation choice
    PostWorktreeCreate {
        worktree_path: PathBuf,
        branch_name: String,
    },
    /// Help screen showing all shortcuts
    Help,
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
