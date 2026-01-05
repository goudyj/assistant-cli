use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub owner: String,
    pub repo: String,
    pub labels: Vec<String>,
    #[serde(default)]
    pub list_commands: HashMap<String, Vec<String>>,
    /// Local path to the repository for Claude Code integration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_path: Option<PathBuf>,
}

impl ProjectConfig {
    pub fn get_list_command_labels(&self, command_name: &str) -> Option<&Vec<String>> {
        self.list_commands.get(command_name)
    }

    pub fn list_command_names(&self) -> Vec<&String> {
        self.list_commands.keys().collect()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub github_client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_token: Option<String>,
    pub projects: HashMap<String, ProjectConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_project: Option<String>,
    #[serde(default)]
    pub auto_format_comments: bool,
}

#[derive(Debug)]
pub enum ConfigError {
    NotFound(PathBuf),
    InvalidJson(String),
    IoError(std::io::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::NotFound(path) => write!(f, "Config file not found: {}", path.display()),
            ConfigError::InvalidJson(msg) => write!(f, "Invalid JSON in config: {}", msg),
            ConfigError::IoError(e) => write!(f, "IO error reading config: {}", e),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        ConfigError::IoError(e)
    }
}

pub fn config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|p| p.join(".config").join("assistant.json"))
}

pub fn load_config() -> Result<Config, ConfigError> {
    let path = config_path().ok_or_else(|| {
        ConfigError::NotFound(PathBuf::from("~/.config/assistant.json"))
    })?;

    if !path.exists() {
        return Err(ConfigError::NotFound(path));
    }

    let content = fs::read_to_string(&path)?;
    serde_json::from_str(&content).map_err(|e| ConfigError::InvalidJson(e.to_string()))
}

impl Config {
    pub fn get_project(&self, name: &str) -> Option<&ProjectConfig> {
        self.projects.get(name)
    }

    pub fn list_projects(&self) -> Vec<&String> {
        self.projects.keys().collect()
    }

    pub fn set_last_project(&mut self, name: &str) {
        self.last_project = Some(name.to_string());
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let path = config_path().ok_or_else(|| {
            ConfigError::NotFound(PathBuf::from("~/.config/assistant.json"))
        })?;

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| ConfigError::InvalidJson(e.to_string()))?;

        fs::write(&path, content)?;
        Ok(())
    }

    pub fn set_token(&mut self, token: &str) {
        self.github_token = Some(token.to_string());
    }

    pub fn clear_token(&mut self) {
        self.github_token = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_config() {
        let json = r#"{
            "github_client_id": "Ov23liXXXXXX",
            "projects": {
                "test-project": {
                    "owner": "jean",
                    "repo": "test-repo",
                    "labels": ["bug", "feature"]
                }
            }
        }"#;

        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.github_client_id, Some("Ov23liXXXXXX".to_string()));
        assert_eq!(config.projects.len(), 1);

        let project = config.get_project("test-project").unwrap();
        assert_eq!(project.owner, "jean");
        assert_eq!(project.repo, "test-repo");
        assert_eq!(project.labels, vec!["bug", "feature"]);
    }

    #[test]
    fn deserialize_config_without_client_id() {
        let json = r#"{
            "projects": {
                "test-project": { "owner": "a", "repo": "ra", "labels": [] }
            }
        }"#;

        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.github_client_id, None);
    }

    #[test]
    fn list_projects() {
        let json = r#"{
            "projects": {
                "project-a": { "owner": "a", "repo": "ra", "labels": [] },
                "project-b": { "owner": "b", "repo": "rb", "labels": [] }
            }
        }"#;

        let config: Config = serde_json::from_str(json).unwrap();
        let projects = config.list_projects();
        assert_eq!(projects.len(), 2);
    }

    #[test]
    fn deserialize_config_with_list_commands() {
        let json = r#"{
            "auto_format_comments": true,
            "projects": {
                "test": {
                    "owner": "user",
                    "repo": "repo",
                    "labels": ["bug"],
                    "list_commands": {
                        "bugs": ["Bug"],
                        "customer": ["Bug", "customer"]
                    }
                }
            }
        }"#;

        let config: Config = serde_json::from_str(json).unwrap();
        assert!(config.auto_format_comments);

        let project = config.get_project("test").unwrap();
        assert_eq!(project.list_commands.len(), 2);
        assert_eq!(
            project.get_list_command_labels("bugs"),
            Some(&vec!["Bug".to_string()])
        );
        assert_eq!(
            project.get_list_command_labels("customer"),
            Some(&vec!["Bug".to_string(), "customer".to_string()])
        );
    }

    #[test]
    fn backward_compatible_without_list_commands() {
        let json = r#"{
            "projects": {
                "test": { "owner": "a", "repo": "r", "labels": [] }
            }
        }"#;

        let config: Config = serde_json::from_str(json).unwrap();
        assert!(!config.auto_format_comments);

        let project = config.get_project("test").unwrap();
        assert!(project.list_commands.is_empty());
    }
}
