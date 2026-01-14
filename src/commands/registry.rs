//! Command registry for centralized access.

use std::collections::HashMap;

use super::shortcuts::Shortcut;
use super::slash::SlashCommand;
use super::types::CommandContext;
use crate::tui_types::CommandSuggestion;

/// Central registry for all commands.
pub struct CommandRegistry {
    /// Custom commands from project config.
    custom_commands: Vec<SlashCommand>,
}

impl CommandRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            custom_commands: Vec::new(),
        }
    }

    /// Create a registry with custom commands from project config.
    pub fn with_custom_commands(list_commands: &HashMap<String, Vec<String>>) -> Self {
        let custom = list_commands
            .iter()
            .map(|(name, labels)| SlashCommand::Custom {
                name: name.clone(),
                labels: labels.clone(),
            })
            .collect();

        Self {
            custom_commands: custom,
        }
    }

    /// Get all slash commands (built-in + custom).
    pub fn all_slash_commands(&self) -> Vec<SlashCommand> {
        let mut commands = SlashCommand::builtins();
        commands.extend(self.custom_commands.clone());
        commands
    }

    /// Convert to CommandSuggestion for backward compatibility with existing code.
    pub fn to_suggestions(&self) -> Vec<CommandSuggestion> {
        self.all_slash_commands()
            .into_iter()
            .map(|cmd| CommandSuggestion {
                name: cmd.name().to_string(),
                description: cmd.description(),
                labels: cmd.labels().cloned(),
            })
            .collect()
    }

    /// Get all shortcuts available in a given context.
    pub fn shortcuts_for_context(context: CommandContext) -> Vec<Shortcut> {
        Shortcut::all()
            .into_iter()
            .filter(|s| s.contexts().contains(&context) || s.contexts().contains(&CommandContext::Global))
            .collect()
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}
