# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust CLI tool that generates GitHub issues using an LLM (Claude Code or Opencode CLI). It provides a TUI interface where users can describe work in natural language (English or French), and the assistant generates well-formatted GitHub issues with proper categorization (bug/task), labels, and structure.

## Architecture

### Core Modules

- **`auth`**: GitHub OAuth Device Flow authentication
  - `DeviceFlowAuth::start(client_id, client_secret)` initiates device flow
  - `poll_for_token()` waits for user authorization
  - Token stored in config file
  - `get_stored_token()`, `store_token()`, `delete_token()` for token operations

- **`config`**: Configuration management from `~/.config/assistant.json`
  - `Config` struct with `github_client_id`, `projects: HashMap<String, ProjectConfig>`, and `last_project`
  - `ProjectConfig` stores `owner`, `repo`, and `labels` per project
  - `load_config()` reads from cross-platform config directory
  - `Config::save()` persists config changes (e.g., last selected project)
  - `last_project` is auto-saved when selecting a project and restored on startup

- **`llm`**: LLM interaction layer using Claude Code or Opencode CLI
  - Executes CLI commands (`claude -p` or `opencode run`) for issue generation
  - Agent type configured via `coding_agent` setting in config file
  - Builds prompts from message history and parses CLI output

- **`issues`**: Issue generation logic with conversational refinement
  - `build_prompt(labels)` generates system prompt with project-specific labels
  - Supports two issue types: `bug` and `task`, each with specific markdown structure
  - `generate_issue_with_labels()` creates issue with custom label set

- **`github`**: GitHub API integration using `octocrab`
  - `GitHubConfig::from_keyring(owner, repo)` loads token from system keyring
  - `create_issue(&IssueContent)` creates issue on GitHub with labels

- **`main.rs`**: Application entry point
  - Handles CLI arguments, config loading, and TUI initialization
  - Manages project selection and authentication flow

### Key Flows

**Authentication Flow (OAuth Device Flow):**
1. User runs `/login`
2. App requests device code from GitHub with `client_id`
3. Browser opens to `github.com/login/device`
4. User enters the displayed code
5. App polls GitHub until authorized
6. Token stored in system keyring

**Issue Generation Flow:**
1. User selects project with `/repository <name>`
2. User enters `/issue <description>`
3. `issues::generate_issue_with_labels()` calls the configured CLI (Claude or Opencode) with prompt
4. User can provide feedback to refine, or confirm to create on GitHub

## Configuration

**File: `~/.config/assistant.json`**
```json
{
  "coding_agent": "claude",
  "projects": {
    "my-project": {
      "owner": "username",
      "repo": "my-repo",
      "labels": ["bug", "feature", "backend"],
      "local_path": "/path/to/repo",
      "base_branch": "develop"
    }
  },
  "last_project": "my-project"
}
```

- `github_client_id`: Optional override for OAuth App Client ID (embedded by default)
- `coding_agent`: CLI to use for issue generation and dispatch (`"claude"` or `"opencode"`, default: `"claude"`)
- `last_project`: Automatically managed by the application
- `local_path`: Path to the local git repository for worktree creation
- `base_branch`: Base branch for new branches (auto-detects main/master/develop if not set)

## Development Commands

```bash
cargo build                         # Build
cargo run                           # Run
cargo test                          # Run all tests
cargo test -- --nocapture           # With output
cargo test config::tests            # Module tests
```

### Test Infrastructure
- Uses `wiremock` for HTTP mocking (GitHub API tests)
- Tests use `#[tokio::test(flavor = "current_thread")]` for async

## CLI Commands

Inside the TUI, press `/` to open the command palette:

```
/all                - Show all issues (clear filters)
/issues             - Show issues list
/prs                - Show pull requests list
/logout             - Remove GitHub authentication
/repository         - Open interactive project selector (alias: /repo)
/agent              - Select dispatch agent (Claude Code or Opencode)
/worktrees          - Manage worktrees (view, delete, open IDE)
/prune              - Clean up orphaned worktrees
/<custom>           - Custom filter commands defined in project config (e.g., /bugs)
```

Startup options:
```
--project <name>    - Start with a specific project
--logout            - Remove GitHub authentication and exit
```

## Code Style

- **All code, comments, and documentation must be in English**
- Commit messages follow Conventional Commits: `feat:`, `fix:`, `chore:`

## Important Conventions

- Issue types: `"bug"` or `"task"` (field name is `type_` to avoid Rust keyword)
- Bug issues require: **Context**, **Steps to reproduce**
- Task issues require: **Context**, **Goal**, **Acceptance criteria**
- Labels are project-specific, defined in config file
- Token stored in config file (`~/.config/assistant.json`)
