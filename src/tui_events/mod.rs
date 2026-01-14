//! TUI event handling functions.
//!
//! This module organizes event handlers by view type:
//! - `list`: Issue list navigation and actions
//! - `detail`: Issue detail view and related dialogs
//! - `search`: Search view
//! - `command`: Command palette
//! - `create`: Issue creation views
//! - `dispatch`: Agent dispatch instructions
//! - `worktree`: Worktree management views
//! - `pr`: Pull request views
//! - `filters`: Filter dialogs
//! - `agents`: Agent logs and selection
//! - `project`: Project selection
//! - `embedded`: Embedded tmux terminal
//! - `help`: Help view
//! - `common`: Shared utilities

mod agents;
mod command;
mod common;
mod create;
mod detail;
mod dispatch;
mod embedded;
mod filters;
mod help;
mod list;
mod pr;
mod project;
mod search;
mod worktree;

pub use common::{filter_commands, handle_paste};

use crate::tui::IssueBrowser;
use crate::tui_types::TuiView;
use crossterm::event::{KeyCode, KeyModifiers};

/// Handle keyboard events
pub async fn handle_key_event(browser: &mut IssueBrowser, key: KeyCode, modifiers: KeyModifiers) {
    // Clear status message on any keypress (except ESC for double-ESC logic)
    if key != KeyCode::Esc {
        browser.status_message = None;
        browser.last_esc_press = None;
    }

    match &mut browser.view {
        TuiView::List => {
            list::handle_list_key(browser, key).await;
        }

        TuiView::Help => {
            help::handle_help_key(browser, key);
        }

        TuiView::Search { input } => {
            let mut input = input.clone();
            search::handle_search_key(browser, key, &mut input).await;
            if let TuiView::Search { input: ref mut i } = browser.view {
                *i = input;
            }
        }

        TuiView::Detail(issue) => {
            let issue = issue.clone();
            detail::handle_detail_key(browser, key, &issue).await;
        }

        TuiView::AddComment { issue, input } => {
            let issue = issue.clone();
            let mut input = input.clone();
            detail::handle_add_comment_key(browser, key, &issue, &mut input).await;
            if let TuiView::AddComment {
                input: ref mut i, ..
            } = browser.view
            {
                *i = input;
            }
        }

        TuiView::ConfirmClose { issue } => {
            let issue = issue.clone();
            detail::handle_confirm_close_key(browser, key, &issue).await;
        }

        TuiView::ConfirmReopen { issue } => {
            let issue = issue.clone();
            detail::handle_confirm_reopen_key(browser, key, &issue).await;
        }

        TuiView::AssignUser {
            issue,
            input,
            suggestions,
            selected,
        } => {
            let issue = issue.clone();
            let input = input.clone();
            let suggestions = suggestions.clone();
            let selected = *selected;
            detail::handle_assign_user_key(browser, key, &issue, &input, &suggestions, selected)
                .await;
        }

        TuiView::ConfirmDispatch { issue } => {
            let issue = issue.clone();
            detail::handle_confirm_dispatch_key(browser, key, &issue).await;
        }

        TuiView::DispatchInstructions { issue, input } => {
            let issue = issue.clone();
            let mut input = input.clone();
            dispatch::handle_dispatch_instructions_key(browser, key, modifiers, &issue, &mut input)
                .await;
            if let TuiView::DispatchInstructions {
                input: ref mut i, ..
            } = browser.view
            {
                *i = input;
            }
        }

        TuiView::WorktreeAgentInstructions {
            worktree_path,
            branch_name,
            input,
        } => {
            let worktree_path = worktree_path.clone();
            let branch_name = branch_name.clone();
            let mut input = input.clone();
            dispatch::handle_worktree_agent_instructions_key(
                browser,
                key,
                modifiers,
                &worktree_path,
                &branch_name,
                &mut input,
            )
            .await;
            if let TuiView::WorktreeAgentInstructions {
                input: ref mut i, ..
            } = browser.view
            {
                *i = input;
            }
        }

        TuiView::AgentLogs { scroll, .. } => {
            let mut scroll = *scroll;
            agents::handle_agent_logs_key(browser, key, &mut scroll);
            if let TuiView::AgentLogs {
                scroll: ref mut s, ..
            } = browser.view
            {
                *s = scroll;
            }
        }

        TuiView::AgentSelect { selected } => {
            let mut selected = *selected;
            agents::handle_agent_select_key(browser, key, &mut selected);
            if let TuiView::AgentSelect {
                selected: ref mut s,
            } = browser.view
            {
                *s = selected;
            }
        }

        TuiView::ProjectSelect { projects, selected } => {
            let projects = projects.clone();
            let mut selected = *selected;
            project::handle_project_select_key(browser, key, &projects, &mut selected).await;
            if let TuiView::ProjectSelect {
                selected: ref mut s,
                ..
            } = browser.view
            {
                *s = selected;
            }
        }

        TuiView::Command {
            input,
            suggestions,
            selected,
        } => {
            let mut input = input.clone();
            let mut suggestions = suggestions.clone();
            let mut selected = *selected;
            command::handle_command_key(
                browser,
                key,
                &mut input,
                &mut suggestions,
                &mut selected,
            )
            .await;
            if let TuiView::Command {
                input: ref mut i,
                suggestions: ref mut s,
                selected: ref mut sel,
            } = browser.view
            {
                *i = input;
                *s = suggestions;
                *sel = selected;
            }
        }

        TuiView::CreateIssue { input, stage } => {
            let mut input = input.clone();
            let mut stage = stage.clone();
            create::handle_create_issue_key(browser, key, &mut input, &mut stage).await;
            if let TuiView::CreateIssue {
                input: ref mut i,
                stage: ref mut st,
            } = browser.view
            {
                *i = input;
                *st = stage;
            }
        }

        TuiView::PreviewIssue {
            issue,
            messages,
            feedback_input,
            scroll,
        } => {
            let mut issue = issue.clone();
            let mut messages = messages.clone();
            let mut feedback_input = feedback_input.clone();
            let mut scroll = *scroll;
            create::handle_preview_issue_key(
                browser,
                key,
                &mut issue,
                &mut messages,
                &mut feedback_input,
                &mut scroll,
            )
            .await;
            if let TuiView::PreviewIssue {
                issue: ref mut is,
                messages: ref mut m,
                feedback_input: ref mut f,
                scroll: ref mut s,
            } = browser.view
            {
                *is = issue;
                *m = messages;
                *f = feedback_input;
                *s = scroll;
            }
        }

        TuiView::DirectIssue {
            title,
            body,
            editing_body,
        } => {
            let mut title = title.clone();
            let mut body = body.clone();
            let mut editing_body = *editing_body;
            create::handle_direct_issue_key(
                browser,
                key,
                modifiers,
                &mut title,
                &mut body,
                &mut editing_body,
            )
            .await;
            if let TuiView::DirectIssue {
                title: ref mut t,
                body: ref mut b,
                editing_body: ref mut e,
            } = browser.view
            {
                *t = title;
                *b = body;
                *e = editing_body;
            }
        }

        TuiView::WorktreeList {
            worktrees,
            selected,
        } => {
            let worktrees_clone = worktrees.clone();
            let mut selected = *selected;
            worktree::handle_worktree_list_key(browser, key, &worktrees_clone, &mut selected);
            if let TuiView::WorktreeList {
                selected: ref mut s,
                ..
            } = browser.view
            {
                *s = selected;
            }
        }

        TuiView::CreateWorktree { input } => {
            let mut input = input.clone();
            worktree::handle_create_worktree_key(browser, key, &mut input);
            if let TuiView::CreateWorktree { input: ref mut i } = browser.view {
                *i = input;
            }
        }

        TuiView::PostWorktreeCreate {
            worktree_path,
            branch_name,
        } => {
            let worktree_path = worktree_path.clone();
            let branch_name = branch_name.clone();
            worktree::handle_post_worktree_create_key(browser, key, &worktree_path, &branch_name);
        }

        TuiView::ConfirmPrune { orphaned } => {
            let orphaned = orphaned.clone();
            worktree::handle_confirm_prune_key(browser, key, &orphaned);
        }

        TuiView::ConfirmDeleteWorktree {
            worktree,
            return_index,
        } => {
            let worktree = worktree.clone();
            let return_index = *return_index;
            worktree::handle_confirm_delete_worktree_key(browser, key, &worktree, return_index);
        }

        TuiView::EmbeddedTmux {
            available_sessions,
            current_index,
            return_to_worktrees,
        } => {
            let mut available_sessions = available_sessions.clone();
            let mut current_index = *current_index;
            let return_to_worktrees = *return_to_worktrees;
            embedded::handle_embedded_tmux_key(
                browser,
                key,
                modifiers,
                &mut available_sessions,
                &mut current_index,
                return_to_worktrees,
            );
            if let TuiView::EmbeddedTmux {
                available_sessions: ref mut a,
                current_index: ref mut c,
                ..
            } = browser.view
            {
                *a = available_sessions;
                *c = current_index;
            }
        }

        TuiView::PullRequestList => {
            pr::handle_pr_list_key(browser, key).await;
        }

        TuiView::PullRequestDetail(pr_detail) => {
            let pr_detail = pr_detail.clone();
            pr::handle_pr_detail_key(browser, key, &pr_detail);
        }

        TuiView::ConfirmMerge { pr: pr_detail } => {
            let pr_detail = pr_detail.clone();
            pr::handle_confirm_merge_key(browser, key, &pr_detail).await;
        }

        TuiView::DispatchPrReview {
            pr: pr_detail,
            input,
        } => {
            let pr_detail = pr_detail.clone();
            let mut input = input.clone();
            pr::handle_dispatch_pr_review_key(browser, key, &pr_detail, &mut input).await;
            if let TuiView::DispatchPrReview {
                input: ref mut i, ..
            } = browser.view
            {
                *i = input;
            }
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
            let mut status_filter = status_filter.clone();
            let mut author_filter = author_filter.clone();
            let available_authors = available_authors.clone();
            let mut focus = *focus;
            let mut selected_status = *selected_status;
            let mut selected_author = *selected_author;
            let mut author_input = author_input.clone();
            let mut author_suggestions = author_suggestions.clone();

            let old_author_filter = browser.pr_author_filter.clone();

            filters::handle_pr_filters_key(
                browser,
                key,
                &mut status_filter,
                &mut author_filter,
                &available_authors,
                &mut focus,
                &mut selected_status,
                &mut selected_author,
                &mut author_input,
                &mut author_suggestions,
            );

            if matches!(browser.view, TuiView::PullRequestList)
                && browser.pr_author_filter != old_author_filter
            {
                browser.reload_pull_requests().await;
            }
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
            let mut status_filter = status_filter.clone();
            let mut author_filter = author_filter.clone();
            let available_authors = available_authors.clone();
            let mut focus = *focus;
            let mut selected_status = *selected_status;
            let mut selected_author = *selected_author;
            let mut author_input = author_input.clone();
            let mut author_suggestions = author_suggestions.clone();

            let old_author_filter = browser.issue_author_filter.clone();

            filters::handle_issue_filters_key(
                browser,
                key,
                &mut status_filter,
                &mut author_filter,
                &available_authors,
                &mut focus,
                &mut selected_status,
                &mut selected_author,
                &mut author_input,
                &mut author_suggestions,
            );

            if matches!(browser.view, TuiView::List)
                && browser.issue_author_filter != old_author_filter
            {
                browser.reload_issues().await;
            }
        }
    }
}
