//! Help text and status bar rendering utilities.

use std::collections::HashMap;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::registry::CommandRegistry;
use super::shortcuts::Shortcut;
use super::types::{CommandCategory, CommandContext};

/// Generate help lines for a specific context.
pub fn help_lines_for_context(context: CommandContext) -> Vec<Line<'static>> {
    let shortcuts = CommandRegistry::shortcuts_for_context(context);

    // Group by category
    let mut by_category: HashMap<CommandCategory, Vec<Shortcut>> = HashMap::new();
    for shortcut in shortcuts {
        by_category
            .entry(shortcut.category())
            .or_default()
            .push(shortcut);
    }

    let mut lines = Vec::new();

    // Sort categories by order
    let mut categories: Vec<_> = by_category.keys().copied().collect();
    categories.sort_by_key(|c| c.order());

    for category in categories {
        if let Some(shortcuts) = by_category.get(&category) {
            // Category header
            lines.push(Line::from(vec![Span::styled(
                format!("  {}", category.display_name()),
                Style::default().fg(Color::Cyan),
            )]));

            // Shortcuts in this category
            for shortcut in shortcuts {
                lines.push(Line::from(format!(
                    "    {:12} {}",
                    shortcut.key_display(),
                    shortcut.description()
                )));
            }

            lines.push(Line::from(""));
        }
    }

    lines
}

/// Generate full help screen content for all main views.
pub fn generate_full_help() -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Issue List section
    lines.push(section_header("ISSUE LIST"));
    lines.push(Line::from(""));
    lines.extend(help_lines_for_context(CommandContext::IssueList));

    // Detail View section
    lines.push(section_header("DETAIL VIEW"));
    lines.push(Line::from(""));
    lines.extend(help_lines_for_context(CommandContext::IssueDetail));

    // Worktree List section
    lines.push(section_header("WORKTREE LIST"));
    lines.push(Line::from(""));
    lines.extend(help_lines_for_context(CommandContext::WorktreeList));

    // PR List section
    lines.push(section_header("PULL REQUEST LIST"));
    lines.push(Line::from(""));
    lines.extend(help_lines_for_context(CommandContext::PullRequestList));

    // Embedded Terminal section
    lines.push(section_header("EMBEDDED TERMINAL"));
    lines.push(Line::from(""));
    lines.extend(help_lines_for_context(CommandContext::EmbeddedTmux));

    lines
}

fn section_header(title: &'static str) -> Line<'static> {
    Line::from(vec![Span::styled(
        format!("─── {} ───", title),
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )])
}

/// Shortcuts to display in the status bar for a given context.
pub fn status_bar_shortcuts(context: CommandContext) -> &'static [Shortcut] {
    match context {
        CommandContext::IssueList => &[
            Shortcut::CreateIssueAI,
            Shortcut::CreateIssueDirect,
            Shortcut::DispatchAgent,
            Shortcut::OpenIDE,
            Shortcut::CreatePR,
            Shortcut::OpenTmux,
            Shortcut::Refresh,
            Shortcut::OpenFilters,
            Shortcut::OpenCommandPalette,
            Shortcut::OpenHelp,
            Shortcut::Quit,
            Shortcut::SwitchToPRs,
        ],
        CommandContext::IssueDetail => &[
            Shortcut::OpenInBrowser,
            Shortcut::AddComment,
            Shortcut::AssignUser,
            Shortcut::DispatchAgent,
            Shortcut::CloseIssue,
            Shortcut::GoBack,
        ],
        CommandContext::WorktreeList => &[
            Shortcut::OpenIDE,
            Shortcut::CreatePR,
            Shortcut::OpenTmux,
            Shortcut::DeleteWorktree,
            Shortcut::KillAgent,
            Shortcut::CreateWorktree,
            Shortcut::GoBack,
        ],
        CommandContext::PullRequestList => &[
            Shortcut::OpenInBrowser,
            Shortcut::CheckoutBranch,
            Shortcut::ReviewPR,
            Shortcut::MergePR,
            Shortcut::OpenFilters,
            Shortcut::OpenHelp,
            Shortcut::SwitchToIssues,
        ],
        CommandContext::PullRequestDetail => &[
            Shortcut::OpenInBrowser,
            Shortcut::ReviewPR,
            Shortcut::MergePR,
            Shortcut::GoBack,
        ],
        CommandContext::EmbeddedTmux => &[
            Shortcut::ExitTerminal,
            Shortcut::PrevSession,
            Shortcut::NextSession,
        ],
        CommandContext::Global => &[],
    }
}

/// Format status bar string from shortcuts.
pub fn format_status_bar(context: CommandContext, prefix: &str) -> String {
    let hints: Vec<String> = status_bar_shortcuts(context)
        .iter()
        .map(|s| format!("{} {}", s.key_display(), s.short_desc()))
        .collect();

    if prefix.is_empty() {
        format!(" {} ", hints.join(" \u{2502} "))
    } else {
        format!(" {} \u{2502} {} ", prefix, hints.join(" \u{2502} "))
    }
}
