//! Common helpers for TUI event handling.

use crate::tui::IssueBrowser;
use crate::tui_types::{CommandSuggestion, CreateStage, TuiView};

/// Filter commands based on input
pub fn filter_commands(commands: &[CommandSuggestion], input: &str) -> Vec<CommandSuggestion> {
    if input.is_empty() {
        commands.to_vec()
    } else {
        let input_lower = input.to_lowercase();
        commands
            .iter()
            .filter(|cmd| cmd.name.to_lowercase().contains(&input_lower))
            .cloned()
            .collect()
    }
}

/// Handle pasted content into input fields
pub fn handle_paste(browser: &mut IssueBrowser, content: &str) {
    let clean_content = content.replace('\r', "");

    match &mut browser.view {
        TuiView::Search { input } => {
            input.push_str(&clean_content.replace('\n', " "));
        }
        TuiView::Command {
            input,
            suggestions,
            selected,
        } => {
            input.push_str(&clean_content.replace('\n', " "));
            let input_clone = input.clone();
            let available = browser.available_commands.clone();
            *suggestions = filter_commands(&available, &input_clone);
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
        TuiView::DirectIssue {
            title,
            body,
            editing_body,
        } => {
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
        TuiView::DispatchInstructions { input, .. } => {
            input.push_str(&clean_content);
        }
        TuiView::WorktreeAgentInstructions { input, .. } => {
            input.push_str(&clean_content);
        }
        TuiView::DispatchPrReview { input, .. } => {
            input.push_str(&clean_content);
        }
        _ => {}
    }
}
