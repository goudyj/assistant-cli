use crate::auth::{self, DeviceFlowAuth};
use crate::github::{GitHubConfig, IssueDetail, IssueSummary};
use crate::issues::IssueContent;
use crate::llm;
use crossterm::{
    event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use image::ImageReader;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, StatefulImage};
use regex::Regex;
use std::io::{self, Cursor};

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

/// Main TUI state
pub struct IssueBrowser {
    pub all_issues: Vec<IssueSummary>,
    pub issues: Vec<IssueSummary>,
    pub list_state: ListState,
    pub view: TuiView,
    pub scroll_offset: u16,
    pub should_quit: bool,
    pub github: GitHubConfig,
    pub github_token: Option<String>,
    pub auto_format: bool,
    pub llm_endpoint: String,
    pub status_message: Option<String>,
    pub current_images: Vec<String>,
    pub current_image_index: usize,
    pub search_query: Option<String>,
    // Pagination state
    pub current_page: u32,
    pub has_next_page: bool,
    pub is_loading: bool,
    pub list_labels: Vec<String>,
    pub list_state_filter: crate::list::IssueState,
    // Assignees cache
    pub available_assignees: Vec<String>,
    // Project info for Claude Code dispatch
    pub project_name: Option<String>,
    pub local_path: Option<std::path::PathBuf>,
    // Multi-select for batch dispatch
    pub selected_issues: std::collections::HashSet<u64>,
    // Session cache for dispatch status display
    pub session_cache: std::collections::HashMap<u64, crate::agents::AgentSession>,
    // Embedded terminal for tmux sessions
    pub embedded_term: Option<crate::embedded_term::EmbeddedTerminal>,
    // Last session cache refresh time
    pub last_session_refresh: std::time::Instant,
    // Project labels for issue creation
    pub project_labels: Vec<String>,
    // Available commands for command palette
    pub available_commands: Vec<CommandSuggestion>,
    // Last ESC press time for double-ESC quit
    pub last_esc_press: Option<std::time::Instant>,
}

impl IssueBrowser {
    pub fn new(
        issues: Vec<IssueSummary>,
        github: GitHubConfig,
        github_token: Option<String>,
        auto_format: bool,
        llm_endpoint: String,
    ) -> Self {
        Self::with_pagination(
            issues,
            github,
            github_token,
            auto_format,
            llm_endpoint,
            Vec::new(),
            crate::list::IssueState::Open,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_pagination(
        issues: Vec<IssueSummary>,
        github: GitHubConfig,
        github_token: Option<String>,
        auto_format: bool,
        llm_endpoint: String,
        list_labels: Vec<String>,
        list_state_filter: crate::list::IssueState,
        has_next_page: bool,
    ) -> Self {
        let mut list_state = ListState::default();
        if !issues.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            all_issues: issues.clone(),
            issues,
            list_state,
            view: TuiView::List,
            scroll_offset: 0,
            should_quit: false,
            github,
            github_token,
            auto_format,
            llm_endpoint,
            status_message: None,
            current_images: Vec::new(),
            current_image_index: 0,
            search_query: None,
            current_page: 1,
            has_next_page,
            is_loading: false,
            list_labels,
            list_state_filter,
            available_assignees: Vec::new(),
            project_name: None,
            local_path: None,
            selected_issues: std::collections::HashSet::new(),
            session_cache: std::collections::HashMap::new(),
            embedded_term: None,
            last_session_refresh: std::time::Instant::now(),
            project_labels: Vec::new(),
            available_commands: Vec::new(),
            last_esc_press: None,
        }
    }

    /// Set project info for Claude Code dispatch
    pub fn set_project_info(&mut self, name: String, path: std::path::PathBuf) {
        self.project_name = Some(name.clone());
        self.local_path = Some(path);
        // Load sessions for this project
        self.refresh_sessions(&name);
    }

    /// Set project labels for issue creation
    pub fn set_project_labels(&mut self, labels: Vec<String>) {
        self.project_labels = labels;
    }

    /// Set available commands for the command palette
    pub fn set_available_commands(&mut self, commands: Vec<CommandSuggestion>) {
        self.available_commands = commands;
    }

    /// Get filtered command suggestions based on input
    pub fn get_command_suggestions(&self, input: &str) -> Vec<CommandSuggestion> {
        if input.is_empty() {
            return self.available_commands.clone();
        }
        let input_lower = input.to_lowercase();
        self.available_commands
            .iter()
            .filter(|cmd| cmd.name.to_lowercase().contains(&input_lower))
            .cloned()
            .collect()
    }

    /// Refresh session cache for the current project
    pub fn refresh_sessions(&mut self, project: &str) {
        let mut manager = crate::agents::SessionManager::load();
        // Sync with actual tmux state before displaying
        if manager.sync_with_tmux() {
            let _ = manager.save();
        }
        self.session_cache.clear();
        for session in manager.list() {
            if session.project == project {
                self.session_cache.insert(session.issue_number, session.clone());
            }
        }
    }

    /// Refresh session cache with fresh stats calculated from git
    pub fn refresh_sessions_with_fresh_stats(&mut self, project: &str) {
        let mut manager = crate::agents::SessionManager::load();

        // Collect running sessions info first to avoid borrow issues
        let running_sessions: Vec<_> = manager
            .running()
            .iter()
            .map(|s| (s.id.clone(), s.worktree_path.clone()))
            .collect();

        // Recalculate stats for all running sessions
        for (session_id, worktree_path) in running_sessions {
            let (lines_added, lines_deleted, files_changed) =
                crate::agents::get_diff_stats(&worktree_path);
            let stats = crate::agents::AgentStats {
                lines_output: 0,
                lines_added,
                lines_deleted,
                files_changed,
            };
            manager.update_stats(&session_id, stats);
        }
        let _ = manager.save();

        // Sync with actual tmux state
        if manager.sync_with_tmux() {
            let _ = manager.save();
        }

        self.session_cache.clear();
        for session in manager.list() {
            if session.project == project {
                self.session_cache.insert(session.issue_number, session.clone());
            }
        }
    }

    /// Load the next page of issues
    pub async fn load_next_page(&mut self) {
        if !self.has_next_page || self.is_loading {
            return;
        }

        self.is_loading = true;
        self.status_message = Some("Loading more issues...".to_string());

        let next_page = self.current_page + 1;
        match self
            .github
            .list_issues_paginated(&self.list_labels, &self.list_state_filter, 20, next_page)
            .await
        {
            Ok((new_issues, has_next)) => {
                // Append new issues to the list
                self.all_issues.extend(new_issues.clone());

                // If we have a search filter, apply it to the new issues too
                if let Some(ref query) = self.search_query {
                    let query_lower = query.to_lowercase();
                    let filtered: Vec<_> = new_issues
                        .into_iter()
                        .filter(|issue| {
                            issue.title.to_lowercase().contains(&query_lower)
                                || issue.labels.iter().any(|l| l.to_lowercase().contains(&query_lower))
                        })
                        .collect();
                    self.issues.extend(filtered);
                } else {
                    self.issues.extend(new_issues);
                }

                self.current_page = next_page;
                self.has_next_page = has_next;
                self.status_message = Some(format!("Loaded page {} ({} issues total)", next_page, self.all_issues.len()));
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to load more: {}", e));
            }
        }
        self.is_loading = false;
    }

    /// Reload issues from scratch (page 1)
    pub async fn reload_issues(&mut self) {
        self.is_loading = true;

        match self
            .github
            .list_issues_paginated(&self.list_labels, &self.list_state_filter, 20, 1)
            .await
        {
            Ok((new_issues, has_next)) => {
                self.all_issues = new_issues.clone();
                self.issues = new_issues;
                self.current_page = 1;
                self.has_next_page = has_next;
                self.search_query = None;

                // Reset selection to first item
                if !self.issues.is_empty() {
                    self.list_state.select(Some(0));
                }
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to reload: {}", e));
            }
        }
        self.is_loading = false;
    }

    /// Filter issues based on search query
    pub fn apply_search_filter(&mut self, query: &str) {
        if query.is_empty() {
            self.issues = self.all_issues.clone();
            self.search_query = None;
        } else {
            let query_lower = query.to_lowercase();
            self.issues = self.all_issues
                .iter()
                .filter(|issue| {
                    issue.title.to_lowercase().contains(&query_lower)
                        || issue.labels.iter().any(|l| l.to_lowercase().contains(&query_lower))
                })
                .cloned()
                .collect();
            self.search_query = Some(query.to_string());
        }

        // Reset selection
        if !self.issues.is_empty() {
            self.list_state.select(Some(0));
        } else {
            self.list_state.select(None);
        }
    }

    /// Clear search filter and restore all issues
    pub fn clear_search(&mut self) {
        self.issues = self.all_issues.clone();
        self.search_query = None;
        if !self.issues.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    /// Load available assignees from GitHub API
    pub async fn load_assignees(&mut self) {
        if self.available_assignees.is_empty()
            && let Ok(assignees) = self.github.list_assignees().await
        {
            self.available_assignees = assignees;
        }
    }

    /// Get filtered assignee suggestions based on input (fuzzy matching)
    pub fn get_assignee_suggestions(&self, input: &str) -> Vec<String> {
        if input.is_empty() {
            return self.available_assignees.clone();
        }

        let matcher = SkimMatcherV2::default();
        let mut scored: Vec<(i64, &String)> = self
            .available_assignees
            .iter()
            .filter_map(|name| matcher.fuzzy_match(name, input).map(|score| (score, name)))
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.into_iter().map(|(_, name)| name.clone()).collect()
    }

    /// Extract image URLs from issue content
    pub fn extract_images_from_issue(&mut self, issue: &IssueDetail) {
        let mut images = Vec::new();

        // Extract from body
        if let Some(ref body) = issue.body {
            images.extend(extract_image_urls(body));
        }

        // Extract from comments
        for comment in &issue.comments {
            images.extend(extract_image_urls(&comment.body));
        }

        self.current_images = images;
        self.current_image_index = 0;
    }

    pub fn selected_issue(&self) -> Option<&IssueSummary> {
        self.list_state.selected().and_then(|i| self.issues.get(i))
    }

    pub fn next(&mut self) {
        if self.issues.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => (i + 1) % self.issues.len(),
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn previous(&mut self) {
        if self.issues.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.issues.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }
}

/// Run the TUI application
pub async fn run_issue_browser(
    issues: Vec<IssueSummary>,
    github: GitHubConfig,
    github_token: Option<String>,
    auto_format: bool,
    llm_endpoint: &str,
) -> io::Result<()> {
    run_issue_browser_with_pagination(
        issues,
        github,
        github_token,
        auto_format,
        llm_endpoint,
        Vec::new(),
        crate::list::IssueState::Open,
        false,
        None,
        None,
        Vec::new(),
        Vec::new(),
    )
    .await
}

/// Run the TUI application with pagination support
#[allow(clippy::too_many_arguments)]
pub async fn run_issue_browser_with_pagination(
    issues: Vec<IssueSummary>,
    github: GitHubConfig,
    github_token: Option<String>,
    auto_format: bool,
    llm_endpoint: &str,
    labels: Vec<String>,
    state_filter: crate::list::IssueState,
    has_next_page: bool,
    project_name: Option<String>,
    local_path: Option<std::path::PathBuf>,
    project_labels: Vec<String>,
    available_commands: Vec<CommandSuggestion>,
) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    // No mouse capture to allow text selection / copy-paste
    // Enable bracketed paste for handling pasted text
    execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut browser = IssueBrowser::with_pagination(
        issues,
        github,
        github_token,
        auto_format,
        llm_endpoint.to_string(),
        labels,
        state_filter,
        has_next_page,
    );

    // Set project info for Claude Code dispatch
    if let (Some(name), Some(path)) = (project_name, local_path) {
        browser.set_project_info(name, path);
    }

    // Set labels and commands for issue creation and command palette
    browser.set_project_labels(project_labels);
    browser.set_available_commands(available_commands);

    // Resume monitoring threads for any running sessions from previous process
    crate::agents::resume_monitoring_for_running_sessions();

    while !browser.should_quit {
        // Auto-refresh session cache every 2 seconds when in List view
        if matches!(browser.view, TuiView::List)
            && browser.last_session_refresh.elapsed() >= std::time::Duration::from_secs(2)
        {
            if let Some(project) = browser.project_name.clone() {
                browser.refresh_sessions(&project);
            }
            browser.last_session_refresh = std::time::Instant::now();
        }

        terminal.draw(|f| draw_ui(f, &mut browser))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    // Handle Ctrl+V for paste from clipboard (if available)
                    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('v') {
                        // Try to paste from clipboard
                        if let Ok(content) = get_clipboard_content() {
                            handle_paste(&mut browser, &content);
                        }
                    } else {
                        handle_key_event(&mut browser, key.code, key.modifiers).await;
                    }
                }
                Event::Paste(content) => {
                    // Handle bracketed paste
                    handle_paste(&mut browser, &content);
                }
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), DisableBracketedPaste, LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

fn draw_ui(f: &mut Frame, browser: &mut IssueBrowser) {
    let image_count = browser.current_images.len();

    // Auto-refresh log content if viewing running agent logs
    if let TuiView::AgentLogs { session_id, content, .. } = &mut browser.view {
        let manager = crate::agents::SessionManager::load();
        if let Some(session) = manager.get(session_id)
            && session.is_running()
                && let Ok(new_content) = std::fs::read_to_string(&session.log_file) {
                    *content = new_content;
                }
    }

    // Clone status message to avoid borrow conflicts
    let status_msg = browser.status_message.clone();

    match &browser.view {
        TuiView::List => {
            if let Some(ref msg) = status_msg {
                let chunks = Layout::vertical([Constraint::Min(3), Constraint::Length(3)])
                    .split(f.area());
                draw_list_view_in_area(f, browser, chunks[0]);
                draw_status_bar(f, chunks[1], msg);
            } else {
                draw_list_view(f, browser);
            }
        }
        TuiView::Search { input } => {
            // Split screen: list on top (90%), search input at bottom (10%)
            let chunks = Layout::vertical([Constraint::Min(3), Constraint::Length(3)])
                .split(f.area());

            let input_clone = input.clone();
            draw_list_view_in_area(f, browser, chunks[0]);
            draw_search_input(f, chunks[1], &input_clone);
        }
        TuiView::Detail(issue) => {
            if let Some(ref msg) = status_msg {
                // Show status message at the bottom
                let chunks = Layout::vertical([Constraint::Min(3), Constraint::Length(3)])
                    .split(f.area());
                draw_detail_view(f, chunks[0], issue, browser.scroll_offset, image_count);
                draw_status_bar(f, chunks[1], msg);
            } else {
                draw_detail_view(f, f.area(), issue, browser.scroll_offset, image_count);
            }
        }
        TuiView::AddComment { issue, input } => {
            // Split screen: issue on top (75%), comment input at bottom (25%)
            let chunks = Layout::vertical([Constraint::Percentage(75), Constraint::Percentage(25)])
                .split(f.area());

            draw_detail_view(f, chunks[0], issue, browser.scroll_offset, image_count);
            draw_comment_input(f, chunks[1], input, browser.status_message.as_deref());
        }
        TuiView::ConfirmClose { issue } => {
            let chunks = Layout::vertical([Constraint::Percentage(80), Constraint::Percentage(20)])
                .split(f.area());

            draw_detail_view(f, chunks[0], issue, browser.scroll_offset, image_count);
            draw_confirmation(f, chunks[1], &format!("Close issue #{}? (y/n)", issue.number));
        }
        TuiView::ConfirmReopen { issue } => {
            let chunks = Layout::vertical([Constraint::Percentage(80), Constraint::Percentage(20)])
                .split(f.area());

            draw_detail_view(f, chunks[0], issue, browser.scroll_offset, image_count);
            draw_confirmation(f, chunks[1], &format!("Reopen issue #{}? (y/n)", issue.number));
        }
        TuiView::AssignUser {
            issue,
            input,
            suggestions,
            selected,
        } => {
            let chunks = Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(f.area());

            draw_detail_view(f, chunks[0], issue, browser.scroll_offset, image_count);
            draw_assignee_picker(f, chunks[1], issue, input, suggestions, *selected);
        }
        TuiView::ConfirmDispatch { issue } => {
            let chunks = Layout::vertical([Constraint::Percentage(80), Constraint::Percentage(20)])
                .split(f.area());

            draw_detail_view(f, chunks[0], issue, browser.scroll_offset, image_count);
            draw_confirmation(
                f,
                chunks[1],
                &format!("Dispatch #{} to Claude Code? (y/n)", issue.number),
            );
        }
        TuiView::AgentLogs {
            session_id,
            content,
            scroll,
        } => {
            draw_agent_logs(f, session_id, content, *scroll);
        }
        TuiView::AgentDiff {
            session_id,
            content,
            scroll,
        } => {
            draw_agent_diff(f, session_id, content, *scroll);
        }
        TuiView::EmbeddedTmux {
            available_sessions,
            current_index,
        } => {
            draw_embedded_tmux(f, browser, available_sessions, *current_index);
        }
        TuiView::ProjectSelect { projects, selected } => {
            draw_project_select_inline(f, projects, *selected);
        }
        TuiView::Command {
            input,
            suggestions,
            selected,
        } => {
            let input_clone = input.clone();
            let suggestions_clone = suggestions.clone();
            draw_command_palette(f, browser, &input_clone, &suggestions_clone, *selected);
        }
        TuiView::CreateIssue { input, stage } => {
            let input_clone = input.clone();
            let stage_clone = stage.clone();
            draw_create_issue(f, &input_clone, &stage_clone);
        }
        TuiView::PreviewIssue {
            issue,
            feedback_input,
            scroll,
            ..
        } => {
            let issue_clone = issue.clone();
            let feedback_clone = feedback_input.clone();
            draw_preview_issue(f, &issue_clone, &feedback_clone, *scroll);
        }
        TuiView::DirectIssue {
            title,
            body,
            editing_body,
        } => {
            draw_direct_issue(f, title, body, *editing_body, browser.status_message.as_deref());
        }
    }
}

fn draw_list_view(f: &mut Frame, browser: &mut IssueBrowser) {
    use crate::agents::AgentStatus;

    let items: Vec<ListItem> = browser
        .issues
        .iter()
        .map(|issue| {
            let is_selected = browser.selected_issues.contains(&issue.number);
            let select_marker = if is_selected { "[x] " } else { "[ ] " };

            // Check session status for this issue
            let session_info = browser.session_cache.get(&issue.number);
            let (session_icon, session_color, session_stats) = match session_info {
                Some(session) => {
                    let (icon, color) = match &session.status {
                        AgentStatus::Running => ("▶", Color::Yellow),
                        AgentStatus::Awaiting => ("⏸", Color::Cyan),
                        AgentStatus::Completed { .. } => ("✓", Color::Green),
                        AgentStatus::Failed { .. } => ("✗", Color::Red),
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
                // Closed issues are shown in gray with strikethrough effect
                Line::from(vec![
                    Span::styled(
                        select_marker,
                        if is_selected { Style::default().fg(Color::Green) } else { Style::default().fg(Color::DarkGray) },
                    ),
                    Span::styled(
                        format!("#{:<5}", issue.number),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        " ✓ ",
                        Style::default().fg(Color::Green),
                    ),
                    Span::styled(
                        &issue.title,
                        Style::default().fg(Color::DarkGray).add_modifier(Modifier::CROSSED_OUT),
                    ),
                    Span::styled(labels_str, Style::default().fg(Color::DarkGray)),
                    Span::styled(assignees_str, Style::default().fg(Color::DarkGray)),
                ])
            } else {
                // Build session status span
                let session_span = if let Some(icon) = session_icon {
                    Span::styled(
                        format!("{}{} ", icon, session_stats),
                        Style::default().fg(session_color),
                    )
                } else {
                    Span::raw("   ")
                };

                Line::from(vec![
                    Span::styled(
                        select_marker,
                        if is_selected { Style::default().fg(Color::Green) } else { Style::default().fg(Color::DarkGray) },
                    ),
                    Span::styled(
                        format!("#{:<5}", issue.number),
                        Style::default().fg(Color::Cyan),
                    ),
                    session_span,
                    Span::raw(&issue.title),
                    Span::styled(labels_str, Style::default().fg(Color::DarkGray)),
                    Span::styled(assignees_str, Style::default().fg(Color::Magenta)),
                ])
            };
            ListItem::new(line)
        })
        .collect();

    let title = {
        let mut parts = Vec::new();
        parts.push("Issues".to_string());

        if let Some(ref query) = browser.search_query {
            parts.push(format!("(filtered: '{}')", query));
        }

        if browser.has_next_page {
            parts.push(format!("[{} loaded, more available]", browser.all_issues.len()));
        } else if browser.all_issues.len() > 20 {
            parts.push(format!("[{} total]", browser.all_issues.len()));
        }

        if browser.is_loading {
            parts.push("[Loading...]".to_string());
        }

        if !browser.selected_issues.is_empty() {
            parts.push(format!("[{} selected]", browser.selected_issues.len()));
        }

        format!(" {} │ C ai │ N new │ R refresh │ d dispatch │ t tmux │ s search │ / cmd │ q quit ", parts.join(" "))
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, f.area(), &mut browser.list_state);
}

fn draw_list_view_in_area(f: &mut Frame, browser: &mut IssueBrowser, area: Rect) {
    let items: Vec<ListItem> = browser
        .issues
        .iter()
        .map(|issue| {
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
                Line::from(vec![
                    Span::styled(
                        format!("#{:<5}", issue.number),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(" ✓ ", Style::default().fg(Color::Green)),
                    Span::styled(
                        &issue.title,
                        Style::default().fg(Color::DarkGray).add_modifier(Modifier::CROSSED_OUT),
                    ),
                    Span::styled(labels_str, Style::default().fg(Color::DarkGray)),
                    Span::styled(assignees_str, Style::default().fg(Color::DarkGray)),
                ])
            } else {
                Line::from(vec![
                    Span::styled(
                        format!("#{:<5}", issue.number),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::raw("   "),
                    Span::raw(&issue.title),
                    Span::styled(labels_str, Style::default().fg(Color::DarkGray)),
                    Span::styled(assignees_str, Style::default().fg(Color::Magenta)),
                ])
            };
            ListItem::new(line)
        })
        .collect();

    let title = {
        let mut parts = Vec::new();
        parts.push("Issues".to_string());

        if let Some(ref query) = browser.search_query {
            parts.push(format!("(filtered: '{}')", query));
        }

        if browser.has_next_page {
            parts.push(format!("[{} loaded, more available]", browser.all_issues.len()));
        } else if browser.all_issues.len() > 20 {
            parts.push(format!("[{} total]", browser.all_issues.len()));
        }

        if browser.is_loading {
            parts.push("[Loading...]".to_string());
        }

        if !browser.selected_issues.is_empty() {
            parts.push(format!("[{} selected]", browser.selected_issues.len()));
        }

        format!(" {} │ C ai │ N new │ R refresh │ d dispatch │ t tmux │ s search │ / cmd │ q quit ", parts.join(" "))
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, area, &mut browser.list_state);
}

fn draw_search_input(f: &mut Frame, area: Rect, input: &str) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Search (Enter confirm, Esc cancel) ")
        .border_style(Style::default().fg(Color::Yellow));

    let text = format!("/{}", input);
    let paragraph = Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
}

fn draw_detail_view(
    f: &mut Frame,
    area: Rect,
    issue: &IssueDetail,
    scroll: u16,
    image_count: usize,
) {
    let assignees_str = if issue.assignees.is_empty() {
        "(none)".to_string()
    } else {
        issue.assignees.join(", ")
    };

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Title: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(&issue.title),
        ]),
        Line::from(vec![
            Span::styled("URL: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(&issue.html_url, Style::default().fg(Color::Blue)),
        ]),
        Line::from(vec![
            Span::styled("Labels: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(issue.labels.join(", ")),
        ]),
        Line::from(vec![
            Span::styled("Assignees: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(assignees_str, Style::default().fg(Color::Magenta)),
        ]),
        Line::from(""),
        Line::styled("─── Body ───", Style::default().fg(Color::Yellow)),
    ];

    if let Some(ref body) = issue.body {
        let parsed_body = parse_markdown_content(body);
        for line in parsed_body.lines() {
            lines.push(render_markdown_line(line));
        }
    } else {
        lines.push(Line::styled(
            "(no description)",
            Style::default().fg(Color::DarkGray),
        ));
    }

    if !issue.comments.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::styled(
            format!("─── Comments ({}) ───", issue.comments.len()),
            Style::default().fg(Color::Yellow),
        ));

        for comment in &issue.comments {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(&comment.author, Style::default().fg(Color::Green)),
                Span::raw(" - "),
                Span::styled(
                    format_date(&comment.created_at),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
            let parsed_comment = parse_markdown_content(&comment.body);
            for line in parsed_comment.lines() {
                lines.push(Line::from(format!("  {}", line)));
            }
        }
    }

    let close_key = if issue.state == "Closed" { "X reopen" } else { "x close" };
    let title = if image_count > 0 {
        format!(
            " #{} │ o open │ c comment │ a assign │ d dispatch │ {} │ i/O image [{}/{}] │ ↑↓ scroll │ Esc ",
            issue.number, close_key, 1, image_count
        )
    } else {
        format!(" #{} │ o open │ c comment │ a assign │ d dispatch │ {} │ ↑↓ scroll │ Esc ", issue.number, close_key)
    };

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));

    f.render_widget(paragraph, area);
}

/// Parse markdown/HTML content for better terminal display
fn parse_markdown_content(content: &str) -> String {
    let mut result = content.to_string();

    // Replace <img> tags with [Image: alt or url]
    let img_regex =
        Regex::new(r#"<img[^>]*(?:alt="([^"]*)")?[^>]*src="([^"]*)"[^>]*/?\s*>"#).unwrap();
    result = img_regex
        .replace_all(&result, |caps: &regex::Captures| {
            let alt = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let src = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            if !alt.is_empty() && alt != "Image" {
                format!("[Image: {}]", alt)
            } else {
                format!("[Image: {}]", src)
            }
        })
        .to_string();

    // Also handle img tags where src comes before alt
    let img_regex2 =
        Regex::new(r#"<img[^>]*src="([^"]*)"[^>]*(?:alt="([^"]*)")?[^>]*/?\s*>"#).unwrap();
    result = img_regex2
        .replace_all(&result, |caps: &regex::Captures| {
            let src = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let alt = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            if !alt.is_empty() && alt != "Image" {
                format!("[Image: {}]", alt)
            } else {
                format!("[Image: {}]", src)
            }
        })
        .to_string();

    result
}

/// Render a markdown line with basic styling
fn render_markdown_line(line: &str) -> Line<'static> {
    let trimmed = line.trim();

    // Headers - differentiated by style
    if let Some(h3_content) = trimmed.strip_prefix("### ") {
        // H3: smaller, gray-cyan
        return Line::styled(
            format!("   {}", h3_content),
            Style::default().fg(Color::DarkGray),
        );
    }
    if let Some(h2_content) = trimmed.strip_prefix("## ") {
        // H2: cyan bold
        return Line::styled(
            format!("▸ {}", h2_content),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    }
    if let Some(h1_content) = trimmed.strip_prefix("# ") {
        // H1: uppercase, bright cyan, with underline effect
        return Line::styled(
            format!("═ {} ═", h1_content.to_uppercase()),
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        );
    }

    // Code blocks
    if trimmed.starts_with("```") {
        return Line::styled(
            "─────────────────────".to_string(),
            Style::default().fg(Color::DarkGray),
        );
    }

    // Bullet points
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        let content = &trimmed[2..];
        return Line::from(vec![
            Span::styled("  • ", Style::default().fg(Color::Yellow)),
            Span::raw(render_inline_markdown(content)),
        ]);
    }

    // Numbered lists
    if trimmed.len() > 2
        && trimmed
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
        && let Some(dot_pos) = trimmed.find(". ") {
            let num = &trimmed[..dot_pos];
            let content = &trimmed[dot_pos + 2..];
            return Line::from(vec![
                Span::styled(format!("  {}. ", num), Style::default().fg(Color::Yellow)),
                Span::raw(render_inline_markdown(content)),
            ]);
        }

    // [Image: ...] markers
    if trimmed.starts_with("[Image:") {
        return Line::styled(line.to_string(), Style::default().fg(Color::Magenta));
    }

    // Regular line with inline markdown (bold)
    render_line_with_bold(line)
}

/// Render inline markdown (remove ** but we can't really bold inline in ratatui easily)
fn render_inline_markdown(text: &str) -> String {
    // Remove ** markers for bold - terminal will show clean text
    let bold_regex = Regex::new(r"\*\*([^*]+)\*\*").unwrap();
    bold_regex.replace_all(text, "$1").to_string()
}

/// Render a line handling **bold** sections
fn render_line_with_bold(line: &str) -> Line<'static> {
    let bold_regex = Regex::new(r"\*\*([^*]+)\*\*").unwrap();

    // Check if line contains bold markers
    if !line.contains("**") {
        return Line::from(line.to_string());
    }

    let mut spans = Vec::new();
    let mut last_end = 0;

    for cap in bold_regex.captures_iter(line) {
        let full_match = cap.get(0).unwrap();
        let bold_text = cap.get(1).unwrap();

        // Add text before the bold part
        if full_match.start() > last_end {
            spans.push(Span::raw(line[last_end..full_match.start()].to_string()));
        }

        // Add bold text
        spans.push(Span::styled(
            bold_text.as_str().to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        ));

        last_end = full_match.end();
    }

    // Add remaining text
    if last_end < line.len() {
        spans.push(Span::raw(line[last_end..].to_string()));
    }

    Line::from(spans)
}

fn draw_comment_input(f: &mut Frame, area: Rect, input: &str, status: Option<&str>) {
    let title = if let Some(msg) = status {
        format!(" {} ", msg)
    } else {
        " Add Comment (Enter send, Esc cancel) ".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::Cyan));

    let display_text = if input.is_empty() {
        "Type your comment here..."
    } else {
        input
    };

    let style = if input.is_empty() {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
    };

    let paragraph = Paragraph::new(display_text)
        .block(block)
        .style(style)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn draw_confirmation(f: &mut Frame, area: Rect, message: &str) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let paragraph = Paragraph::new(message)
        .block(block)
        .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .alignment(ratatui::layout::Alignment::Center);

    f.render_widget(paragraph, area);
}

fn draw_status_bar(f: &mut Frame, area: Rect, message: &str) {
    let color = if message.contains("Failed") || message.contains("No ") || message.contains("error") {
        Color::Red
    } else {
        Color::Green
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color));

    let paragraph = Paragraph::new(message)
        .block(block)
        .style(Style::default().fg(color))
        .alignment(ratatui::layout::Alignment::Center);

    f.render_widget(paragraph, area);
}

fn draw_assignee_picker(
    f: &mut Frame,
    area: Rect,
    issue: &IssueDetail,
    input: &str,
    suggestions: &[String],
    selected: usize,
) {
    // Split into: current assignees (top), input field (middle), suggestions (bottom)
    let chunks = Layout::vertical([
        Constraint::Length(3), // Current assignees
        Constraint::Length(3), // Input field
        Constraint::Min(3),    // Suggestions list
    ])
    .split(area);

    // Current assignees
    let assignees_text = if issue.assignees.is_empty() {
        "No assignees".to_string()
    } else {
        issue.assignees.join(", ")
    };
    let assignees_block = Block::default()
        .borders(Borders::ALL)
        .title(" Current Assignees (- to unassign) ");
    let assignees_paragraph = Paragraph::new(assignees_text)
        .block(assignees_block)
        .style(Style::default().fg(Color::Cyan));
    f.render_widget(assignees_paragraph, chunks[0]);

    // Input field
    let input_block = Block::default()
        .borders(Borders::ALL)
        .title(" Type to search (Enter assign, Esc cancel) ")
        .border_style(Style::default().fg(Color::Yellow));
    let input_text = format!("@{}", input);
    let input_paragraph = Paragraph::new(input_text)
        .block(input_block)
        .style(Style::default().fg(Color::White));
    f.render_widget(input_paragraph, chunks[1]);

    // Suggestions list
    let items: Vec<ListItem> = suggestions
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let style = if i == selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            // Mark already assigned users
            let prefix = if issue.assignees.contains(name) {
                "✓ "
            } else {
                "  "
            };
            ListItem::new(Line::from(format!("{}{}", prefix, name))).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Suggestions "))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(list, chunks[2]);
}

fn draw_agent_logs(f: &mut Frame, session_id: &str, content: &str, scroll: u16) {
    let lines: Vec<Line> = content
        .lines()
        .map(|line| Line::from(line.to_string()))
        .collect();

    let title = format!(" Agent {} │ ↑↓ scroll │ q back ", &session_id[..8]);

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));

    f.render_widget(paragraph, f.area());
}

fn draw_agent_diff(f: &mut Frame, session_id: &str, content: &str, scroll: u16) {
    // Color diff lines: green for additions, red for deletions
    let lines: Vec<Line> = content
        .lines()
        .map(|line| {
            let style = if line.starts_with('+') && !line.starts_with("+++") {
                Style::default().fg(Color::Green)
            } else if line.starts_with('-') && !line.starts_with("---") {
                Style::default().fg(Color::Red)
            } else if line.starts_with("@@") {
                Style::default().fg(Color::Cyan)
            } else if line.starts_with("diff ") || line.starts_with("index ") {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };
            Line::from(Span::styled(line.to_string(), style))
        })
        .collect();

    let title = format!(" Agent {} Diff │ ↑↓ scroll │ q back ", &session_id[..8]);

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(Color::Green)),
        )
        .scroll((scroll, 0));

    f.render_widget(paragraph, f.area());
}

fn draw_embedded_tmux(
    f: &mut Frame,
    browser: &IssueBrowser,
    available_sessions: &[String],
    current_index: usize,
) {
    let area = f.area();

    // Header showing session info and controls
    let header_text = if available_sessions.is_empty() {
        " No tmux session │ ESC to exit ".to_string()
    } else {
        let session_name = &available_sessions[current_index];
        format!(
            " {} │ ←→ switch ({}/{}) │ ESC exit ",
            session_name,
            current_index + 1,
            available_sessions.len()
        )
    };

    // Split: header at top, terminal content below
    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(area);

    // Draw header
    let header = Paragraph::new(header_text)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(header, chunks[0]);

    // Draw terminal content
    if let Some(ref term) = browser.embedded_term {
        let screen = term.get_screen();
        let mut lines: Vec<Line> = Vec::new();

        for row in screen {
            let mut spans: Vec<Span> = Vec::new();
            for cell in row {
                let mut style = Style::default().fg(cell.fg).bg(cell.bg);
                if cell.bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                if cell.underline {
                    style = style.add_modifier(Modifier::UNDERLINED);
                }
                if cell.inverse {
                    style = style.add_modifier(Modifier::REVERSED);
                }
                let content = if cell.content.is_empty() {
                    " ".to_string()
                } else {
                    cell.content
                };
                spans.push(Span::styled(content, style));
            }
            lines.push(Line::from(spans));
        }

        let text = Text::from(lines);
        let paragraph = Paragraph::new(text);
        f.render_widget(paragraph, chunks[1]);
    } else {
        // No terminal - show placeholder
        let placeholder = Paragraph::new("Starting terminal...")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(placeholder, chunks[1]);
    }
}

/// Draw project selection inline (within issue browser)
fn draw_project_select_inline(f: &mut Frame, projects: &[String], selected: usize) {
    let area = f.area();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Select Project ")
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let items: Vec<ListItem> = projects
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let style = if i == selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let prefix = if i == selected { "> " } else { "  " };
            ListItem::new(Line::from(format!("{}{}", prefix, name))).style(style)
        })
        .collect();

    let list = List::new(items);

    let chunks = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(inner);

    f.render_widget(list, chunks[0]);

    let help = Paragraph::new("↑↓ navigate │ Enter select │ Esc cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(help, chunks[1]);
}

/// Draw command palette
fn draw_command_palette(
    f: &mut Frame,
    browser: &IssueBrowser,
    input: &str,
    suggestions: &[CommandSuggestion],
    selected: usize,
) {
    let area = f.area();

    // Split: list on top (60%), command input at bottom (40%)
    let chunks = Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Draw issues list in background (dimmed)
    draw_list_view_in_area_dimmed(f, browser, chunks[0]);

    // Command palette panel
    let cmd_area = chunks[1];
    let title = if suggestions.is_empty() {
        " Commands (no match) │ Esc cancel "
    } else {
        " Commands │ ↑↓ navigate │ Enter execute │ Esc cancel "
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(cmd_area);
    f.render_widget(block, cmd_area);

    // Split inner: input field + suggestions
    let inner_chunks =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(1)])
            .split(inner);

    // Input field with cursor
    let input_text = format!("/{}_", input);
    let input_para = Paragraph::new(input_text)
        .style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
    f.render_widget(input_para, inner_chunks[0]);

    // Separator
    let sep = Paragraph::new("─".repeat(inner_chunks[1].width as usize))
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(sep, inner_chunks[1]);

    // Suggestions list
    let items: Vec<ListItem> = suggestions
        .iter()
        .enumerate()
        .map(|(i, cmd)| {
            let is_selected = i == selected;
            let prefix = if is_selected { "▸ " } else { "  " };
            let style = if is_selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            let line = Line::from(vec![
                Span::raw(prefix),
                Span::styled(
                    format!("/{}", cmd.name),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(&cmd.description, Style::default().fg(Color::White)),
            ]);
            ListItem::new(line).style(style)
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, inner_chunks[2]);
}

/// Draw list view but dimmed (for overlay effect)
fn draw_list_view_in_area_dimmed(f: &mut Frame, browser: &IssueBrowser, area: Rect) {
    let items: Vec<ListItem> = browser
        .issues
        .iter()
        .map(|issue| {
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
                Span::styled(&issue.title, Style::default().fg(Color::DarkGray)),
                Span::styled(labels_str, Style::default().fg(Color::DarkGray)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Issues ")
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    f.render_widget(list, area);
}

/// Draw issue creation screen
fn draw_create_issue(f: &mut Frame, input: &str, stage: &CreateStage) {
    let area = f.area();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Create Issue ")
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    match stage {
        CreateStage::Description => {
            let chunks =
                Layout::vertical([Constraint::Length(2), Constraint::Min(3), Constraint::Length(2)])
                    .split(inner);

            let prompt = Paragraph::new("Describe the issue:")
                .style(Style::default().fg(Color::White));
            f.render_widget(prompt, chunks[0]);

            let input_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow));
            let input_text = if input.is_empty() {
                "Type your issue description here..."
            } else {
                input
            };
            let input_style = if input.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            };
            let input_para = Paragraph::new(input_text)
                .block(input_block)
                .style(input_style)
                .wrap(Wrap { trim: false });
            f.render_widget(input_para, chunks[1]);

            let help = Paragraph::new("Enter: generate │ Esc: cancel")
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center);
            f.render_widget(help, chunks[2]);
        }
        CreateStage::Generating => {
            let text = Text::from(vec![
                Line::from(""),
                Line::styled(
                    "Generating issue...",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Line::from(""),
                Line::styled("Please wait...", Style::default().fg(Color::DarkGray)),
            ]);
            let para = Paragraph::new(text).alignment(Alignment::Center);
            f.render_widget(para, inner);
        }
    }
}

/// Draw issue preview screen
fn draw_preview_issue(f: &mut Frame, issue: &IssueContent, feedback_input: &str, scroll: u16) {
    let area = f.area();

    // Split: preview on top (75%), feedback input at bottom (25%)
    let chunks = Layout::vertical([Constraint::Percentage(75), Constraint::Percentage(25)])
        .split(area);

    // Issue preview
    let preview_block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Preview: {} ", issue.title))
        .border_style(Style::default().fg(Color::Green));

    let preview_inner = preview_block.inner(chunks[0]);
    f.render_widget(preview_block, chunks[0]);

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Type: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(&issue.type_, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("Labels: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(issue.labels.join(", ")),
        ]),
        Line::from(""),
        Line::styled("─── Body ───", Style::default().fg(Color::Yellow)),
    ];

    for line in issue.body.lines() {
        lines.push(render_markdown_line(line));
    }

    let text = Text::from(lines);
    let preview_para = Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    f.render_widget(preview_para, preview_inner);

    // Feedback input
    let feedback_block = Block::default()
        .borders(Borders::ALL)
        .title(" Type feedback to refine, Enter to create, Esc to cancel ")
        .border_style(Style::default().fg(Color::Yellow));

    let feedback_text = if feedback_input.is_empty() {
        "Type feedback here or press Enter to create the issue..."
    } else {
        feedback_input
    };
    let feedback_style = if feedback_input.is_empty() {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };

    let feedback_para = Paragraph::new(feedback_text)
        .block(feedback_block)
        .style(feedback_style)
        .wrap(Wrap { trim: false });
    f.render_widget(feedback_para, chunks[1]);
}

/// Draw direct issue creation screen (no AI)
fn draw_direct_issue(f: &mut Frame, title: &str, body: &str, editing_body: bool, status_message: Option<&str>) {
    let area = f.area();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" New Issue (direct) ")
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Split: title field, body field, status, help
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(5),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);

    // Title field
    let title_style = if !editing_body {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let title_block = Block::default()
        .borders(Borders::ALL)
        .title(if !editing_body { " Title (editing) " } else { " Title " })
        .border_style(title_style);
    let title_text = if title.is_empty() && !editing_body {
        "Enter issue title...".to_string()
    } else if title.is_empty() {
        String::new()
    } else {
        title.to_string()
    };
    let title_para = Paragraph::new(if !editing_body {
        format!("{}_", title_text)
    } else {
        title_text
    })
    .block(title_block)
    .style(if title.is_empty() && !editing_body {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    });
    f.render_widget(title_para, chunks[0]);

    // Body field
    let body_style = if editing_body {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let body_block = Block::default()
        .borders(Borders::ALL)
        .title(if editing_body { " Body (editing) " } else { " Body " })
        .border_style(body_style);
    let body_text = if body.is_empty() && editing_body {
        "Enter issue body (markdown supported)...".to_string()
    } else if body.is_empty() {
        String::new()
    } else {
        body.to_string()
    };
    let body_para = Paragraph::new(if editing_body {
        format!("{}_", body_text)
    } else {
        body_text
    })
    .block(body_block)
    .style(if body.is_empty() && editing_body {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    })
    .wrap(Wrap { trim: false });
    f.render_widget(body_para, chunks[1]);

    // Status message
    if let Some(msg) = status_message {
        let status = Paragraph::new(msg)
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);
        f.render_widget(status, chunks[2]);
    }

    // Help
    let help_text = if editing_body {
        "Enter: newline │ Tab: title │ Shift+Enter/Ctrl+S: create │ Esc: cancel"
    } else {
        "Enter: body │ Tab: body │ Shift+Enter/Ctrl+S: create │ Esc: cancel"
    };
    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(help, chunks[3]);
}

fn format_date(date_str: &str) -> String {
    // Simple date formatting: take first 10 chars if available
    if date_str.len() >= 10 {
        date_str[..10].to_string()
    } else {
        date_str.to_string()
    }
}

/// Attach to a tmux session, temporarily exiting the TUI
#[allow(dead_code)]
fn attach_to_tmux_session(session_name: &str) -> io::Result<()> {
    // Exit raw mode and alternate screen
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;

    // Run tmux attach interactively
    let status = std::process::Command::new("tmux")
        .args(["attach", "-t", session_name])
        .status()?;

    // Re-enter alternate screen and raw mode
    execute!(io::stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;

    if !status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "tmux attach failed",
        ));
    }

    Ok(())
}

async fn handle_key_event(browser: &mut IssueBrowser, key: KeyCode, modifiers: KeyModifiers) {
    // Clear status message on any keypress (except ESC for double-ESC logic)
    if key != KeyCode::Esc {
        browser.status_message = None;
        browser.last_esc_press = None; // Reset ESC state on other keys
    }

    match &mut browser.view {
        TuiView::List => match key {
            KeyCode::Esc => {
                // Double-ESC to quit
                if let Some(last_press) = browser.last_esc_press {
                    if last_press.elapsed() < std::time::Duration::from_secs(2) {
                        browser.should_quit = true;
                        return;
                    }
                }
                browser.last_esc_press = Some(std::time::Instant::now());
                browser.status_message = Some("Press ESC again to quit".to_string());
            }
            KeyCode::Char('q') => browser.should_quit = true,
            KeyCode::Down | KeyCode::Char('j') => {
                browser.next();
                // Check if we need to load more issues
                if let Some(selected) = browser.list_state.selected()
                    && browser.has_next_page && selected >= browser.issues.len().saturating_sub(5) {
                        browser.load_next_page().await;
                    }
            }
            KeyCode::Up | KeyCode::Char('k') => browser.previous(),
            KeyCode::Enter => {
                if let Some(issue) = browser.selected_issue() {
                    let number = issue.number;
                    if let Ok(detail) = browser.github.get_issue(number).await {
                        browser.extract_images_from_issue(&detail);
                        browser.view = TuiView::Detail(detail);
                        browser.scroll_offset = 0;
                    }
                }
            }
            KeyCode::Char('s') => {
                // Enter search mode
                browser.view = TuiView::Search { input: String::new() };
            }
            KeyCode::Char('/') => {
                // Open command palette
                let suggestions = browser.available_commands.clone();
                browser.view = TuiView::Command {
                    input: String::new(),
                    suggestions,
                    selected: 0,
                };
            }
            KeyCode::Char('c') => {
                // Clear search filter
                browser.clear_search();
                browser.status_message = Some("Filter cleared".to_string());
            }
            KeyCode::Char(' ') => {
                // Toggle selection of current issue
                if let Some(issue) = browser.selected_issue() {
                    let number = issue.number;
                    if browser.selected_issues.contains(&number) {
                        browser.selected_issues.remove(&number);
                    } else {
                        browser.selected_issues.insert(number);
                    }
                }
            }
            KeyCode::Char('d') => {
                // Dispatch to Claude Code
                if browser.local_path.is_none() {
                    browser.status_message = Some("No local_path configured for this project.".to_string());
                } else if browser.selected_issues.is_empty() {
                    // No selection: dispatch currently highlighted issue
                    if let Some(issue) = browser.selected_issue() {
                        let issue_number = issue.number;
                        let project_name = browser.project_name.clone().unwrap_or_default();
                        let local_path = browser.local_path.clone().unwrap();

                        // Check if a session already exists for this issue
                        let tmux_name = crate::agents::tmux_session_name(&project_name, issue_number);
                        if crate::agents::is_tmux_session_running(&tmux_name) {
                            browser.status_message = Some(format!("Session already running for #{}. Use 't' to open tmux or 'K' to kill it.", issue_number));
                        } else if let Ok(detail) = browser.github.get_issue(issue_number).await {
                            match crate::agents::dispatch_to_claude(&detail, &local_path, &project_name).await {
                                Ok(_) => {
                                    browser.status_message = Some(format!("Dispatched #{} to Claude Code.", issue_number));
                                }
                                Err(e) => {
                                    browser.status_message = Some(format!("Failed to dispatch: {}", e));
                                }
                            }
                        }
                        // Refresh session cache
                        if let Some(project) = browser.project_name.clone() {
                            browser.refresh_sessions(&project);
                        }
                    }
                } else {
                    // Dispatch all selected issues (skip those with existing sessions)
                    let project_name = browser.project_name.clone().unwrap_or_default();
                    let local_path = browser.local_path.clone().unwrap();
                    let mut dispatched = 0;
                    let mut skipped = 0;

                    for issue_number in browser.selected_issues.iter() {
                        let tmux_name = crate::agents::tmux_session_name(&project_name, *issue_number);
                        if crate::agents::is_tmux_session_running(&tmux_name) {
                            skipped += 1;
                            continue;
                        }
                        if let Ok(detail) = browser.github.get_issue(*issue_number).await {
                            if crate::agents::dispatch_to_claude(&detail, &local_path, &project_name).await.is_ok() {
                                dispatched += 1;
                            }
                        }
                    }

                    if skipped > 0 {
                        browser.status_message = Some(format!("Dispatched {} issues ({} skipped, already running).", dispatched, skipped));
                    } else {
                        browser.status_message = Some(format!("Dispatched {} issues to Claude Code.", dispatched));
                    }
                    browser.selected_issues.clear();
                    // Refresh session cache
                    if let Some(project) = browser.project_name.clone() {
                        browser.refresh_sessions(&project);
                    }
                }
            }
            KeyCode::Char('t') => {
                // Open embedded tmux view for current issue
                if let Some(issue) = browser.selected_issue() {
                    let issue_number = issue.number;
                    if let Some(project) = browser.project_name.clone() {
                        let tmux_name = crate::agents::tmux_session_name(&project, issue_number);
                        if crate::agents::is_tmux_session_running(&tmux_name) {
                            // Get all running tmux sessions for this project
                            let all_sessions = crate::agents::list_tmux_sessions();
                            let current_idx = all_sessions.iter().position(|s| s == &tmux_name).unwrap_or(0);

                            // Create embedded terminal
                            let area = crossterm::terminal::size().unwrap_or((80, 24));
                            match crate::embedded_term::EmbeddedTerminal::new(
                                &tmux_name,
                                area.1.saturating_sub(1),
                                area.0,
                            ) {
                                Ok(term) => {
                                    browser.embedded_term = Some(term);
                                    browser.view = TuiView::EmbeddedTmux {
                                        available_sessions: all_sessions,
                                        current_index: current_idx,
                                    };
                                }
                                Err(e) => {
                                    browser.status_message = Some(format!("Failed to open terminal: {}", e));
                                }
                            }
                        } else {
                            browser.status_message = Some("No active session for this issue".to_string());
                        }
                    } else {
                        browser.status_message = Some("No project selected".to_string());
                    }
                }
            }
            KeyCode::Char('T') => {
                // Open embedded tmux with all sessions (capital T)
                let all_sessions = crate::agents::list_tmux_sessions();
                if !all_sessions.is_empty() {
                    let area = crossterm::terminal::size().unwrap_or((80, 24));
                    match crate::embedded_term::EmbeddedTerminal::new(
                        &all_sessions[0],
                        area.1.saturating_sub(1),
                        area.0,
                    ) {
                        Ok(term) => {
                            browser.embedded_term = Some(term);
                            browser.view = TuiView::EmbeddedTmux {
                                available_sessions: all_sessions,
                                current_index: 0,
                            };
                        }
                        Err(e) => {
                            browser.status_message = Some(format!("Failed to open terminal: {}", e));
                        }
                    }
                } else {
                    browser.status_message = Some("No tmux sessions available".to_string());
                }
            }
            KeyCode::Char('l') => {
                // View logs for agent of current issue
                if let Some(issue) = browser.selected_issue() {
                    if let Some(session) = browser.session_cache.get(&issue.number) {
                        let content = std::fs::read_to_string(&session.log_file).unwrap_or_default();
                        browser.view = TuiView::AgentLogs {
                            session_id: session.id.clone(),
                            content,
                            scroll: 0,
                        };
                    } else {
                        browser.status_message = Some("No agent session for this issue".to_string());
                    }
                }
            }
            KeyCode::Char('D') => {
                // View diff for agent of current issue
                if let Some(issue) = browser.selected_issue() {
                    if let Some(session) = browser.session_cache.get(&issue.number) {
                        let output = std::process::Command::new("git")
                            .current_dir(&session.worktree_path)
                            .args(["diff", "HEAD"])
                            .output();

                        let content = match output {
                            Ok(out) if out.status.success() => {
                                String::from_utf8_lossy(&out.stdout).to_string()
                            }
                            _ => "No changes or failed to get diff".to_string(),
                        };

                        browser.view = TuiView::AgentDiff {
                            session_id: session.id.clone(),
                            content,
                            scroll: 0,
                        };
                    } else {
                        browser.status_message = Some("No agent session for this issue".to_string());
                    }
                }
            }
            KeyCode::Char('p') => {
                // Create PR from agent of current issue
                if let Some(issue) = browser.selected_issue() {
                    let issue_number = issue.number;
                    if let Some(session) = browser.session_cache.get(&issue_number) {
                        if session.is_running() {
                            browser.status_message = Some("Agent is still running".to_string());
                        } else if session.pr_url.is_some() {
                            browser.status_message = Some("PR already created".to_string());
                        } else {
                            match crate::agents::create_pr(session) {
                                Ok(url) => {
                                    browser.status_message = Some(format!("PR created: {}", url));
                                }
                                Err(e) => {
                                    browser.status_message = Some(format!("Failed to create PR: {}", e));
                                }
                            }
                        }
                    } else {
                        browser.status_message = Some("No agent session for this issue".to_string());
                    }
                    // Refresh session cache
                    if let Some(project) = browser.project_name.clone() {
                        browser.refresh_sessions(&project);
                    }
                }
            }
            KeyCode::Char('K') => {
                // Kill agent of current issue
                if let Some(issue) = browser.selected_issue() {
                    let issue_number = issue.number;
                    if let Some(session) = browser.session_cache.get(&issue_number) {
                        if session.is_running() {
                            let _ = crate::agents::kill_agent(&session.id);
                            browser.status_message = Some(format!("Killed agent for #{}", issue_number));
                        } else {
                            browser.status_message = Some("Agent is not running".to_string());
                        }
                    } else {
                        browser.status_message = Some("No agent session for this issue".to_string());
                    }
                    // Refresh session cache
                    if let Some(project) = browser.project_name.clone() {
                        browser.refresh_sessions(&project);
                    }
                }
            }
            KeyCode::Char('W') => {
                // Cleanup worktree for agent of current issue
                if let Some(issue) = browser.selected_issue() {
                    let issue_number = issue.number;
                    if let Some(session) = browser.session_cache.get(&issue_number).cloned() {
                        if session.is_running() {
                            browser.status_message = Some("Agent is still running".to_string());
                        } else if let Some(parent) = session.worktree_path.parent()
                            && let Some(grandparent) = parent.parent()
                        {
                            let _ = crate::agents::remove_worktree(
                                grandparent,
                                &session.worktree_path,
                                true,
                            );
                            browser.status_message =
                                Some(format!("Cleaned up worktree for #{}", issue_number));

                            // Remove session from manager
                            let mut manager = crate::agents::SessionManager::load();
                            manager.remove(&session.id);
                            let _ = manager.save();
                        }
                    } else {
                        browser.status_message = Some("No agent session for this issue".to_string());
                    }
                    // Refresh session cache
                    if let Some(project) = browser.project_name.clone() {
                        browser.refresh_sessions(&project);
                    }
                }
            }
            KeyCode::Char('C') => {
                // Create new issue with AI
                if browser.project_labels.is_empty() {
                    browser.status_message = Some("No project labels configured.".to_string());
                } else {
                    browser.view = TuiView::CreateIssue {
                        input: String::new(),
                        stage: CreateStage::Description,
                    };
                }
            }
            KeyCode::Char('N') => {
                // Create new issue directly (no AI)
                browser.view = TuiView::DirectIssue {
                    title: String::new(),
                    body: String::new(),
                    editing_body: false,
                };
            }
            KeyCode::Char('R') => {
                // Refresh issue list
                browser.status_message = Some("Refreshing...".to_string());
                browser.reload_issues().await;
                browser.status_message = Some("Refreshed".to_string());
            }
            _ => {}
        },
        TuiView::Search { input } => {
            match key {
                KeyCode::Esc => {
                    // Cancel search, return to list
                    browser.view = TuiView::List;
                }
                KeyCode::Enter => {
                    // Apply search filter and return to list
                    browser.view = TuiView::List;
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(c) => {
                    input.push(c);
                }
                _ => {}
            }
            // Apply filter after processing key (unless we're leaving search mode)
            if matches!(browser.view, TuiView::Search { .. })
                && let TuiView::Search { input } = &browser.view {
                    let query = input.clone();
                    browser.apply_search_filter(&query);
                }
        }
        TuiView::Detail(issue) => match key {
            KeyCode::Esc | KeyCode::Char('q') => {
                browser.view = TuiView::List;
                browser.scroll_offset = 0;
                browser.current_images.clear();
            }
            KeyCode::Down | KeyCode::Char('j') => browser.scroll_offset += 1,
            KeyCode::Up | KeyCode::Char('k') => {
                browser.scroll_offset = browser.scroll_offset.saturating_sub(1);
            }
            KeyCode::Char('c') => {
                let issue_clone = issue.clone();
                browser.view = TuiView::AddComment {
                    issue: issue_clone,
                    input: String::new(),
                };
                browser.status_message = None;
            }
            KeyCode::Char('o') => {
                // Open issue in browser
                open_url(&issue.html_url);
                browser.status_message = Some("Opened in browser".to_string());
            }
            KeyCode::Char('O') => {
                // Open current image in browser
                if !browser.current_images.is_empty() {
                    let url = &browser.current_images[browser.current_image_index];
                    open_url(url);
                    browser.status_message = Some("Image opened in browser".to_string());
                    // Cycle to next image
                    browser.current_image_index =
                        (browser.current_image_index + 1) % browser.current_images.len();
                } else {
                    browser.status_message = Some("No images".to_string());
                }
            }
            KeyCode::Char('i') => {
                // Show image in terminal if available
                if !browser.current_images.is_empty() {
                    let url = browser.current_images[browser.current_image_index].clone();
                    let token = browser.github_token.clone();
                    if let Err(e) = display_image(&url, token.as_deref()).await {
                        browser.status_message = Some(format!("Image error: {}", e));
                    }
                    // Cycle to next image for next press
                    browser.current_image_index =
                        (browser.current_image_index + 1) % browser.current_images.len();
                } else {
                    browser.status_message = Some("No images in this issue".to_string());
                }
            }
            KeyCode::Char('x') => {
                // Close issue confirmation (only if open)
                if issue.state == "Open" {
                    let issue_clone = issue.clone();
                    browser.view = TuiView::ConfirmClose { issue: issue_clone };
                } else {
                    browser.status_message = Some("Issue is already closed".to_string());
                }
            }
            KeyCode::Char('X') => {
                // Reopen issue confirmation (only if closed)
                if issue.state == "Closed" {
                    let issue_clone = issue.clone();
                    browser.view = TuiView::ConfirmReopen { issue: issue_clone };
                } else {
                    browser.status_message = Some("Issue is already open".to_string());
                }
            }
            KeyCode::Char('a') => {
                // Open assignee picker - clone issue first to avoid borrow issues
                let issue_clone = issue.clone();
                browser.load_assignees().await;
                let suggestions = browser.get_assignee_suggestions("");
                browser.view = TuiView::AssignUser {
                    issue: issue_clone,
                    input: String::new(),
                    suggestions,
                    selected: 0,
                };
            }
            KeyCode::Char('d') => {
                // Dispatch to Claude Code - check if session already exists
                if let Some(project) = browser.project_name.clone() {
                    let tmux_name = crate::agents::tmux_session_name(&project, issue.number);
                    if crate::agents::is_tmux_session_running(&tmux_name) {
                        browser.status_message = Some(format!(
                            "Session already running for #{}. Use 't' to open tmux or 'K' to kill it.",
                            issue.number
                        ));
                    } else {
                        let issue_clone = issue.clone();
                        browser.view = TuiView::ConfirmDispatch { issue: issue_clone };
                    }
                } else {
                    browser.status_message = Some("No project selected".to_string());
                }
            }
            _ => {}
        },
        TuiView::AddComment { issue, input } => match key {
            KeyCode::Esc => {
                let number = issue.number;
                if let Ok(detail) = browser.github.get_issue(number).await {
                    browser.view = TuiView::Detail(detail);
                } else {
                    browser.view = TuiView::List;
                }
                browser.status_message = None;
            }
            KeyCode::Enter => {
                if !input.is_empty() {
                    let comment_body = if browser.auto_format {
                        browser.status_message = Some("Formatting...".to_string());
                        format_comment_with_llm(input, &browser.llm_endpoint)
                            .await
                            .unwrap_or_else(|_| input.clone())
                    } else {
                        input.clone()
                    };

                    browser.status_message = Some("Sending...".to_string());
                    let number = issue.number;
                    if browser
                        .github
                        .add_comment(number, &comment_body)
                        .await
                        .is_ok()
                    {
                        // Reload issue to show new comment
                        if let Ok(detail) = browser.github.get_issue(number).await {
                            browser.view = TuiView::Detail(detail);
                        } else {
                            browser.view = TuiView::List;
                        }
                    } else {
                        browser.view = TuiView::List;
                    }
                    browser.status_message = None;
                }
            }
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Char(c) => {
                input.push(c);
            }
            _ => {}
        },
        TuiView::ConfirmClose { issue } => match key {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let number = issue.number;
                if browser.github.close_issue(number).await.is_ok() {
                    browser.status_message = Some(format!("Issue #{} closed", number));
                    // Remove from list or update state
                    if let Some(pos) = browser.issues.iter().position(|i| i.number == number) {
                        browser.issues[pos].state = "Closed".to_string();
                    }
                    browser.view = TuiView::List;
                } else {
                    browser.status_message = Some("Failed to close issue".to_string());
                    // Return to detail view
                    if let Ok(detail) = browser.github.get_issue(number).await {
                        browser.view = TuiView::Detail(detail);
                    } else {
                        browser.view = TuiView::List;
                    }
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                let number = issue.number;
                if let Ok(detail) = browser.github.get_issue(number).await {
                    browser.view = TuiView::Detail(detail);
                } else {
                    browser.view = TuiView::List;
                }
            }
            _ => {}
        },
        TuiView::ConfirmReopen { issue } => match key {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let number = issue.number;
                if browser.github.reopen_issue(number).await.is_ok() {
                    browser.status_message = Some(format!("Issue #{} reopened", number));
                    // Update state in list
                    if let Some(pos) = browser.issues.iter().position(|i| i.number == number) {
                        browser.issues[pos].state = "Open".to_string();
                    }
                    // Reload and show updated issue
                    if let Ok(detail) = browser.github.get_issue(number).await {
                        browser.view = TuiView::Detail(detail);
                    } else {
                        browser.view = TuiView::List;
                    }
                } else {
                    browser.status_message = Some("Failed to reopen issue".to_string());
                    if let Ok(detail) = browser.github.get_issue(number).await {
                        browser.view = TuiView::Detail(detail);
                    } else {
                        browser.view = TuiView::List;
                    }
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                let number = issue.number;
                if let Ok(detail) = browser.github.get_issue(number).await {
                    browser.view = TuiView::Detail(detail);
                } else {
                    browser.view = TuiView::List;
                }
            }
            _ => {}
        },
        TuiView::AssignUser {
            issue,
            input,
            suggestions,
            selected,
        } => {
            // Extract data we need before modifying browser
            let number = issue.number;
            let current_assignees = issue.assignees.clone();
            let input_str = input.clone();
            let sel = *selected;

            match key {
                KeyCode::Esc => {
                    if let Ok(detail) = browser.github.get_issue(number).await {
                        browser.view = TuiView::Detail(detail);
                    } else {
                        browser.view = TuiView::List;
                    }
                }
                KeyCode::Up => {
                    if let TuiView::AssignUser { selected, .. } = &mut browser.view
                        && *selected > 0
                    {
                        *selected -= 1;
                    }
                }
                KeyCode::Down => {
                    let sugg_len = suggestions.len();
                    if let TuiView::AssignUser { selected, .. } = &mut browser.view
                        && *selected < sugg_len.saturating_sub(1)
                    {
                        *selected += 1;
                    }
                }
                KeyCode::Enter => {
                    // Assign selected user
                    if let Some(user) = suggestions.get(sel) {
                        let user_to_assign = user.clone();

                        // Check if already assigned
                        if current_assignees.contains(&user_to_assign) {
                            browser.status_message = Some(format!("{} is already assigned", user_to_assign));
                        } else if browser
                            .github
                            .assign_issue(number, std::slice::from_ref(&user_to_assign))
                            .await
                            .is_ok()
                        {
                            browser.status_message = Some(format!("Assigned {} to #{}", user_to_assign, number));
                            // Update the issue in list
                            if let Some(pos) = browser.issues.iter().position(|i| i.number == number) {
                                browser.issues[pos].assignees.push(user_to_assign);
                            }
                        } else {
                            browser.status_message = Some("Failed to assign user".to_string());
                        }

                        // Reload and return to detail view
                        if let Ok(detail) = browser.github.get_issue(number).await {
                            browser.view = TuiView::Detail(detail);
                        } else {
                            browser.view = TuiView::List;
                        }
                    }
                }
                KeyCode::Char('-') => {
                    // Unassign: remove first current assignee
                    if !current_assignees.is_empty() {
                        let user_to_remove = current_assignees[0].clone();

                        if browser
                            .github
                            .unassign_issue(number, std::slice::from_ref(&user_to_remove))
                            .await
                            .is_ok()
                        {
                            browser.status_message = Some(format!("Unassigned {} from #{}", user_to_remove, number));
                            // Update the issue in list
                            if let Some(pos) = browser.issues.iter().position(|i| i.number == number) {
                                browser.issues[pos].assignees.retain(|u| u != &user_to_remove);
                            }
                        } else {
                            browser.status_message = Some("Failed to unassign user".to_string());
                        }

                        // Reload and stay in assign view
                        if let Ok(detail) = browser.github.get_issue(number).await {
                            let new_suggestions = browser.get_assignee_suggestions(&input_str);
                            browser.view = TuiView::AssignUser {
                                issue: detail,
                                input: input_str,
                                suggestions: new_suggestions,
                                selected: 0,
                            };
                        } else {
                            browser.view = TuiView::List;
                        }
                    }
                }
                KeyCode::Backspace => {
                    let mut new_input = input_str.clone();
                    new_input.pop();
                    let new_suggestions = browser.get_assignee_suggestions(&new_input);
                    if let TuiView::AssignUser {
                        input: ref mut inp,
                        suggestions: ref mut sug,
                        selected: ref mut sel,
                        ..
                    } = browser.view
                    {
                        *inp = new_input;
                        *sug = new_suggestions;
                        *sel = 0;
                    }
                }
                KeyCode::Char(c) => {
                    let mut new_input = input_str.clone();
                    new_input.push(c);
                    let new_suggestions = browser.get_assignee_suggestions(&new_input);
                    if let TuiView::AssignUser {
                        input: ref mut inp,
                        suggestions: ref mut sug,
                        selected: ref mut sel,
                        ..
                    } = browser.view
                    {
                        *inp = new_input;
                        *sug = new_suggestions;
                        *sel = 0;
                    }
                }
                _ => {}
            }
        }
        TuiView::ConfirmDispatch { issue } => match key {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let number = issue.number;

                // Check if we have local_path configured
                if let (Some(project), Some(local_path)) =
                    (&browser.project_name, &browser.local_path)
                {
                    // Double-check that no session is already running
                    let tmux_name = crate::agents::tmux_session_name(project, number);
                    if crate::agents::is_tmux_session_running(&tmux_name) {
                        browser.status_message = Some(format!(
                            "Session already running for #{}. Use 't' to open tmux or 'K' to kill it.",
                            number
                        ));
                    } else {
                        match crate::agents::dispatch_to_claude(issue, local_path, project).await {
                            Ok(session) => {
                                browser.status_message = Some(format!(
                                    "Dispatched #{} to Claude Code (session {})",
                                    number,
                                    &session.id[..8]
                                ));
                            }
                            Err(e) => {
                                browser.status_message = Some(format!("Failed to dispatch: {}", e));
                            }
                        }
                    }
                } else {
                    browser.status_message = Some(
                        "No local_path configured for this project. Add it to assistant.json"
                            .to_string(),
                    );
                }

                // Return to detail view
                if let Ok(detail) = browser.github.get_issue(number).await {
                    browser.view = TuiView::Detail(detail);
                } else {
                    browser.view = TuiView::List;
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                let number = issue.number;
                if let Ok(detail) = browser.github.get_issue(number).await {
                    browser.view = TuiView::Detail(detail);
                } else {
                    browser.view = TuiView::List;
                }
            }
            _ => {}
        },
        TuiView::AgentLogs { scroll, .. } => match key {
            KeyCode::Esc | KeyCode::Char('q') => {
                browser.view = TuiView::List;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                *scroll = scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                *scroll += 1;
            }
            KeyCode::PageUp => {
                *scroll = scroll.saturating_sub(20);
            }
            KeyCode::PageDown => {
                *scroll += 20;
            }
            _ => {}
        },
        TuiView::AgentDiff { scroll, .. } => match key {
            KeyCode::Esc | KeyCode::Char('q') => {
                browser.view = TuiView::List;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                *scroll = scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                *scroll += 1;
            }
            KeyCode::PageUp => {
                *scroll = scroll.saturating_sub(20);
            }
            KeyCode::PageDown => {
                *scroll += 20;
            }
            _ => {}
        },
        TuiView::EmbeddedTmux {
            available_sessions,
            current_index,
        } => {
            match key {
                KeyCode::Esc => {
                    // Exit embedded terminal and go back to list
                    browser.embedded_term = None;
                    browser.view = TuiView::List;
                    // Refresh session cache with fresh stats from git
                    if let Some(project) = browser.project_name.clone() {
                        browser.refresh_sessions_with_fresh_stats(&project);
                    }
                }
                KeyCode::Left => {
                    // Switch to previous session
                    if !available_sessions.is_empty() && *current_index > 0 {
                        *current_index -= 1;
                        let session_name = &available_sessions[*current_index];
                        // Create new embedded terminal for this session
                        let area = crossterm::terminal::size().unwrap_or((80, 24));
                        if let Ok(term) = crate::embedded_term::EmbeddedTerminal::new(
                            session_name,
                            area.1.saturating_sub(1), // -1 for header
                            area.0,
                        ) {
                            browser.embedded_term = Some(term);
                        }
                    }
                }
                KeyCode::Right => {
                    // Switch to next session
                    if !available_sessions.is_empty()
                        && *current_index < available_sessions.len() - 1
                    {
                        *current_index += 1;
                        let session_name = &available_sessions[*current_index];
                        // Create new embedded terminal for this session
                        let area = crossterm::terminal::size().unwrap_or((80, 24));
                        if let Ok(term) = crate::embedded_term::EmbeddedTerminal::new(
                            session_name,
                            area.1.saturating_sub(1),
                            area.0,
                        ) {
                            browser.embedded_term = Some(term);
                        }
                    }
                }
                _ => {
                    // Forward all other keys to the embedded terminal
                    if let Some(ref term) = browser.embedded_term {
                        term.send_key(key);
                    }
                }
            }
        }
        TuiView::ProjectSelect { projects, selected } => match key {
            KeyCode::Esc => {
                browser.view = TuiView::List;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if *selected > 0 {
                    *selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if *selected < projects.len().saturating_sub(1) {
                    *selected += 1;
                }
            }
            KeyCode::Enter => {
                // Project selected - this would typically trigger a callback
                // For now, just return to list with a message
                let project_name = projects.get(*selected).cloned();
                browser.view = TuiView::List;
                if let Some(name) = project_name {
                    browser.status_message = Some(format!("Selected project: {}", name));
                }
            }
            _ => {}
        },
        TuiView::Command {
            input,
            suggestions,
            selected,
        } => match key {
            KeyCode::Esc => {
                browser.view = TuiView::List;
            }
            KeyCode::Up => {
                if *selected > 0 {
                    *selected -= 1;
                }
            }
            KeyCode::Down => {
                if *selected < suggestions.len().saturating_sub(1) {
                    *selected += 1;
                }
            }
            KeyCode::Enter => {
                // Execute the selected command
                if let Some(cmd) = suggestions.get(*selected) {
                    let cmd_name = cmd.name.clone();
                    let labels = cmd.labels.clone();
                    browser.view = TuiView::List;

                    // Handle built-in commands
                    match cmd_name.as_str() {
                        "logout" => {
                            let _ = auth::delete_token();
                            browser.status_message = Some("Logged out.".to_string());
                            browser.should_quit = true;
                        }
                        "project" | "repo" => {
                            // Would show project selection - for now just message
                            browser.status_message =
                                Some("Use the main app to switch projects.".to_string());
                        }
                        _ => {
                            // Custom list command - filter by labels
                            if let Some(filter_labels) = labels {
                                browser.list_labels = filter_labels;
                                browser.status_message =
                                    Some(format!("Filter applied: /{}", cmd_name));
                                // Note: Would need to reload issues with new filter
                            }
                        }
                    }
                } else {
                    browser.view = TuiView::List;
                }
            }
            KeyCode::Backspace => {
                input.pop();
                // Filter suggestions based on new input
                let input_clone = input.clone();
                let available = browser.available_commands.clone();
                *suggestions = if input_clone.is_empty() {
                    available
                } else {
                    let input_lower = input_clone.to_lowercase();
                    available
                        .into_iter()
                        .filter(|cmd| cmd.name.to_lowercase().contains(&input_lower))
                        .collect()
                };
                *selected = 0;
            }
            KeyCode::Char(c) => {
                input.push(c);
                // Filter suggestions based on new input
                let input_clone = input.clone();
                let available = browser.available_commands.clone();
                *suggestions = if input_clone.is_empty() {
                    available
                } else {
                    let input_lower = input_clone.to_lowercase();
                    available
                        .into_iter()
                        .filter(|cmd| cmd.name.to_lowercase().contains(&input_lower))
                        .collect()
                };
                *selected = 0;
            }
            _ => {}
        },
        TuiView::CreateIssue { input, stage } => match key {
            KeyCode::Esc => {
                browser.view = TuiView::List;
            }
            KeyCode::Enter => {
                if matches!(stage, CreateStage::Description) && !input.is_empty() {
                    // Start generating
                    let description = input.clone();
                    let labels = browser.project_labels.clone();
                    let endpoint = browser.llm_endpoint.clone();

                    *stage = CreateStage::Generating;

                    // Generate issue asynchronously
                    match crate::issues::generate_issue_with_labels(&description, &labels, &endpoint)
                        .await
                    {
                        Ok((issue, messages)) => {
                            browser.view = TuiView::PreviewIssue {
                                issue,
                                messages,
                                feedback_input: String::new(),
                                scroll: 0,
                            };
                        }
                        Err(e) => {
                            browser.status_message = Some(format!("Generation failed: {}", e));
                            browser.view = TuiView::List;
                        }
                    }
                }
            }
            KeyCode::Backspace => {
                if matches!(stage, CreateStage::Description) {
                    input.pop();
                }
            }
            KeyCode::Char(c) => {
                if matches!(stage, CreateStage::Description) {
                    input.push(c);
                }
            }
            _ => {}
        },
        TuiView::PreviewIssue {
            issue,
            messages,
            feedback_input,
            scroll,
        } => match key {
            KeyCode::Esc => {
                browser.view = TuiView::List;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                *scroll = scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                *scroll += 1;
            }
            KeyCode::Enter => {
                if feedback_input.is_empty() {
                    // Create the issue on GitHub
                    let issue_clone = issue.clone();
                    match browser.github.create_issue(&issue_clone).await {
                        Ok((url, new_issue)) => {
                            browser.status_message = Some(format!("Issue created: {}", url));
                            // Insert new issue at the beginning of the list
                            browser.all_issues.insert(0, new_issue.clone());
                            browser.issues.insert(0, new_issue);
                            browser.list_state.select(Some(0));
                            browser.view = TuiView::List;
                        }
                        Err(e) => {
                            browser.status_message = Some(format!("Failed to create: {}", e));
                        }
                    }
                } else {
                    // Refine the issue with feedback
                    let feedback = feedback_input.clone();
                    let endpoint = browser.llm_endpoint.clone();

                    messages.push(llm::Message {
                        role: "user".to_string(),
                        content: feedback,
                    });

                    match llm::generate_response(messages, &endpoint).await {
                        Ok(response) => {
                            if let Ok(updated_issue) =
                                serde_json::from_str::<IssueContent>(&response.message.content)
                            {
                                messages.push(llm::Message {
                                    role: "assistant".to_string(),
                                    content: serde_json::to_string(&updated_issue)
                                        .unwrap_or_default(),
                                });
                                *issue = updated_issue;
                                *feedback_input = String::new();
                                *scroll = 0;
                            } else {
                                browser.status_message =
                                    Some("Failed to parse updated issue.".to_string());
                            }
                        }
                        Err(e) => {
                            browser.status_message = Some(format!("Refinement failed: {}", e));
                        }
                    }
                }
            }
            KeyCode::Backspace => {
                feedback_input.pop();
            }
            KeyCode::Char(c) => {
                feedback_input.push(c);
            }
            _ => {}
        },
        TuiView::DirectIssue {
            title,
            body,
            editing_body,
        } => {
            match key {
            KeyCode::Esc => {
                browser.view = TuiView::List;
            }
            KeyCode::Tab => {
                // Toggle between title and body editing
                *editing_body = !*editing_body;
            }
            KeyCode::Enter if modifiers.contains(KeyModifiers::SHIFT) => {
                // Shift+Enter: Create issue
                if title.is_empty() {
                    browser.status_message = Some("Title cannot be empty".to_string());
                } else {
                    let issue = IssueContent {
                        type_: "task".to_string(),
                        title: title.clone(),
                        body: body.clone(),
                        labels: Vec::new(),
                    };
                    match browser.github.create_issue(&issue).await {
                        Ok((url, new_issue)) => {
                            browser.status_message = Some(format!("Issue created: {}", url));
                            // Insert new issue at the beginning of the list
                            browser.all_issues.insert(0, new_issue.clone());
                            browser.issues.insert(0, new_issue);
                            browser.list_state.select(Some(0));
                            browser.view = TuiView::List;
                        }
                        Err(e) => {
                            browser.status_message = Some(format!("Failed to create: {}", e));
                        }
                    }
                }
            }
            KeyCode::Char('s') | KeyCode::Char('j') if modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+S or Ctrl+J: Create issue
                // Note: Ghostty sends Ctrl+J for Shift+Enter
                if title.is_empty() {
                    browser.status_message = Some("Title cannot be empty".to_string());
                } else {
                    let issue = IssueContent {
                        type_: "task".to_string(),
                        title: title.clone(),
                        body: body.clone(),
                        labels: Vec::new(),
                    };
                    match browser.github.create_issue(&issue).await {
                        Ok((url, new_issue)) => {
                            browser.status_message = Some(format!("Issue created: {}", url));
                            // Insert new issue at the beginning of the list
                            browser.all_issues.insert(0, new_issue.clone());
                            browser.issues.insert(0, new_issue);
                            browser.list_state.select(Some(0));
                            browser.view = TuiView::List;
                        }
                        Err(e) => {
                            browser.status_message = Some(format!("Failed to create: {}", e));
                        }
                    }
                }
            }
            KeyCode::Enter => {
                if *editing_body {
                    // Enter in body: add newline
                    body.push('\n');
                } else {
                    // Enter in title: move to body
                    *editing_body = true;
                }
            }
            KeyCode::Backspace => {
                if *editing_body {
                    body.pop();
                } else {
                    title.pop();
                }
            }
            KeyCode::Char(c) => {
                if *editing_body {
                    body.push(c);
                } else {
                    title.push(c);
                }
            }
            _ => {}
            }
        }
    }
}

/// Handle pasted content into input fields
fn handle_paste(browser: &mut IssueBrowser, content: &str) {
    // Remove newlines for single-line fields, keep them for body fields
    let clean_content = content.replace('\r', "");

    match &mut browser.view {
        TuiView::Search { input } => {
            input.push_str(&clean_content.replace('\n', " "));
        }
        TuiView::Command { input, suggestions, selected } => {
            input.push_str(&clean_content.replace('\n', " "));
            // Update suggestions
            let input_clone = input.clone();
            let available = browser.available_commands.clone();
            *suggestions = if input_clone.is_empty() {
                available
            } else {
                let input_lower = input_clone.to_lowercase();
                available
                    .into_iter()
                    .filter(|cmd| cmd.name.to_lowercase().contains(&input_lower))
                    .collect()
            };
            *selected = 0;
        }
        TuiView::CreateIssue { input, stage } => {
            if matches!(stage, CreateStage::Description) {
                input.push_str(&clean_content);
            }
        }
        TuiView::PreviewIssue { feedback_input, .. } => {
            feedback_input.push_str(&clean_content);
        }
        TuiView::DirectIssue { title, body, editing_body } => {
            if *editing_body {
                body.push_str(&clean_content);
            } else {
                title.push_str(&clean_content.replace('\n', " "));
            }
        }
        TuiView::AddComment { input, .. } => {
            input.push_str(&clean_content);
        }
        TuiView::AssignUser { input, .. } => {
            input.push_str(&clean_content.replace('\n', " "));
        }
        _ => {}
    }
}

/// Try to get clipboard content (platform-specific)
fn get_clipboard_content() -> Result<String, Box<dyn std::error::Error>> {
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

async fn format_comment_with_llm(
    comment: &str,
    endpoint: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut messages = vec![
        llm::Message {
            role: "system".to_string(),
            content: "You are a writing assistant. Correct grammar, fix typos, and improve clarity of the following comment for a GitHub issue. Keep it concise and professional. Return only the corrected text, no explanations or quotes.".to_string(),
        },
        llm::Message {
            role: "user".to_string(),
            content: comment.to_string(),
        },
    ];

    let response = llm::generate_response(&mut messages, endpoint).await?;
    Ok(response.message.content.trim().to_string())
}

/// Extract image URLs from content (supports HTML img tags and markdown images)
fn extract_image_urls(content: &str) -> Vec<String> {
    let mut urls = Vec::new();

    // HTML img tags: <img src="..." />
    let img_regex = Regex::new(r#"<img[^>]*src="([^"]+)"[^>]*>"#).unwrap();
    for cap in img_regex.captures_iter(content) {
        if let Some(url) = cap.get(1) {
            urls.push(url.as_str().to_string());
        }
    }

    // Markdown images: ![alt](url)
    let md_regex = Regex::new(r"!\[[^\]]*\]\(([^)]+)\)").unwrap();
    for cap in md_regex.captures_iter(content) {
        if let Some(url) = cap.get(1) {
            urls.push(url.as_str().to_string());
        }
    }

    urls
}

/// Display an image in the terminal using ratatui-image
async fn display_image(
    url: &str,
    github_token: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Download the image with timeout
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    // Add GitHub token for private repo images
    let mut request = client.get(url);
    if (url.contains("github.com") || url.contains("githubusercontent.com"))
        && let Some(token) = github_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

    let response = request.send().await?;

    if !response.status().is_success() {
        return Err(format!("Failed to download: HTTP {}", response.status()).into());
    }

    let bytes = response.bytes().await?;

    // Decode the image
    let img = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()?
        .decode()?;

    // Create picker to detect terminal protocol (must be done before entering alternate screen)
    // Temporarily exit raw mode for protocol detection
    disable_raw_mode()?;
    let picker = Picker::from_query_stdio()?;
    enable_raw_mode()?;

    // Create image protocol
    let mut image_state = picker.new_resize_protocol(img);

    // Show image in a dedicated view (handles its own event loop)
    show_image_view(&mut image_state, url)?;

    Ok(())
}

/// Show image in a fullscreen ratatui view
fn show_image_view(
    image_state: &mut StatefulProtocol,
    url: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut stdout = io::stdout();

    // Create a new terminal for the image view
    let backend = CrosstermBackend::new(&mut stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    loop {
        terminal.draw(|f| {
            let area = f.area();

            // Leave space for URL and instructions at top
            let chunks = Layout::vertical([
                Constraint::Length(3),
                Constraint::Min(1),
            ])
            .split(area);

            // Header with URL and instructions
            let header = Paragraph::new(format!(
                "{}\n\nPress any key to return, 'b' to open in browser",
                url
            ))
            .style(Style::default().fg(Color::DarkGray));
            f.render_widget(header, chunks[0]);

            // Image widget
            let image_widget = StatefulImage::default();
            f.render_stateful_widget(image_widget, chunks[1], image_state);
        })?;

        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press {
                    if key.code == KeyCode::Char('b') {
                        let _ = open::that(url);
                    }
                    break;
                }
    }

    // Clear terminal before returning
    terminal.clear()?;

    Ok(())
}

/// Open a URL in the default browser
fn open_url(url: &str) {
    let _ = open::that(url);
}

/// Login screen state
enum LoginState {
    Initial,
    WaitingForAuth { auth: DeviceFlowAuth },
    Error(String),
}

/// Run the login screen TUI
/// Returns Ok(Some(token)) on successful login, Ok(None) if user cancelled
pub async fn run_login_screen(client_id: &str) -> io::Result<Option<String>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = LoginState::Initial;
    let mut result: Option<String> = None;
    let mut should_quit = false;

    while !should_quit {
        terminal.draw(|f| {
            draw_login_screen(f, &state);
        })?;

        match &state {
            LoginState::Initial => {
                if event::poll(std::time::Duration::from_millis(100))? {
                    if let Event::Key(key) = event::read()? {
                        if key.kind == KeyEventKind::Press {
                            match key.code {
                                KeyCode::Enter => {
                                    // Start device flow
                                    state = LoginState::Error("Starting authentication...".to_string());
                                }
                                KeyCode::Esc => {
                                    should_quit = true;
                                }
                                _ => {}
                            }
                        }
                    }
                }

                // Check if we need to start the auth flow
                if matches!(state, LoginState::Error(ref msg) if msg == "Starting authentication...") {
                    // Temporarily exit TUI to start auth
                    disable_raw_mode()?;
                    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

                    match DeviceFlowAuth::start(client_id).await {
                        Ok(auth) => {
                            let _ = auth.open_browser();
                            execute!(io::stdout(), EnterAlternateScreen)?;
                            enable_raw_mode()?;
                            state = LoginState::WaitingForAuth { auth };
                        }
                        Err(e) => {
                            execute!(io::stdout(), EnterAlternateScreen)?;
                            enable_raw_mode()?;
                            state = LoginState::Error(format!("Auth failed: {}", e));
                        }
                    }
                }
            }
            LoginState::WaitingForAuth { auth } => {
                // Poll for events while waiting
                if event::poll(std::time::Duration::from_millis(100))? {
                    if let Event::Key(key) = event::read()? {
                        if key.kind == KeyEventKind::Press && key.code == KeyCode::Esc {
                            should_quit = true;
                            continue;
                        }
                    }
                }

                // We need to own the auth to poll it, so we'll try once
                // This is a bit tricky - we'll use a timeout approach
                let poll_result = tokio::time::timeout(
                    std::time::Duration::from_millis(500),
                    check_auth_once(client_id, &auth.device_code, &auth.client_id),
                )
                .await;

                match poll_result {
                    Ok(Ok(Some(token))) => {
                        if let Err(e) = auth::store_token(&token) {
                            state = LoginState::Error(format!("Failed to store token: {}", e));
                        } else {
                            result = Some(token);
                            should_quit = true;
                        }
                    }
                    Ok(Ok(None)) => {
                        // Still pending, continue waiting
                    }
                    Ok(Err(e)) => {
                        state = LoginState::Error(format!("Auth failed: {}", e));
                    }
                    Err(_) => {
                        // Timeout, continue waiting
                    }
                }
            }
            LoginState::Error(_) => {
                if event::poll(std::time::Duration::from_millis(100))? {
                    if let Event::Key(key) = event::read()? {
                        if key.kind == KeyEventKind::Press {
                            match key.code {
                                KeyCode::Enter => {
                                    state = LoginState::Initial;
                                }
                                KeyCode::Esc => {
                                    should_quit = true;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(result)
}

/// Check auth status once (helper for polling)
async fn check_auth_once(
    _client_id: &str,
    device_code: &str,
    client_id_stored: &str,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();

    let response = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", client_id_stored),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await?;

    let response_text = response.text().await?;

    #[derive(serde::Deserialize)]
    struct TokenResponse {
        access_token: Option<String>,
        error: Option<String>,
    }

    let data: TokenResponse = serde_json::from_str(&response_text)?;

    if let Some(token) = data.access_token {
        return Ok(Some(token));
    }

    if let Some(error) = data.error {
        match error.as_str() {
            "authorization_pending" | "slow_down" => Ok(None),
            _ => Err(error.into()),
        }
    } else {
        Ok(None)
    }
}

/// Draw the login screen
fn draw_login_screen(f: &mut Frame, state: &LoginState) {
    let area = f.area();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" GitHub Login ")
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let content = match state {
        LoginState::Initial => {
            vec![
                Line::from(""),
                Line::from(""),
                Line::styled(
                    "No GitHub connection detected.",
                    Style::default().fg(Color::Yellow),
                ),
                Line::from(""),
                Line::styled(
                    "Press Enter to login...",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Line::styled("Press Esc to quit.", Style::default().fg(Color::DarkGray)),
            ]
        }
        LoginState::WaitingForAuth { auth } => {
            vec![
                Line::from(""),
                Line::styled(
                    "Open this URL in your browser:",
                    Style::default().fg(Color::White),
                ),
                Line::from(""),
                Line::styled(
                    format!("  {}", auth.verification_uri),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
                Line::from(""),
                Line::styled("Enter code:", Style::default().fg(Color::White)),
                Line::from(""),
                Line::styled(
                    format!("  {}", auth.user_code),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Line::from(""),
                Line::styled(
                    "Waiting for authorization...",
                    Style::default().fg(Color::DarkGray),
                ),
                Line::from(""),
                Line::styled("Press Esc to cancel.", Style::default().fg(Color::DarkGray)),
            ]
        }
        LoginState::Error(msg) => {
            vec![
                Line::from(""),
                Line::styled(msg.clone(), Style::default().fg(Color::Red)),
                Line::from(""),
                Line::styled(
                    "Press Enter to retry, Esc to quit.",
                    Style::default().fg(Color::DarkGray),
                ),
            ]
        }
    };

    let text = Text::from(content);
    let paragraph = Paragraph::new(text).alignment(Alignment::Center);

    f.render_widget(paragraph, inner);
}

/// Run project selection screen
/// Returns the selected project name or None if cancelled
pub async fn run_project_select(projects: Vec<String>) -> io::Result<Option<String>> {
    if projects.is_empty() {
        return Ok(None);
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut selected: usize = 0;
    let mut result: Option<String> = None;
    let mut should_quit = false;

    while !should_quit {
        terminal.draw(|f| {
            draw_project_select_screen(f, &projects, selected);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            should_quit = true;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if selected > 0 {
                                selected -= 1;
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if selected < projects.len() - 1 {
                                selected += 1;
                            }
                        }
                        KeyCode::Enter => {
                            result = Some(projects[selected].clone());
                            should_quit = true;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(result)
}

/// Draw the project selection screen
fn draw_project_select_screen(f: &mut Frame, projects: &[String], selected: usize) {
    let area = f.area();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Select Project ")
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let items: Vec<ListItem> = projects
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let style = if i == selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let prefix = if i == selected { "> " } else { "  " };
            ListItem::new(Line::from(format!("{}{}", prefix, name))).style(style)
        })
        .collect();

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    // Add instructions at the bottom
    let chunks = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(inner);

    f.render_widget(list, chunks[0]);

    let help = Paragraph::new("↑↓ navigate │ Enter select │ q quit")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(help, chunks[1]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_image_urls_from_html() {
        let content = r#"Some text <img src="https://example.com/image.png" /> more text"#;
        let urls = extract_image_urls(content);
        assert_eq!(urls, vec!["https://example.com/image.png"]);
    }

    #[test]
    fn extract_image_urls_from_markdown() {
        let content = "Check this ![screenshot](https://example.com/shot.jpg) out";
        let urls = extract_image_urls(content);
        assert_eq!(urls, vec!["https://example.com/shot.jpg"]);
    }

    #[test]
    fn extract_image_urls_mixed_content() {
        let content = r#"
            ![First](https://a.com/1.png)
            <img src="https://b.com/2.png" alt="Second" />
            ![Third](https://c.com/3.png)
        "#;
        let urls = extract_image_urls(content);
        assert_eq!(urls.len(), 3);
        assert!(urls.contains(&"https://a.com/1.png".to_string()));
        assert!(urls.contains(&"https://b.com/2.png".to_string()));
        assert!(urls.contains(&"https://c.com/3.png".to_string()));
    }

    #[test]
    fn extract_image_urls_empty_content() {
        let urls = extract_image_urls("No images here");
        assert!(urls.is_empty());
    }

    #[test]
    fn parse_markdown_content_replaces_img_tag() {
        // Img tags are replaced with readable markers
        let content = r#"<img src="https://example.com/img.png" />"#;
        let result = parse_markdown_content(content);
        assert!(result.contains("[Image:"));
        assert!(result.contains("example.com"));
    }

    #[test]
    fn parse_markdown_content_preserves_text() {
        let content = "Some text before <img src=\"https://example.com/img.png\" /> and after";
        let result = parse_markdown_content(content);
        assert!(result.contains("Some text before"));
        assert!(result.contains("and after"));
        assert!(result.contains("[Image:"));
    }

    #[test]
    fn render_inline_markdown_removes_bold() {
        assert_eq!(render_inline_markdown("**bold** text"), "bold text");
        assert_eq!(
            render_inline_markdown("normal **bold** normal"),
            "normal bold normal"
        );
        assert_eq!(render_inline_markdown("no formatting"), "no formatting");
    }

    #[test]
    fn render_inline_markdown_multiple_bold() {
        assert_eq!(
            render_inline_markdown("**first** and **second**"),
            "first and second"
        );
    }

    #[test]
    fn fuzzy_match_assignees_exact() {
        let matcher = SkimMatcherV2::default();
        // Exact match should have high score
        assert!(matcher.fuzzy_match("alice", "alice").is_some());
    }

    #[test]
    fn fuzzy_match_assignees_partial() {
        let matcher = SkimMatcherV2::default();
        // Partial match should work
        assert!(matcher.fuzzy_match("alice", "ali").is_some());
        assert!(matcher.fuzzy_match("bob_smith", "bob").is_some());
    }

    #[test]
    fn fuzzy_match_assignees_no_match() {
        let matcher = SkimMatcherV2::default();
        // Completely different strings shouldn't match
        assert!(matcher.fuzzy_match("alice", "xyz").is_none());
    }

    #[test]
    fn fuzzy_match_assignees_case_insensitive() {
        let matcher = SkimMatcherV2::default();
        // Should match regardless of case
        assert!(matcher.fuzzy_match("Alice", "alice").is_some());
        assert!(matcher.fuzzy_match("BOB", "bob").is_some());
    }
}
