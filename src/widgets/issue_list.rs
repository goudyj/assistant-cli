//! Issue list widget for displaying GitHub issues.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::ListItem,
};

use crate::agents::{AgentSession, AgentStatus};
use crate::github::IssueSummary;

/// Represents the display state of an issue in the list.
pub struct IssueItemState<'a> {
    pub issue: &'a IssueSummary,
    pub is_selected: bool,
    pub session: Option<&'a AgentSession>,
}

/// Build a ListItem for an issue with session status and selection state.
pub fn build_issue_item<'a>(
    issue: &'a IssueSummary,
    is_selected: bool,
    session: Option<&'a AgentSession>,
) -> ListItem<'a> {
    let state = IssueItemState {
        issue,
        is_selected,
        session,
    };
    build_issue_item_from_state(&state)
}

/// Build a ListItem from an IssueItemState.
pub fn build_issue_item_from_state<'a>(state: &IssueItemState<'a>) -> ListItem<'a> {
    let issue = state.issue;
    let is_selected = state.is_selected;

    // Selection marker
    let select_marker = if is_selected { "[x] " } else { "[ ] " };
    let select_style = if is_selected {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    // Session status
    let (session_icon, session_color, session_stats) = match state.session {
        Some(session) => {
            let (icon, color) = match &session.status {
                AgentStatus::Running => ("▶", Color::Yellow),
                AgentStatus::Awaiting => ("⏸", Color::Cyan),
                AgentStatus::Completed { .. } | AgentStatus::Failed { .. } => ("●", Color::Blue),
            };
            let stats = if session.stats.lines_added > 0 || session.stats.lines_deleted > 0 {
                format!(" +{} -{}", session.stats.lines_added, session.stats.lines_deleted)
            } else {
                String::new()
            };
            (Some(icon), color, stats)
        }
        None => (None, Color::DarkGray, String::new()),
    };

    // Labels and assignees
    let labels_str = if issue.labels.is_empty() {
        String::new()
    } else {
        format!(" [{}]", issue.labels.join(", "))
    };

    let assignees_str = if issue.assignees.is_empty() {
        String::new()
    } else {
        format!(" @{}", issue.assignees.join(", @"))
    };

    let is_closed = issue.state == "Closed";

    let line = if is_closed {
        // Closed issue style
        Line::from(vec![
            Span::styled(select_marker, select_style),
            Span::styled(
                format!("#{:<5}", issue.number),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(" ✓ ", Style::default().fg(Color::Green)),
            Span::styled(
                issue.title.clone(),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::CROSSED_OUT),
            ),
            Span::styled(labels_str, Style::default().fg(Color::DarkGray)),
            Span::styled(assignees_str, Style::default().fg(Color::DarkGray)),
        ])
    } else {
        // Open issue style
        let session_span = if let Some(icon) = session_icon {
            Span::styled(
                format!("{}{}  ", icon, session_stats),
                Style::default().fg(session_color),
            )
        } else {
            Span::raw("    ")
        };

        Line::from(vec![
            Span::styled(select_marker, select_style),
            Span::styled(
                format!("#{:<5}", issue.number),
                Style::default().fg(Color::Cyan),
            ),
            session_span,
            Span::raw(issue.title.clone()),
            Span::styled(labels_str, Style::default().fg(Color::DarkGray)),
            Span::styled(assignees_str, Style::default().fg(Color::Magenta)),
        ])
    };

    ListItem::new(line)
}

/// Build a dimmed ListItem for an issue (used in overlay backgrounds).
pub fn build_issue_item_dimmed(issue: &IssueSummary) -> ListItem<'_> {
    let labels_str = if issue.labels.is_empty() {
        String::new()
    } else {
        format!(" [{}]", issue.labels.join(", "))
    };

    let line = Line::from(vec![
        Span::styled(
            format!("#{:<5}", issue.number),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(issue.title.clone(), Style::default().fg(Color::DarkGray)),
        Span::styled(labels_str, Style::default().fg(Color::DarkGray)),
    ]);

    ListItem::new(line)
}

/// Configuration for the issue list title.
pub struct IssueListTitleConfig {
    pub search_query: Option<String>,
    pub total_count: usize,
    pub has_next_page: bool,
    pub is_loading: bool,
    pub selected_count: usize,
}

/// Build the title string for the issue list.
pub fn build_issue_list_title(config: &IssueListTitleConfig, base_title: &str) -> String {
    let mut parts = vec![base_title.to_string()];

    if let Some(ref query) = config.search_query {
        parts.push(format!("(filtered: '{}')", query));
    }

    if config.has_next_page {
        parts.push(format!("[{} loaded, more available]", config.total_count));
    } else if config.total_count > 20 {
        parts.push(format!("[{} total]", config.total_count));
    }

    if config.is_loading {
        parts.push("[Loading...]".to_string());
    }

    if config.selected_count > 0 {
        parts.push(format!("[{} selected]", config.selected_count));
    }

    parts.join(" ")
}
