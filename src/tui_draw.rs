//! TUI rendering functions.

use crate::github::IssueDetail;
use crate::issues::IssueContent;
use crate::markdown::{parse_markdown_content, render_markdown_line};
use crate::tui_types::{CommandSuggestion, CreateStage, TuiView};
use crate::tui_utils::format_date;

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
            let chunks =
                Layout::vertical([Constraint::Min(3), Constraint::Length(3)]).split(f.area());

            let input_clone = input.clone();
            draw_list_view_in_area(f, browser, chunks[0]);
            draw_search_input(f, chunks[1], &input_clone);
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
        TuiView::Help => {
            draw_help(f);
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

    format!(
        " {} │ C ai │ N new │ d dispatch │ o ide │ p pr │ t tmux │ R refresh │ / cmd │ ? help │ q quit ",
        parts.join(" ")
    )
}

/// Draw search input field
pub fn draw_search_input(f: &mut Frame, area: Rect, input: &str) {
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

/// Draw agent diff view
pub fn draw_agent_diff(f: &mut Frame, session_id: &str, content: &str, scroll: u16) {
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

/// Draw embedded tmux terminal view
pub fn draw_embedded_tmux(
    f: &mut Frame,
    browser: &IssueBrowser,
    available_sessions: &[String],
    current_index: usize,
) {
    let area = f.area();

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

            let name_style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else if wt.has_session {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let orphan_indicator = if !wt.has_session {
                Span::styled(" (orphaned)", Style::default().fg(Color::Red))
            } else {
                Span::raw("")
            };

            let line = Line::from(vec![
                status_icon,
                Span::styled(
                    issue_str,
                    Style::default().fg(if wt.has_session {
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
    let help = Paragraph::new("o: open IDE │ p: create PR │ d: delete │ K: kill tmux │ Esc: back")
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

/// Draw help screen with all keyboard shortcuts
pub fn draw_help(f: &mut Frame) {
    let area = f.area();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Keyboard Shortcuts │ Esc/q/? to close ")
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let help_text = vec![
        Line::from(vec![
            Span::styled("LIST VIEW", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Navigation", Style::default().fg(Color::Cyan)),
        ]),
        Line::from("    j/↓       Move down"),
        Line::from("    k/↑       Move up"),
        Line::from("    Enter     Open issue details"),
        Line::from("    s         Search issues"),
        Line::from("    /         Open command palette"),
        Line::from("    q         Quit"),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Issues", Style::default().fg(Color::Cyan)),
        ]),
        Line::from("    C         Create issue with AI"),
        Line::from("    N         Create issue (direct)"),
        Line::from("    c         Add comment"),
        Line::from("    Space     Select/deselect issue"),
        Line::from("    R         Refresh issues"),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Agent / Worktree", Style::default().fg(Color::Cyan)),
        ]),
        Line::from("    d         Dispatch agent"),
        Line::from("    o         Open worktree in IDE"),
        Line::from("    p         Create PR"),
        Line::from("    l         View agent logs"),
        Line::from("    D         View agent diff"),
        Line::from("    K         Kill running agent"),
        Line::from("    W         Delete worktree"),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Tmux", Style::default().fg(Color::Cyan)),
        ]),
        Line::from("    t         Open tmux for issue"),
        Line::from("    T         Embedded tmux terminal"),
        Line::from(""),
        Line::from(vec![
            Span::styled("DETAIL VIEW", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from("    o         Open in browser"),
        Line::from("    c         Add comment"),
        Line::from("    a         Assign user"),
        Line::from("    x         Close issue"),
        Line::from("    X         Reopen issue"),
        Line::from("    d         Dispatch agent"),
        Line::from("    i/O       Navigate images"),
        Line::from("    Esc       Back to list"),
    ];

    let paragraph = Paragraph::new(help_text).scroll((0, 0));
    f.render_widget(paragraph, inner);
}
