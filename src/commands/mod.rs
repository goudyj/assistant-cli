//! Centralized command system.
//!
//! This module provides a single source of truth for all commands and shortcuts
//! in the application. It enables auto-generation of help text and status bar hints.
//!
//! # Architecture
//!
//! - `types`: Context and category enums
//! - `shortcuts`: Keyboard shortcuts definitions
//! - `slash`: Slash commands (command palette)
//! - `registry`: Central registry for all commands
//! - `render`: Help text and status bar generation

mod registry;
mod render;
mod shortcuts;
mod slash;
mod types;

pub use registry::CommandRegistry;
pub use render::{format_status_bar, generate_full_help, help_lines_for_context, status_bar_shortcuts};
pub use shortcuts::Shortcut;
pub use slash::SlashCommand;
pub use types::{CommandCategory, CommandContext};
