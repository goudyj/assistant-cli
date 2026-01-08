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
    // Verify local_path exists
    if !local_path.exists() {
        return Err(WorktreeError::GitError(format!(
            "Path does not exist: {}",
            local_path.display()
        )));
    }

    // Verify it's a git repository
    let is_git_repo = Command::new("git")
        .current_dir(local_path)
        .args(["rev-parse", "--git-dir"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !is_git_repo {
        return Err(WorktreeError::GitError(format!(
            "Not a git repository: {}. Run 'git init' or clone a repo.",
            local_path.display()
        )));
    }

    // Verify there's at least one commit
    let has_commits = Command::new("git")
        .current_dir(local_path)
        .args(["rev-parse", "HEAD"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !has_commits {
        return Err(WorktreeError::GitError(
            "Repository has no commits. Create at least one commit first.".to_string(),
        ));
    }

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

/// Create a git worktree with a custom branch name.
///
/// This creates an isolated working directory with a user-specified branch.
///
/// # Arguments
/// * `local_path` - Path to the main repository
/// * `project` - Project name
/// * `branch_name` - Custom branch name (e.g., "feature/dark-mode")
///
/// # Returns
/// * The path to the created worktree and the branch name
pub fn create_worktree_with_branch(
    local_path: &Path,
    project: &str,
    branch_name: &str,
) -> Result<(PathBuf, String), WorktreeError> {
    // Verify local_path exists
    if !local_path.exists() {
        return Err(WorktreeError::GitError(format!(
            "Path does not exist: {}",
            local_path.display()
        )));
    }

    // Verify it's a git repository
    let is_git_repo = Command::new("git")
        .current_dir(local_path)
        .args(["rev-parse", "--git-dir"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !is_git_repo {
        return Err(WorktreeError::GitError(format!(
            "Not a git repository: {}. Run 'git init' or clone a repo.",
            local_path.display()
        )));
    }

    // Verify there's at least one commit
    let has_commits = Command::new("git")
        .current_dir(local_path)
        .args(["rev-parse", "HEAD"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !has_commits {
        return Err(WorktreeError::GitError(
            "Repository has no commits. Create at least one commit first.".to_string(),
        ));
    }

    // Sanitize branch name for directory (replace / with -)
    let sanitized_name = branch_name.replace('/', "-");
    let worktree_name = format!("{}-{}", project, sanitized_name);
    let worktree_path = worktrees_dir().join(&worktree_name);

    // Ensure worktrees directory exists
    std::fs::create_dir_all(worktrees_dir())?;

    // Check if worktree already exists
    if worktree_path.exists() {
        return Ok((worktree_path, branch_name.to_string()));
    }

    // Check if branch already exists
    let branch_exists = Command::new("git")
        .current_dir(local_path)
        .args(["rev-parse", "--verify", branch_name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !branch_exists {
        // Create branch from HEAD
        let output = Command::new("git")
            .current_dir(local_path)
            .args(["branch", branch_name, "HEAD"])
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
            branch_name,
        ])
        .output()?;

    if !output.status.success() {
        return Err(WorktreeError::GitError(format!(
            "Failed to create worktree: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    Ok((worktree_path, branch_name.to_string()))
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
/// Compares current HEAD against the merge-base with main/master branch
/// to show all changes since the branch was created.
///
/// # Returns
/// * (lines_added, lines_deleted, files_changed)
pub fn get_diff_stats(worktree_path: &Path) -> (usize, usize, usize) {
    // Find the merge-base with main or master branch
    let base_commit = find_merge_base(worktree_path);

    if let Some(base) = base_commit {
        // Compare HEAD against the merge-base (includes both committed and uncommitted changes)
        let output = Command::new("git")
            .current_dir(worktree_path)
            .args(["diff", "--numstat", &base])
            .output()
            .ok();

        if let Some(output) = output
            && output.status.success()
        {
            return parse_numstat(&String::from_utf8_lossy(&output.stdout));
        }
    }

    // Fallback: try diff of uncommitted changes only
    let output = Command::new("git")
        .current_dir(worktree_path)
        .args(["diff", "--numstat", "HEAD"])
        .output()
        .ok();

    if let Some(output) = output
        && output.status.success()
    {
        return parse_numstat(&String::from_utf8_lossy(&output.stdout));
    }

    (0, 0, 0)
}

/// Find the merge-base commit with main or master branch.
fn find_merge_base(worktree_path: &Path) -> Option<String> {
    // Try main first, then master
    for branch in ["main", "master"] {
        let output = Command::new("git")
            .current_dir(worktree_path)
            .args(["merge-base", "HEAD", branch])
            .output()
            .ok();

        if let Some(output) = output
            && output.status.success()
        {
            let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !commit.is_empty() {
                return Some(commit);
            }
        }
    }
    None
}

/// Parse git diff --numstat output into (lines_added, lines_deleted, files_changed).
fn parse_numstat(stdout: &str) -> (usize, usize, usize) {
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

    (lines_added, lines_deleted, files_changed)
}

/// Information about a worktree on disk
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    /// Full path to the worktree
    pub path: PathBuf,
    /// Worktree directory name (e.g., "project-123")
    pub name: String,
    /// Project name extracted from the worktree name
    pub project: String,
    /// Issue number extracted from the worktree name
    pub issue_number: Option<u64>,
    /// Whether this worktree has an active session
    pub has_session: bool,
    /// Whether there's a running tmux session for this worktree
    pub has_tmux: bool,
}

/// List all worktrees in the cache directory with their status.
pub fn list_worktrees() -> Vec<WorktreeInfo> {
    let worktrees_path = worktrees_dir();
    if !worktrees_path.exists() {
        return Vec::new();
    }

    let mut worktrees = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&worktrees_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                // Parse project and issue number from name (format: "project-123")
                let (project, issue_number) = parse_worktree_name(&name);

                worktrees.push(WorktreeInfo {
                    path,
                    name,
                    project,
                    issue_number,
                    has_session: false, // Will be filled in by caller
                    has_tmux: false,    // Will be filled in by caller
                });
            }
        }
    }

    worktrees.sort_by(|a, b| a.name.cmp(&b.name));
    worktrees
}

/// Parse worktree name into project and issue number.
/// Format: "project-name-123" -> ("project-name", Some(123))
fn parse_worktree_name(name: &str) -> (String, Option<u64>) {
    // Find the last dash followed by digits
    if let Some(pos) = name.rfind('-') {
        let (project, num_part) = name.split_at(pos);
        if let Ok(num) = num_part[1..].parse::<u64>() {
            return (project.to_string(), Some(num));
        }
    }
    (name.to_string(), None)
}

/// List orphaned worktrees (those without active sessions).
pub fn list_orphaned_worktrees(session_worktrees: &[PathBuf]) -> Vec<WorktreeInfo> {
    list_worktrees()
        .into_iter()
        .filter(|w| !session_worktrees.contains(&w.path))
        .collect()
}

/// Open a worktree in the configured IDE.
pub fn open_in_ide(worktree_path: &Path, ide_command: Option<&str>) -> Result<(), WorktreeError> {
    let cmd = ide_command.unwrap_or_else(|| detect_ide());

    let output = Command::new(cmd)
        .arg(worktree_path)
        .spawn();

    match output {
        Ok(_) => Ok(()),
        Err(e) => Err(WorktreeError::GitError(format!(
            "Failed to open IDE '{}': {}",
            cmd, e
        ))),
    }
}

/// Auto-detect available IDE command.
fn detect_ide() -> &'static str {
    // Check for cursor first, then VS Code
    for cmd in ["cursor", "code"] {
        if Command::new("which")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return match cmd {
                "cursor" => "cursor",
                _ => "code",
            };
        }
    }
    "code" // Default fallback
}

/// Remove multiple worktrees and their branches.
pub fn prune_worktrees(worktrees: &[WorktreeInfo]) -> Vec<(String, Result<(), WorktreeError>)> {
    let mut results = Vec::new();

    for worktree in worktrees {
        // We need a parent repo to remove worktrees properly
        // Try to find the original repo by looking at git config
        let result = if let Some(local_path) = find_parent_repo(&worktree.path) {
            remove_worktree(&local_path, &worktree.path, true)
        } else {
            // Fallback: just remove the directory
            std::fs::remove_dir_all(&worktree.path)
                .map_err(WorktreeError::from)
        };

        results.push((worktree.name.clone(), result));
    }

    results
}

/// Try to find the parent git repository for a worktree.
fn find_parent_repo(worktree_path: &Path) -> Option<PathBuf> {
    // Read .git file which points to the parent repo
    let git_file = worktree_path.join(".git");
    if git_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&git_file) {
            // Format: "gitdir: /path/to/repo/.git/worktrees/name"
            if let Some(gitdir) = content.strip_prefix("gitdir: ") {
                let gitdir = PathBuf::from(gitdir.trim());
                // Go up from .git/worktrees/name to the repo root
                if let Some(worktrees_dir) = gitdir.parent() {
                    if let Some(git_dir) = worktrees_dir.parent() {
                        if let Some(repo_root) = git_dir.parent() {
                            return Some(repo_root.to_path_buf());
                        }
                    }
                }
            }
        }
    }
    None
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

    #[test]
    fn parse_worktree_name_simple() {
        let (project, num) = parse_worktree_name("myproject-42");
        assert_eq!(project, "myproject");
        assert_eq!(num, Some(42));
    }

    #[test]
    fn parse_worktree_name_with_dashes() {
        let (project, num) = parse_worktree_name("my-cool-project-123");
        assert_eq!(project, "my-cool-project");
        assert_eq!(num, Some(123));
    }

    #[test]
    fn parse_worktree_name_no_number() {
        let (project, num) = parse_worktree_name("myproject");
        assert_eq!(project, "myproject");
        assert_eq!(num, None);
    }

    // Integration tests would require actual git repos
    // and are skipped in unit tests
}
