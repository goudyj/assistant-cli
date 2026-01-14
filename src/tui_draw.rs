//! TUI rendering functions.

use std::path::Path;

use std::collections::HashSet;

use crate::commands::{format_status_bar, generate_full_help, CommandContext};
use crate::github::{IssueDetail, PullRequestDetail};
use crate::issues::IssueContent;
use crate::markdown::{parse_markdown_content, render_markdown_line};
use crate::tui_types::{CommandSuggestion, CreateStage, IssueFilterFocus, IssueStatus, PrFilterFocus, PrStatus, TuiView};
use crate::tui_utils::{format_date, truncate_str};

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::tui::IssueBrowser;

/// Main UI dispatcher - routes to appropriate view renderer
pub fn draw_ui(f: &mut Frame, browser: &mut IssueBrowser) {
    let image_count = browser.current_images.len();

    // Auto-refresh log content if viewing running agent logs
    if let TuiView::AgentLogs {
        session_id,
        content,
        ..
    } = &mut browser.view
    {
        let manager = crate::agents::SessionManager::load();
        if let Some(session) = manager.get(session_id)
            && session.is_running()
                && let Ok(new_content) = std::fs::read_to_string(&session.log_file)
        {
            *content = new_content;
        }
    }

    // Clone status message to avoid borrow conflicts
    let status_msg = browser.status_message.clone();

    // Extract search input before match to avoid borrow conflicts
    let search_input = if let TuiView::Search { input } = &browser.view {
        Some(input.clone())
    } else {
        None
    };

    // Extract dispatch instructions data before match to avoid borrow conflicts
    let dispatch_instructions_data =
        if let TuiView::DispatchInstructions { issue, input } = &browser.view {
            Some((issue.number, input.clone()))
        } else {
            None
        };

    // Extract worktree agent instructions data before match to avoid borrow conflicts
    let worktree_instructions_data =
        if let TuiView::WorktreeAgentInstructions {
            branch_name, input, ..
        } = &browser.view
        {
            Some((branch_name.clone(), input.clone()))
        } else {
            None
        };

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
        TuiView::Search { .. } => {
            // Draw the list behind the popup
            draw_list_view(f, browser);
            // Draw centered search popup on top
            if let Some(input) = &search_input {
                draw_search_popup(f, input);
            }
        }
        TuiView::Detail(issue) => {
            if let Some(ref msg) = status_msg {
                let chunks = Layout::vertical([Constraint::Min(3), Constraint::Length(3)])
                    .split(f.area());
                draw_detail_view(f, chunks[0], issue, browser.scroll_offset, image_count);
                draw_status_bar(f, chunks[1], msg);
            } else {
                draw_detail_view(f, f.area(), issue, browser.scroll_offset, image_count);
            }
        }
        TuiView::AddComment { issue, input } => {
            let chunks =
                Layout::vertical([Constraint::Percentage(75), Constraint::Percentage(25)])
                    .split(f.area());

            draw_detail_view(f, chunks[0], issue, browser.scroll_offset, image_count);
            draw_comment_input(f, chunks[1], input, browser.status_message.as_deref());
        }
        TuiView::ConfirmClose { issue } => {
            let chunks =
                Layout::vertical([Constraint::Percentage(80), Constraint::Percentage(20)])
                    .split(f.area());

            draw_detail_view(f, chunks[0], issue, browser.scroll_offset, image_count);
            draw_confirmation(f, chunks[1], &format!("Close issue #{}? (y/n)", issue.number));
        }
        TuiView::ConfirmReopen { issue } => {
            let chunks =
                Layout::vertical([Constraint::Percentage(80), Constraint::Percentage(20)])
                    .split(f.area());

            draw_detail_view(f, chunks[0], issue, browser.scroll_offset, image_count);
            draw_confirmation(
                f,
                chunks[1],
                &format!("Reopen issue #{}? (y/n)", issue.number),
            );
        }
        TuiView::AssignUser {
            issue,
            input,
            suggestions,
            selected,
        } => {
            let chunks =
                Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)])
                    .split(f.area());

            draw_detail_view(f, chunks[0], issue, browser.scroll_offset, image_count);
            draw_assignee_picker(f, chunks[1], issue, input, suggestions, *selected);
        }
        TuiView::ConfirmDispatch { issue } => {
            let chunks =
                Layout::vertical([Constraint::Percentage(80), Constraint::Percentage(20)])
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
        TuiView::EmbeddedTmux {
            available_sessions,
            current_index,
            ..
        } => {
            draw_embedded_tmux(f, browser, available_sessions, *current_index);
        }
        TuiView::ProjectSelect { projects, selected } => {
            draw_project_select_inline(f, projects, *selected);
        }
        TuiView::AgentSelect { selected } => {
            draw_agent_select(f, *selected, &browser.coding_agent);
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
            draw_direct_issue(
                f,
                title,
                body,
                *editing_body,
                browser.status_message.as_deref(),
            );
        }
        TuiView::WorktreeList {
            worktrees,
            selected,
        } => {
            if let Some(ref msg) = status_msg {
                let chunks = Layout::vertical([Constraint::Min(3), Constraint::Length(3)])
                    .split(f.area());
                draw_worktree_list(f, chunks[0], worktrees, *selected);
                draw_status_bar(f, chunks[1], msg);
            } else {
                draw_worktree_list(f, f.area(), worktrees, *selected);
            }
        }
        TuiView::ConfirmPrune { orphaned } => {
            draw_confirm_prune(f, orphaned);
        }
        TuiView::ConfirmDeleteWorktree { worktree, .. } => {
            draw_confirm_delete_worktree(f, worktree);
        }
        TuiView::CreateWorktree { input } => {
            draw_create_worktree(f, input);
        }
        TuiView::PostWorktreeCreate {
            worktree_path,
            branch_name,
        } => {
            draw_post_worktree_create(f, worktree_path, branch_name);
        }
        TuiView::DispatchInstructions { .. } => {
            // Draw the list behind the popup
            draw_list_view(f, browser);
            // Draw centered instructions popup on top
            if let Some((issue_number, input)) = &dispatch_instructions_data {
                draw_dispatch_instructions(f, *issue_number, input);
            }
        }
        TuiView::WorktreeAgentInstructions { .. } => {
            // Draw centered instructions popup
            if let Some((branch_name, input)) = &worktree_instructions_data {
                draw_worktree_agent_instructions(f, branch_name, input);
            }
        }
        TuiView::Help => {
            draw_help(f);
        }
        TuiView::PullRequestList => {
            if let Some(ref msg) = status_msg {
                let chunks = Layout::vertical([Constraint::Min(3), Constraint::Length(3)])
                    .split(f.area());
                draw_pr_list_view_in_area(f, browser, chunks[0]);
                draw_status_bar(f, chunks[1], msg);
            } else {
                draw_pr_list_view(f, browser);
            }
        }
        TuiView::PullRequestDetail(pr) => {
            draw_pr_detail_view(f, f.area(), pr, browser.scroll_offset);
        }
        TuiView::ConfirmMerge { pr } => {
            draw_pr_detail_view(f, f.area(), pr, browser.scroll_offset);
            draw_confirm_merge_popup(f, pr);
        }
        TuiView::DispatchPrReview { pr, input } => {
            // Clone data to avoid borrow issues
            let pr_clone = pr.clone();
            let input_clone = input.clone();
            draw_pr_list_view(f, browser);
            draw_pr_review_popup(f, &pr_clone, &input_clone);
        }
        TuiView::PrFilters {
            status_filter,
            author_filter,
            available_authors,
            focus,
            selected_status,
            selected_author,
            author_input,
            author_suggestions,
        } => {
            // Clone data to avoid borrow issues
            let status_filter = status_filter.clone();
            let author_filter = author_filter.clone();
            let available_authors = available_authors.clone();
            let focus = *focus;
            let selected_status = *selected_status;
            let selected_author = *selected_author;
            let author_input = author_input.clone();
            let author_suggestions = author_suggestions.clone();
            draw_pr_list_view(f, browser);
            draw_pr_filters_popup(
                f,
                &status_filter,
                &author_filter,
                &available_authors,
                &focus,
                selected_status,
                selected_author,
                &author_input,
                &author_suggestions,
            );
        }
        TuiView::IssueFilters {
            status_filter,
            author_filter,
            available_authors,
            focus,
            selected_status,
            selected_author,
            author_input,
            author_suggestions,
        } => {
            // Clone data to avoid borrow issues
            let status_filter = status_filter.clone();
            let author_filter = author_filter.clone();
            let available_authors = available_authors.clone();
            let focus = *focus;
            let selected_status = *selected_status;
            let selected_author = *selected_author;
            let author_input = author_input.clone();
            let author_suggestions = author_suggestions.clone();
            draw_list_view(f, browser);
            draw_issue_filters_popup(
                f,
                &status_filter,
                &author_filter,
                &available_authors,
                &focus,
                selected_status,
                selected_author,
                &author_input,
                &author_suggestions,
            );
        }
    }
}

/// Draw the main issue list view (full screen)
pub fn draw_list_view(f: &mut Frame, browser: &mut IssueBrowser) {
    let area = f.area();
    draw_list_view_in_area(f, browser, area);
}

/// Draw list view in a specific area
pub fn draw_list_view_in_area(f: &mut Frame, browser: &mut IssueBrowser, area: Rect) {
    use crate::agents::AgentStatus;

    let items: Vec<ListItem> = browser
        .issues
        .iter()
        .map(|issue| {
            let is_selected = browser.selected_issues.contains(&issue.number);
            let select_marker = if is_selected { "[x] " } else { "[ ] " };

            let session_info = browser.session_cache.get(&issue.number);
            let (session_icon, session_color, session_stats) = match session_info {
                Some(session) => {
                    let (icon, color) = match &session.status {
                        AgentStatus::Running => ("▶", Color::Yellow),
                        AgentStatus::Awaiting => ("⏸", Color::Cyan),
                        AgentStatus::Completed { .. } | AgentStatus::Failed { .. } => {
                            ("●", Color::Blue)
                        }
                    };
                    let stats = if session.stats.lines_added > 0 || session.stats.lines_deleted > 0
                    {
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
                Line::from(vec![
                    Span::styled(
                        select_marker,
                        if is_selected {
                            Style::default().fg(Color::Green)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        },
                    ),
                    Span::styled(
                        format!("#{:<5}", issue.number),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(" ✓ ", Style::default().fg(Color::Green)),
                    Span::styled(
                        &issue.title,
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::CROSSED_OUT),
                    ),
                    Span::styled(labels_str, Style::default().fg(Color::DarkGray)),
                    Span::styled(assignees_str, Style::default().fg(Color::DarkGray)),
                ])
            } else {
                let session_span = if let Some(icon) = session_icon {
                    Span::styled(
                        format!("{}{}  ", icon, session_stats),
                        Style::default().fg(session_color),
                    )
                } else {
                    Span::raw("    ")
                };

                Line::from(vec![
                    Span::styled(
                        select_marker,
                        if is_selected {
                            Style::default().fg(Color::Green)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        },
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

    let title = build_list_title(browser);

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, area, &mut browser.list_state);
}

/// Build the title string for the list view
fn build_list_title(browser: &IssueBrowser) -> String {
    let mut parts = Vec::new();
    parts.push("Issues".to_string());

    if let Some(ref query) = browser.search_query {
        parts.push(format!("(filtered: '{}')", query));
    }

    if browser.has_next_page {
        parts.push(format!(
            "[{} loaded, more available]",
            browser.all_issues.len()
        ));
    } else if browser.all_issues.len() > 20 {
        parts.push(format!("[{} total]", browser.all_issues.len()));
    }

    if browser.is_loading {
        parts.push("[Loading...]".to_string());
    }

    if !browser.selected_issues.is_empty() {
        parts.push(format!("[{} selected]", browser.selected_issues.len()));
    }

    format_status_bar(CommandContext::IssueList, &parts.join(" "))
}

/// Draw centered search popup
pub fn draw_search_popup(f: &mut Frame, input: &str) {
    let area = f.area();

    // Calculate centered popup area (50 chars wide, 3 lines tall)
    let popup_width = 50.min(area.width.saturating_sub(4));
    let popup_height = 3;
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear the background behind the popup
    let clear = Block::default().style(Style::default().bg(Color::Black));
    f.render_widget(clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Search GitHub ")
        .border_style(Style::default().fg(Color::Yellow));

    let text = format!("{}_", input);
    let paragraph = Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, popup_area);
}

/// Draw issue detail view
pub fn draw_detail_view(
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

    let close_key = if issue.state == "Closed" {
        "X reopen"
    } else {
        "x close"
    };
    let title = if image_count > 0 {
        format!(
            " #{} │ o open │ c comment │ a assign │ d dispatch │ {} │ i/O image [{}/{}] │ ↑↓ scroll │ Esc ",
            issue.number, close_key, 1, image_count
        )
    } else {
        format!(
            " #{} │ o open │ c comment │ a assign │ d dispatch │ {} │ ↑↓ scroll │ Esc ",
            issue.number, close_key
        )
    };

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));

    f.render_widget(paragraph, area);
}

/// Draw comment input area
pub fn draw_comment_input(f: &mut Frame, area: Rect, input: &str, status: Option<&str>) {
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

/// Draw confirmation dialog
pub fn draw_confirmation(f: &mut Frame, area: Rect, message: &str) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let paragraph = Paragraph::new(message)
        .block(block)
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center);

    f.render_widget(paragraph, area);
}

/// Draw status bar
pub fn draw_status_bar(f: &mut Frame, area: Rect, message: &str) {
    let color = if message.contains("Failed")
        || message.contains("No ")
        || message.contains("error")
    {
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
        .alignment(Alignment::Center);

    f.render_widget(paragraph, area);
}

/// Draw assignee picker
pub fn draw_assignee_picker(
    f: &mut Frame,
    area: Rect,
    issue: &IssueDetail,
    input: &str,
    suggestions: &[String],
    selected: usize,
) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(3),
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
            let prefix = if issue.assignees.contains(name) {
                "✓ "
            } else {
                "  "
            };
            ListItem::new(Line::from(format!("{}{}", prefix, name))).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Suggestions "),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(list, chunks[2]);
}

/// Draw agent logs view
pub fn draw_agent_logs(f: &mut Frame, session_id: &str, content: &str, scroll: u16) {
    let lines: Vec<Line> = content
        .lines()
        .map(|line| Line::from(line.to_string()))
        .collect();

    let short_id: String = session_id.chars().take(8).collect();
    let title = format!(" Agent {} │ ↑↓ scroll │ q back ", short_id);

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

/// Draw embedded tmux terminal view
pub fn draw_embedded_tmux(
    f: &mut Frame,
    browser: &IssueBrowser,
    available_sessions: &[String],
    current_index: usize,
) {
    let area = f.area();

    let header_text = if available_sessions.is_empty() {
        " No tmux session │ ESC ESC to exit ".to_string()
    } else {
        let session_name = &available_sessions[current_index];
        format!(
            " {} │ ←→ switch ({}/{}) │ ESC ESC exit ",
            session_name,
            current_index + 1,
            available_sessions.len()
        )
    };

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
        let placeholder = Paragraph::new("Starting terminal...")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(placeholder, chunks[1]);
    }
}

/// Draw project selection inline
pub fn draw_project_select_inline(f: &mut Frame, projects: &[String], selected: usize) {
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

/// Draw agent selection screen
pub fn draw_agent_select(
    f: &mut Frame,
    selected: usize,
    current_agent: &crate::config::CodingAgentType,
) {
    let area = f.area();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Select Dispatch Agent ")
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let agents = [
        ("Claude Code", crate::config::CodingAgentType::Claude),
        ("Opencode", crate::config::CodingAgentType::Opencode),
    ];

    let items: Vec<ListItem> = agents
        .iter()
        .enumerate()
        .map(|(i, (name, agent_type))| {
            let is_current = agent_type == current_agent;
            let style = if i == selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let prefix = if i == selected { "> " } else { "  " };
            let suffix = if is_current { " (current)" } else { "" };
            ListItem::new(Line::from(format!("{}{}{}", prefix, name, suffix))).style(style)
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
pub fn draw_command_palette(
    f: &mut Frame,
    browser: &IssueBrowser,
    input: &str,
    suggestions: &[CommandSuggestion],
    selected: usize,
) {
    let area = f.area();

    let chunks =
        Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)]).split(area);

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
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
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
pub fn draw_create_issue(f: &mut Frame, input: &str, stage: &CreateStage) {
    let area = f.area();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Create Issue ")
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    match stage {
        CreateStage::Description => {
            let chunks = Layout::vertical([
                Constraint::Length(2),
                Constraint::Min(3),
                Constraint::Length(2),
            ])
            .split(inner);

            let prompt =
                Paragraph::new("Describe the issue:").style(Style::default().fg(Color::White));
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
pub fn draw_preview_issue(
    f: &mut Frame,
    issue: &IssueContent,
    feedback_input: &str,
    scroll: u16,
) {
    let area = f.area();

    let chunks =
        Layout::vertical([Constraint::Percentage(75), Constraint::Percentage(25)]).split(area);

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

/// Draw direct issue creation screen
pub fn draw_direct_issue(
    f: &mut Frame,
    title: &str,
    body: &str,
    editing_body: bool,
    status_message: Option<&str>,
) {
    let area = f.area();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" New Issue (direct) ")
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

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
        .title(if !editing_body {
            " Title (editing) "
        } else {
            " Title "
        })
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
        .title(if editing_body {
            " Body (editing) "
        } else {
            " Body "
        })
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

/// Draw worktree list view
pub fn draw_worktree_list(
    f: &mut Frame,
    area: Rect,
    worktrees: &[crate::agents::WorktreeInfo],
    selected: usize,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Worktrees ")
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::vertical([Constraint::Min(3), Constraint::Length(2)]).split(inner);

    // Handle empty list
    if worktrees.is_empty() {
        let empty_msg = Paragraph::new("No worktrees. Press 'n' to create one.")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(empty_msg, chunks[0]);

        let help = Paragraph::new(format_status_bar(CommandContext::WorktreeList, ""))
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(help, chunks[1]);
        return;
    }

    let items: Vec<ListItem> = worktrees
        .iter()
        .enumerate()
        .map(|(idx, wt)| {
            let is_selected = idx == selected;

            // Status indicators
            let status_icon = if wt.has_tmux {
                Span::styled("▶ ", Style::default().fg(Color::Yellow))
            } else if wt.has_session {
                Span::styled("● ", Style::default().fg(Color::Green))
            } else {
                Span::styled("○ ", Style::default().fg(Color::DarkGray))
            };

            let issue_str = wt
                .issue_number
                .map(|n| format!("#{:<5}", n))
                .unwrap_or_else(|| "     ".to_string());

            let has_agent = wt.has_session || wt.has_tmux;
            let name_style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else if has_agent {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let orphan_indicator = if !has_agent {
                Span::styled(" (no agent)", Style::default().fg(Color::DarkGray))
            } else {
                Span::raw("")
            };

            let line = Line::from(vec![
                status_icon,
                Span::styled(
                    issue_str,
                    Style::default().fg(if has_agent {
                        Color::Cyan
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::raw(" "),
                Span::styled(&wt.name, name_style),
                orphan_indicator,
            ]);

            let style = if is_selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            ListItem::new(line).style(style)
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, chunks[0]);

    // Help bar
    let help = Paragraph::new(format_status_bar(CommandContext::WorktreeList, ""))
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(help, chunks[1]);
}

/// Draw confirmation dialog for pruning orphaned worktrees
pub fn draw_confirm_prune(f: &mut Frame, orphaned: &[crate::agents::WorktreeInfo]) {
    let area = f.area();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Confirm Prune ")
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks =
        Layout::vertical([Constraint::Length(3), Constraint::Min(3), Constraint::Length(2)])
            .split(inner);

    // Header
    let header = Paragraph::new(format!(
        "Delete {} orphaned worktree{}?",
        orphaned.len(),
        if orphaned.len() == 1 { "" } else { "s" }
    ))
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
    .alignment(Alignment::Center);
    f.render_widget(header, chunks[0]);

    // List of worktrees to delete
    let items: Vec<ListItem> = orphaned
        .iter()
        .map(|wt| {
            let issue_str = wt
                .issue_number
                .map(|n| format!("#{}", n))
                .unwrap_or_default();

            let line = Line::from(vec![
                Span::styled("  • ", Style::default().fg(Color::Red)),
                Span::styled(&wt.name, Style::default().fg(Color::White)),
                Span::styled(
                    format!(" ({})", issue_str),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, chunks[1]);

    // Confirmation prompt
    let prompt = Paragraph::new("Press Y to confirm, N or Esc to cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(prompt, chunks[2]);
}

/// Draw confirmation dialog for deleting a single worktree
pub fn draw_confirm_delete_worktree(f: &mut Frame, worktree: &crate::agents::WorktreeInfo) {
    let area = f.area();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Confirm Delete ")
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks =
        Layout::vertical([Constraint::Length(3), Constraint::Length(3), Constraint::Length(2)])
            .split(inner);

    // Header
    let header = Paragraph::new("Delete this worktree?")
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center);
    f.render_widget(header, chunks[0]);

    // Worktree info
    let issue_str = worktree
        .issue_number
        .map(|n| format!(" (#{}) ", n))
        .unwrap_or_else(|| " ".to_string());

    let info = Paragraph::new(Line::from(vec![
        Span::styled("  • ", Style::default().fg(Color::Red)),
        Span::styled(&worktree.name, Style::default().fg(Color::White)),
        Span::styled(issue_str, Style::default().fg(Color::Cyan)),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(info, chunks[1]);

    // Confirmation prompt
    let prompt = Paragraph::new("Press Y to confirm, N or Esc to cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(prompt, chunks[2]);
}

/// Draw create worktree input screen
pub fn draw_create_worktree(f: &mut Frame, input: &str) {
    let area = f.area();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Create Worktree ")
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .split(inner);

    let prompt = Paragraph::new("Enter branch name:")
        .style(Style::default().fg(Color::White));
    f.render_widget(prompt, chunks[0]);

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let (input_text, input_style) = if input.is_empty() {
        (
            "e.g., feature/dark-mode".to_string(),
            Style::default().fg(Color::DarkGray),
        )
    } else {
        (
            format!("{}_", input),
            Style::default().fg(Color::White),
        )
    };
    let input_para = Paragraph::new(input_text)
        .block(input_block)
        .style(input_style);
    f.render_widget(input_para, chunks[1]);

    let help = Paragraph::new("Enter: create │ Esc: cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(help, chunks[3]);
}

/// Draw post worktree creation choice screen
pub fn draw_post_worktree_create(f: &mut Frame, worktree_path: &Path, branch_name: &str) {
    let area = f.area();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Worktree Created ")
        .border_style(Style::default().fg(Color::Green));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(4),
        Constraint::Length(5),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .split(inner);

    // Success message
    let success = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Branch: ", Style::default().fg(Color::DarkGray)),
            Span::styled(branch_name, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Path: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                worktree_path.display().to_string(),
                Style::default().fg(Color::White),
            ),
        ]),
    ]);
    f.render_widget(success, chunks[0]);

    // Options
    let options = Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  o  ", Style::default().fg(Color::Yellow)),
            Span::raw("Open in IDE"),
        ]),
        Line::from(vec![
            Span::styled("  a  ", Style::default().fg(Color::Yellow)),
            Span::raw("Start agent"),
        ]),
        Line::from(vec![
            Span::styled(" Esc ", Style::default().fg(Color::Yellow)),
            Span::raw("Back to worktree list"),
        ]),
    ]);
    f.render_widget(options, chunks[1]);

    let help = Paragraph::new("What would you like to do?")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(help, chunks[3]);
}

/// Draw help screen with all keyboard shortcuts (auto-generated from commands module)
pub fn draw_help(f: &mut Frame) {
    let area = f.area();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Keyboard Shortcuts \u{2502} Esc/q/? to close ")
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let help_text = generate_full_help();

    let paragraph = Paragraph::new(help_text).scroll((0, 0));
    f.render_widget(paragraph, inner);
}

/// Draw instructions popup for dispatching an issue
pub fn draw_dispatch_instructions(f: &mut Frame, issue_number: u64, input: &str) {
    let area = f.area();

    // Calculate centered popup area (60% width, 10 lines tall)
    let popup_width = (area.width * 60 / 100).max(40).min(area.width.saturating_sub(4));
    let popup_height = 10.min(area.height.saturating_sub(4));
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear the background behind the popup
    let clear = Block::default().style(Style::default().bg(Color::Black));
    f.render_widget(clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Add instructions for #{} ", issue_number))
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Split inner area: instructions text area + hint
    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);

    // Display input with cursor
    let display_text = if input.is_empty() {
        "Optional: add context for the agent...".to_string()
    } else {
        format!("{}_", input)
    };

    let style = if input.is_empty() {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };

    let paragraph = Paragraph::new(display_text)
        .style(style)
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, chunks[0]);

    // Draw hint at bottom
    let hint = Paragraph::new("Shift+Enter: newline │ Enter: dispatch │ Esc: cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(hint, chunks[1]);
}

/// Draw instructions popup for starting agent on worktree
pub fn draw_worktree_agent_instructions(f: &mut Frame, branch_name: &str, input: &str) {
    let area = f.area();

    // Calculate centered popup area (60% width, 10 lines tall)
    let popup_width = (area.width * 60 / 100).max(40).min(area.width.saturating_sub(4));
    let popup_height = 10.min(area.height.saturating_sub(4));
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear the background behind the popup
    let clear = Block::default().style(Style::default().bg(Color::Black));
    f.render_widget(clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Add instructions for {} ", branch_name))
        .border_style(Style::default().fg(Color::Green));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Split inner area: instructions text area + hint
    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);

    // Display input with cursor
    let display_text = if input.is_empty() {
        "Optional: add context for the agent...".to_string()
    } else {
        format!("{}_", input)
    };

    let style = if input.is_empty() {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };

    let paragraph = Paragraph::new(display_text)
        .style(style)
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, chunks[0]);

    // Draw hint at bottom
    let hint = Paragraph::new("Shift+Enter: newline │ Enter: start agent │ Esc: cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(hint, chunks[1]);
}

/// Draw the pull request list view (full screen)
pub fn draw_pr_list_view(f: &mut Frame, browser: &mut IssueBrowser) {
    let area = f.area();
    draw_pr_list_view_in_area(f, browser, area);
}

/// Draw PR list view in a specific area
pub fn draw_pr_list_view_in_area(f: &mut Frame, browser: &mut IssueBrowser, area: Rect) {
    let items: Vec<ListItem> = browser
        .pull_requests
        .iter()
        .map(|pr| {
            let mut spans = Vec::new();

            // PR number
            spans.push(Span::styled(
                format!("#{:<5}", pr.number),
                Style::default().fg(Color::Cyan),
            ));

            // Status indicator
            let (status_text, status_style) = if pr.draft {
                ("DRAFT ", Style::default().fg(Color::DarkGray))
            } else if pr.state.to_lowercase().contains("merged") {
                ("MERGED", Style::default().fg(Color::Magenta))
            } else if pr.state.to_lowercase().contains("closed") {
                ("CLOSED", Style::default().fg(Color::Red))
            } else {
                ("OPEN  ", Style::default().fg(Color::Green))
            };
            spans.push(Span::styled(format!("{} ", status_text), status_style));

            // Branches: head → base
            let branch_text = format!("{} → {}", pr.head_ref, pr.base_ref);
            let truncated_branch = format!("{:<30}", truncate_str(&branch_text, 27));
            spans.push(Span::styled(truncated_branch, Style::default().fg(Color::Yellow)));

            // Title (truncated)
            let max_title_len = area.width.saturating_sub(70) as usize;
            let title = truncate_str(&pr.title, max_title_len);
            spans.push(Span::raw(title));
            spans.push(Span::raw(" "));

            // Author
            if !pr.author.is_empty() {
                spans.push(Span::styled(
                    format!("@{} ", pr.author),
                    Style::default().fg(Color::Magenta),
                ));
            }

            // Review status
            if let Some(ref review) = pr.review_decision {
                let (icon, style) = match review.as_str() {
                    "APPROVED" => ("✓ approved", Style::default().fg(Color::Green)),
                    "CHANGES_REQUESTED" => ("✗ changes", Style::default().fg(Color::Red)),
                    _ => ("○ pending", Style::default().fg(Color::Yellow)),
                };
                spans.push(Span::styled(icon, style));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    // Build title with filter indicators
    let title = build_pr_list_title(browser);

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, area, &mut browser.pr_list_state);
}

/// Build the title for PR list with active filters
fn build_pr_list_title(browser: &IssueBrowser) -> String {
    let mut parts = vec!["Issues | [PRs]".to_string()];

    // Show active status filters
    if !browser.pr_status_filter.is_empty() {
        let statuses: Vec<&str> = browser
            .pr_status_filter
            .iter()
            .map(|s| s.label())
            .collect();
        parts.push(format!("[{}]", statuses.join(",")));
    }

    // Show active author filters
    if !browser.pr_author_filter.is_empty() {
        let authors: Vec<String> = browser
            .pr_author_filter
            .iter()
            .map(|a| format!("@{}", a))
            .collect();
        parts.push(format!("[{}]", authors.join(",")));
    }

    // Loading indicator
    if browser.pr_is_loading {
        parts.push("[Loading...]".to_string());
    } else if browser.pr_has_next_page {
        parts.push(format!("[{} loaded, more available]", browser.pull_requests.len()));
    } else {
        parts.push(format!("[{} total]", browser.pull_requests.len()));
    }

    format_status_bar(CommandContext::PullRequestList, &parts.join(" "))
}

/// Draw PR detail view
pub fn draw_pr_detail_view(
    f: &mut Frame,
    area: Rect,
    pr: &PullRequestDetail,
    scroll: u16,
) {
    let mut lines = vec![];

    // Header
    lines.push(Line::from(vec![
        Span::styled("PR #", Style::default().fg(Color::Cyan)),
        Span::styled(
            pr.number.to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": "),
        Span::styled(
            pr.title.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));

    // Branch info
    lines.push(Line::from(vec![
        Span::styled("Branch: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(&pr.head_ref, Style::default().fg(Color::Yellow)),
        Span::raw(" → "),
        Span::styled(&pr.base_ref, Style::default().fg(Color::Green)),
    ]));

    // Status
    let (status_text, status_style) = if pr.draft {
        ("Draft", Style::default().fg(Color::DarkGray))
    } else if pr.state.to_lowercase().contains("merged") {
        ("Merged", Style::default().fg(Color::Magenta))
    } else if pr.state.to_lowercase().contains("closed") {
        ("Closed", Style::default().fg(Color::Red))
    } else {
        ("Open", Style::default().fg(Color::Green))
    };
    lines.push(Line::from(vec![
        Span::styled("Status: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(status_text, status_style),
    ]));

    // Author
    lines.push(Line::from(vec![
        Span::styled("Author: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(format!("@{}", pr.author), Style::default().fg(Color::Magenta)),
    ]));

    // Mergeable status
    if let Some(mergeable) = pr.mergeable {
        let (text, style) = if mergeable {
            ("Yes", Style::default().fg(Color::Green))
        } else {
            ("No (conflicts)", Style::default().fg(Color::Red))
        };
        lines.push(Line::from(vec![
            Span::styled("Mergeable: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(text, style),
        ]));
    }

    lines.push(Line::from(""));

    // Body
    if let Some(body) = &pr.body {
        lines.push(Line::from(Span::styled(
            "Description:",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
        )));
        lines.push(Line::from(""));
        let parsed_body = parse_markdown_content(body);
        for content_line in parsed_body.lines() {
            lines.push(render_markdown_line(content_line));
        }
        lines.push(Line::from(""));
    }

    // Comments
    if !pr.comments.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("Comments ({}):", pr.comments.len()),
            Style::default()
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
        )));
        lines.push(Line::from(""));

        for comment in &pr.comments {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("@{}", comment.author),
                    Style::default().fg(Color::Magenta),
                ),
                Span::styled(
                    format!(" • {}", format_date(&comment.created_at)),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
            let parsed_comment = parse_markdown_content(&comment.body);
            for content_line in parsed_comment.lines() {
                lines.push(render_markdown_line(content_line));
            }
            lines.push(Line::from(""));
        }
    }

    let title = " PR Detail │ o:browser │ m:merge │ r:review │ Esc:back ";
    let text = Text::from(lines);
    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));

    f.render_widget(paragraph, area);
}

/// Calculate centered rectangle for popups
fn centered_rect(percent_x: u16, percent_y: u16, outer: Rect) -> Rect {
    let popup_width = (outer.width * percent_x / 100).max(20).min(outer.width.saturating_sub(4));
    let popup_height = (outer.height * percent_y / 100).max(5).min(outer.height.saturating_sub(4));
    let popup_x = (outer.width.saturating_sub(popup_width)) / 2;
    let popup_y = (outer.height.saturating_sub(popup_height)) / 2;
    Rect::new(popup_x, popup_y, popup_width, popup_height)
}

/// Draw merge confirmation popup
fn draw_confirm_merge_popup(f: &mut Frame, pr: &PullRequestDetail) {
    let area = centered_rect(60, 30, f.area());

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Confirm Merge ")
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(area);
    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(block, area);

    let text = vec![
        Line::from(""),
        Line::from(format!("Merge PR #{}?", pr.number)),
        Line::from(""),
        Line::from(pr.title.clone()),
        Line::from(""),
        Line::from(vec![
            Span::styled(&pr.head_ref, Style::default().fg(Color::Yellow)),
            Span::raw(" → "),
            Span::styled(&pr.base_ref, Style::default().fg(Color::Green)),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled("y", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(": Yes, merge │ "),
            Span::styled("n", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw(": No, cancel"),
        ]),
    ];

    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, inner);
}

/// Draw PR review instructions popup
fn draw_pr_review_popup(f: &mut Frame, pr: &PullRequestDetail, input: &str) {
    let area = centered_rect(70, 50, f.area());

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Review PR #{}: {} ", pr.number, pr.title))
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(area);
    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .split(inner);

    // Instructions header
    let header = Paragraph::new("The agent will review this PR for code quality, bugs, and security issues.")
        .style(Style::default().fg(Color::DarkGray))
        .wrap(Wrap { trim: false });
    f.render_widget(header, chunks[0]);

    // Input area
    let display_text = if input.is_empty() {
        "Optional: add specific review instructions...".to_string()
    } else {
        format!("{}_", input)
    };

    let style = if input.is_empty() {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };

    let input_paragraph = Paragraph::new(display_text)
        .style(style)
        .wrap(Wrap { trim: false });
    f.render_widget(input_paragraph, chunks[1]);

    // Hint
    let hint = Paragraph::new("Enter: start review │ Esc: cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(hint, chunks[2]);
}

/// Draw PR filters popup
fn draw_pr_filters_popup(
    f: &mut Frame,
    status_filter: &HashSet<PrStatus>,
    author_filter: &HashSet<String>,
    _available_authors: &[String],
    focus: &PrFilterFocus,
    selected_status: usize,
    selected_author: usize,
    author_input: &str,
    author_suggestions: &[String],
) {
    let area = centered_rect(50, 60, f.area());

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" PR Filters ")
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(area);
    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(6),
        Constraint::Length(1),
        Constraint::Length(1), // Author input field
        Constraint::Min(5),    // Author suggestions/selected
        Constraint::Length(1),
    ])
    .split(inner);

    // Status section header
    let status_header_style = if *focus == PrFilterFocus::Status {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let status_header = Paragraph::new("Status:")
        .style(status_header_style);
    f.render_widget(status_header, chunks[0]);

    // Status options
    let status_items: Vec<ListItem> = PrStatus::all()
        .iter()
        .enumerate()
        .map(|(i, status)| {
            let checked = if status_filter.contains(status) { "[x]" } else { "[ ]" };
            let style = if *focus == PrFilterFocus::Status && i == selected_status {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };
            ListItem::new(format!("{} {}", checked, status.label())).style(style)
        })
        .collect();

    let status_list = List::new(status_items);
    f.render_widget(status_list, chunks[1]);

    // Author section header
    let author_header_style = if *focus == PrFilterFocus::Author {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let author_header = Paragraph::new("Author:")
        .style(author_header_style);
    f.render_widget(author_header, chunks[2]);

    // Author input field
    let input_style = if *focus == PrFilterFocus::Author {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let input_text = if author_input.is_empty() && *focus == PrFilterFocus::Author {
        "Type to search or add author...".to_string()
    } else if author_input.is_empty() {
        "".to_string()
    } else {
        format!("@{}_", author_input)
    };
    let input_paragraph = Paragraph::new(input_text)
        .style(if author_input.is_empty() && *focus == PrFilterFocus::Author {
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)
        } else {
            input_style
        });
    f.render_widget(input_paragraph, chunks[3]);

    // Author list: show suggestions if typing, otherwise show selected authors
    let author_items: Vec<ListItem> = if !author_input.is_empty() {
        // Show suggestions
        author_suggestions
            .iter()
            .enumerate()
            .map(|(i, author)| {
                let checked = if author_filter.contains(author) { "[x]" } else { "[ ]" };
                let style = if *focus == PrFilterFocus::Author && i == selected_author {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                };
                ListItem::new(format!("{} @{}", checked, author)).style(style)
            })
            .collect()
    } else {
        // Show selected authors + hint to add more
        let mut items: Vec<ListItem> = author_filter
            .iter()
            .enumerate()
            .map(|(i, author)| {
                let style = if *focus == PrFilterFocus::Author && i == selected_author {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                };
                ListItem::new(format!("[x] @{}", author)).style(style)
            })
            .collect();
        if items.is_empty() {
            items.push(ListItem::new("No authors selected").style(Style::default().fg(Color::DarkGray)));
        }
        items
    };

    let author_list = List::new(author_items);
    f.render_widget(author_list, chunks[4]);

    // Hint
    let hint = Paragraph::new("Tab: switch │ Space: toggle │ Enter: add/apply │ Esc: cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(hint, chunks[5]);
}

/// Draw issue filters popup
fn draw_issue_filters_popup(
    f: &mut Frame,
    status_filter: &HashSet<IssueStatus>,
    author_filter: &HashSet<String>,
    _available_authors: &[String],
    focus: &IssueFilterFocus,
    selected_status: usize,
    selected_author: usize,
    author_input: &str,
    author_suggestions: &[String],
) {
    let area = centered_rect(50, 60, f.area());

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Issue Filters ")
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(area);
    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(4), // Status has only 2 options
        Constraint::Length(1),
        Constraint::Length(1), // Author input field
        Constraint::Min(5),    // Author suggestions/selected
        Constraint::Length(1),
    ])
    .split(inner);

    // Status section header
    let status_header_style = if *focus == IssueFilterFocus::Status {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let status_header = Paragraph::new("Status:")
        .style(status_header_style);
    f.render_widget(status_header, chunks[0]);

    // Status options
    let status_items: Vec<ListItem> = IssueStatus::all()
        .iter()
        .enumerate()
        .map(|(i, status)| {
            let checked = if status_filter.contains(status) { "[x]" } else { "[ ]" };
            let style = if *focus == IssueFilterFocus::Status && i == selected_status {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };
            ListItem::new(format!("{} {}", checked, status.label())).style(style)
        })
        .collect();

    let status_list = List::new(status_items);
    f.render_widget(status_list, chunks[1]);

    // Author section header
    let author_header_style = if *focus == IssueFilterFocus::Author {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let author_header = Paragraph::new("Author:")
        .style(author_header_style);
    f.render_widget(author_header, chunks[2]);

    // Author input field
    let input_style = if *focus == IssueFilterFocus::Author {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let input_text = if author_input.is_empty() && *focus == IssueFilterFocus::Author {
        "Type to search or add author...".to_string()
    } else if author_input.is_empty() {
        "".to_string()
    } else {
        format!("@{}_", author_input)
    };
    let input_paragraph = Paragraph::new(input_text)
        .style(if author_input.is_empty() && *focus == IssueFilterFocus::Author {
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)
        } else {
            input_style
        });
    f.render_widget(input_paragraph, chunks[3]);

    // Author list: show suggestions if typing, otherwise show selected authors
    let author_items: Vec<ListItem> = if !author_input.is_empty() {
        // Show suggestions
        author_suggestions
            .iter()
            .enumerate()
            .map(|(i, author)| {
                let checked = if author_filter.contains(author) { "[x]" } else { "[ ]" };
                let style = if *focus == IssueFilterFocus::Author && i == selected_author {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                };
                ListItem::new(format!("{} @{}", checked, author)).style(style)
            })
            .collect()
    } else {
        // Show selected authors + hint to add more
        let mut items: Vec<ListItem> = author_filter
            .iter()
            .enumerate()
            .map(|(i, author)| {
                let style = if *focus == IssueFilterFocus::Author && i == selected_author {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                };
                ListItem::new(format!("[x] @{}", author)).style(style)
            })
            .collect();
        if items.is_empty() {
            items.push(ListItem::new("No authors selected").style(Style::default().fg(Color::DarkGray)));
        }
        items
    };

    let author_list = List::new(author_items);
    f.render_widget(author_list, chunks[4]);

    // Hint
    let hint = Paragraph::new("Tab: switch │ Space: toggle │ Enter: add/apply │ Esc: cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(hint, chunks[5]);
}
