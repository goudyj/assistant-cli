//! Composable widgets for the TUI.
//!
//! This module provides reusable widget components that encapsulate
//! both state and rendering logic, making the UI code more modular
//! and easier to maintain.

mod command_palette;
mod confirm_dialog;
mod filter_popup;
mod input_field;
pub mod issue_list;
mod popup;
mod status_bar;

pub use command_palette::{build_command_item, render_command_palette, CommandPaletteConfig, Separator};
pub use confirm_dialog::ConfirmDialog;
pub use filter_popup::{render_filter_popup, FilterFocus, FilterOption, FilterPopupConfig};
#[allow(unused_imports)]
pub use input_field::InlineInput;
pub use input_field::InputField;
pub use issue_list::{build_issue_item, build_issue_item_dimmed, build_issue_list_title, IssueListTitleConfig};
pub use popup::Popup;
pub use status_bar::StatusBar;
