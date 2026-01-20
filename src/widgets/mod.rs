//! Composable widgets for the TUI.
//!
//! This module provides reusable widget components that encapsulate
//! both state and rendering logic, making the UI code more modular
//! and easier to maintain.

mod confirm_dialog;
mod input_field;
mod popup;
mod status_bar;

pub use confirm_dialog::ConfirmDialog;
pub use input_field::InputField;
pub use popup::Popup;
pub use status_bar::StatusBar;
