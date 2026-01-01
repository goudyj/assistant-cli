//! Git worktree management for isolated issue processing.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::worktrees_dir;

/// Error types for worktree operations
#[derive(Debug)]
pub enum WorktreeError {
    GitError(String),
    IoError(std::io::Error),
}

impl std::fmt::Display for WorktreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorktreeError::GitError(msg) => write!(f, "Git error: {}", msg),
            WorktreeError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for WorktreeError {}

impl From<std::io::Error> for WorktreeError {
    fn from(e: std::io::Error) -> Self {
        WorktreeError::IoError(e)
    }
}

/// Create a git worktree for an issue.
///
/// This creates an isolated working directory for the agent to work in.
///
/// # Arguments
/// * `local_path` - Path to the main repository
/// * `project` - Project name
/// * `issue_number` - Issue number
///
/// # Returns
/// * The path to the created worktree and the branch name
pub fn create_worktree(
    local_path: &Path,
    project: &str,
    issue_number: u64,
) -> Result<(PathBuf, String), WorktreeError> {
    let branch_name = format!("issue-{}", issue_number);
    let worktree_name = format!("{}-{}", project, issue_number);
    let worktree_path = worktrees_dir().join(&worktree_name);

    // Ensure worktrees directory exists
    std::fs::create_dir_all(worktrees_dir())?;

    // Check if worktree already exists
    if worktree_path.exists() {
        // Worktree already exists, just return it
        return Ok((worktree_path, branch_name));
    }

    // Check if branch already exists
    let branch_exists = Command::new("git")
        .current_dir(local_path)
        .args(["rev-parse", "--verify", &branch_name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !branch_exists {
        // Create branch from HEAD
        let output = Command::new("git")
            .current_dir(local_path)
            .args(["branch", &branch_name, "HEAD"])
            .output()?;

        if !output.status.success() {
            return Err(WorktreeError::GitError(format!(
                "Failed to create branch: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
    }

    // Create the worktree
    let output = Command::new("git")
        .current_dir(local_path)
        .args([
            "worktree",
            "add",
            worktree_path.to_str().unwrap(),
            &branch_name,
        ])
        .output()?;

    if !output.status.success() {
        return Err(WorktreeError::GitError(format!(
            "Failed to create worktree: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    Ok((worktree_path, branch_name))
}

/// Remove a git worktree.
///
/// # Arguments
/// * `local_path` - Path to the main repository
/// * `worktree_path` - Path to the worktree to remove
/// * `remove_branch` - Whether to also remove the associated branch
pub fn remove_worktree(
    local_path: &Path,
    worktree_path: &Path,
    remove_branch: bool,
) -> Result<(), WorktreeError> {
    // Get branch name before removing worktree
    let branch_name = if remove_branch {
        // Extract branch name from worktree path
        let output = Command::new("git")
            .current_dir(worktree_path)
            .args(["branch", "--show-current"])
            .output()
            .ok();

        output.and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
    } else {
        None
    };

    // Remove the worktree
    let output = Command::new("git")
        .current_dir(local_path)
        .args([
            "worktree",
            "remove",
            worktree_path.to_str().unwrap(),
            "--force",
        ])
        .output()?;

    if !output.status.success() {
        // Try to remove directory manually if git worktree remove fails
        if worktree_path.exists() {
            std::fs::remove_dir_all(worktree_path)?;
        }
        // Prune orphaned worktree entries
        let _ = Command::new("git")
            .current_dir(local_path)
            .args(["worktree", "prune"])
            .output();
    }

    // Remove the branch if requested
    if let Some(branch) = branch_name
        && !branch.is_empty() && branch != "master" && branch != "main" {
            let _ = Command::new("git")
                .current_dir(local_path)
                .args(["branch", "-D", &branch])
                .output();
        }

    Ok(())
}

/// Get git diff stats for a worktree.
///
/// # Returns
/// * (lines_added, lines_deleted, files_changed)
pub fn get_diff_stats(worktree_path: &Path) -> (usize, usize, usize) {
    let output = Command::new("git")
        .current_dir(worktree_path)
        .args(["diff", "--numstat", "HEAD~1"])
        .output()
        .ok();

    if let Some(output) = output
        && output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut lines_added = 0;
            let mut lines_deleted = 0;
            let mut files_changed = 0;

            for line in stdout.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 2 {
                    files_changed += 1;
                    // Parts[0] is additions, parts[1] is deletions
                    if let Ok(added) = parts[0].parse::<usize>() {
                        lines_added += added;
                    }
                    if let Ok(deleted) = parts[1].parse::<usize>() {
                        lines_deleted += deleted;
                    }
                }
            }

            return (lines_added, lines_deleted, files_changed);
        }

    // Fallback: try diff from initial state
    let output = Command::new("git")
        .current_dir(worktree_path)
        .args(["diff", "--numstat"])
        .output()
        .ok();

    if let Some(output) = output
        && output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut lines_added = 0;
            let mut lines_deleted = 0;
            let mut files_changed = 0;

            for line in stdout.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 2 {
                    files_changed += 1;
                    if let Ok(added) = parts[0].parse::<usize>() {
                        lines_added += added;
                    }
                    if let Ok(deleted) = parts[1].parse::<usize>() {
                        lines_deleted += deleted;
                    }
                }
            }

            return (lines_added, lines_deleted, files_changed);
        }

    (0, 0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worktree_path_generation() {
        let path = worktrees_dir().join("test-project-123");
        assert!(path.to_str().unwrap().contains("worktrees"));
        assert!(path.to_str().unwrap().contains("test-project-123"));
    }

    // Integration tests would require actual git repos
    // and are skipped in unit tests
}
