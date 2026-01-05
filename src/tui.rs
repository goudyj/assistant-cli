//! TUI main module - Issue browser for GitHub issues.
//!
//! This module provides the main TUI application for browsing and managing GitHub issues.
//! The implementation is split across several modules:
//! - `tui_types`: View state and type definitions
//! - `tui_draw`: UI rendering functions
//! - `tui_events`: Keyboard event handling
//! - `tui_image`: Image display functionality
//! - `tui_utils`: Utility functions

use crate::clipboard::get_clipboard_content;
use crate::config::ProjectConfig;
use crate::github::{GitHubConfig, IssueDetail, IssueSummary};
use crate::images::extract_image_urls;
use crate::llm;

// Re-export types for external use
pub use crate::tui_types::{CommandSuggestion, CreateStage, TuiView};

// Re-export standalone TUI screens from their modules
pub use crate::login_screen::run_login_screen;
pub use crate::project_select::run_project_select;

// Import from internal modules
use crate::tui_draw::draw_ui;
use crate::tui_events::{handle_key_event, handle_paste};
pub use crate::tui_image::display_image;

use crossterm::{
    event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use ratatui::{backend::CrosstermBackend, widgets::ListState, Terminal};
use std::io;

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
    // Available projects for repository switching
    pub available_projects: Vec<(String, ProjectConfig)>,
    // IDE command for opening worktrees
    pub ide_command: Option<String>,
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
            available_projects: Vec::new(),
            ide_command: None,
        }
    }

    /// Set the IDE command for opening worktrees
    pub fn set_ide_command(&mut self, command: Option<String>) {
        self.ide_command = command;
    }

    /// Build worktree list with session status
    pub fn build_worktree_list(&self) -> Vec<crate::agents::WorktreeInfo> {
        let manager = crate::agents::SessionManager::load();
        let session_worktrees: Vec<_> = manager.list().iter().map(|s| s.worktree_path.clone()).collect();

        let mut worktrees = crate::agents::list_worktrees();
        for wt in &mut worktrees {
            wt.has_session = session_worktrees.contains(&wt.path);
            if let Some(issue_num) = wt.issue_number {
                let tmux_name = crate::agents::tmux_session_name(&wt.project, issue_num);
                wt.has_tmux = crate::agents::is_tmux_session_running(&tmux_name);
            }
        }
        worktrees
    }

    /// Get orphaned worktrees (no active session)
    pub fn get_orphaned_worktrees(&self) -> Vec<crate::agents::WorktreeInfo> {
        let manager = crate::agents::SessionManager::load();
        let session_worktrees: Vec<_> = manager.list().iter().map(|s| s.worktree_path.clone()).collect();
        crate::agents::list_orphaned_worktrees(&session_worktrees)
    }

    /// Set project info for Claude Code dispatch
    pub fn set_project_info(&mut self, name: String, path: std::path::PathBuf) {
        self.project_name = Some(name.clone());
        self.local_path = Some(path);
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

    /// Set available projects for repository switching
    pub fn set_available_projects(&mut self, projects: Vec<(String, ProjectConfig)>) {
        self.available_projects = projects;
    }

    /// Switch to a different project/repository
    pub async fn switch_project(&mut self, name: &str, project: &ProjectConfig, token: &str) {
        // Update GitHub config
        self.github = GitHubConfig::new(project.owner.clone(), project.repo.clone(), token.to_string());

        // Update project info
        self.project_name = Some(name.to_string());
        self.local_path = project.local_path.clone();

        // Update labels
        self.project_labels = project.labels.clone();
        self.list_labels.clear();

        // Rebuild commands
        let mut commands = vec![
            CommandSuggestion {
                name: "all".to_string(),
                description: "Show all issues (clear filters)".to_string(),
                labels: None,
            },
            CommandSuggestion {
                name: "logout".to_string(),
                description: "Logout from GitHub".to_string(),
                labels: None,
            },
            CommandSuggestion {
                name: "repository".to_string(),
                description: "Switch repository".to_string(),
                labels: None,
            },
            CommandSuggestion {
                name: "worktrees".to_string(),
                description: "Manage worktrees (view, delete, open IDE)".to_string(),
                labels: None,
            },
            CommandSuggestion {
                name: "prune".to_string(),
                description: "Clean up orphaned worktrees".to_string(),
                labels: None,
            },
        ];
        for (cmd_name, labels) in &project.list_commands {
            commands.push(CommandSuggestion {
                name: cmd_name.clone(),
                description: format!("Filter: {}", labels.join(", ")),
                labels: Some(labels.clone()),
            });
        }
        self.available_commands = commands;

        // Refresh sessions for new project
        self.session_cache.clear();
        self.refresh_sessions(name);

        // Reload issues for the new project
        self.reload_issues().await;

        // Save last project to config
        if let Ok(mut config) = crate::config::load_config() {
            config.set_last_project(name);
            let _ = config.save();
        }
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

        let running_sessions: Vec<_> = manager
            .running()
            .iter()
            .map(|s| (s.id.clone(), s.worktree_path.clone()))
            .collect();

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
                self.all_issues.extend(new_issues.clone());

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
                self.status_message = Some(format!(
                    "Loaded page {} ({} issues total)",
                    next_page,
                    self.all_issues.len()
                ));
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
            self.issues = self
                .all_issues
                .iter()
                .filter(|issue| {
                    issue.title.to_lowercase().contains(&query_lower)
                        || issue.labels.iter().any(|l| l.to_lowercase().contains(&query_lower))
                })
                .cloned()
                .collect();
            self.search_query = Some(query.to_string());
        }

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

        if let Some(ref body) = issue.body {
            images.extend(extract_image_urls(body));
        }

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
        Vec::new(),
        None,
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
    available_projects: Vec<(String, ProjectConfig)>,
    ide_command: Option<String>,
) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
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

    if let (Some(name), Some(path)) = (project_name.clone(), local_path) {
        browser.set_project_info(name, path);
    } else if let Some(name) = project_name {
        browser.project_name = Some(name);
    }

    browser.set_project_labels(project_labels);
    browser.set_available_commands(available_commands);
    browser.set_available_projects(available_projects);
    browser.set_ide_command(ide_command);

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
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('v')
                    {
                        if let Ok(content) = get_clipboard_content() {
                            handle_paste(&mut browser, &content);
                        }
                    } else {
                        handle_key_event(&mut browser, key.code, key.modifiers).await;
                    }
                }
                Event::Paste(content) => {
                    handle_paste(&mut browser, &content);
                }
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableBracketedPaste,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    Ok(())
}

/// Format a comment using LLM
pub async fn format_comment_with_llm(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_match_assignees_exact() {
        let matcher = SkimMatcherV2::default();
        assert!(matcher.fuzzy_match("alice", "alice").is_some());
    }

    #[test]
    fn fuzzy_match_assignees_partial() {
        let matcher = SkimMatcherV2::default();
        assert!(matcher.fuzzy_match("alice", "ali").is_some());
        assert!(matcher.fuzzy_match("bob_smith", "bob").is_some());
    }

    #[test]
    fn fuzzy_match_assignees_no_match() {
        let matcher = SkimMatcherV2::default();
        assert!(matcher.fuzzy_match("alice", "xyz").is_none());
    }

    #[test]
    fn fuzzy_match_assignees_case_insensitive() {
        let matcher = SkimMatcherV2::default();
        assert!(matcher.fuzzy_match("Alice", "alice").is_some());
        assert!(matcher.fuzzy_match("BOB", "bob").is_some());
    }
}
