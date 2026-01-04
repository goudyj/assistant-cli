//! TUI type definitions.

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
    },
    /// Project selection screen
    ProjectSelect {
        projects: Vec<String>,
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
