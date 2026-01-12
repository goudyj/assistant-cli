# Assistant

CLI to generate GitHub issues using an LLM (Ollama/Mistral).

## Installation

### Quick Install (Recommended)

**macOS / Linux:**
```bash
curl -fsSL https://raw.githubusercontent.com/goudyj/assistant-cli/master/scripts/install.sh | bash
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/goudyj/assistant-cli/master/scripts/install.ps1 | iex
```

### Install Specific Version

```bash
curl -fsSL https://raw.githubusercontent.com/goudyj/assistant-cli/master/scripts/install.sh | bash -s v0.1.0
```

### Build from Source

```bash
cargo build --release
```

## Configuration

Configure your settings in `~/.config/assistant.json`:

```json
{
  "coding_agent": "claude",
  "ide_command": "cursor",
  "projects": {
    "my-project": {
      "owner": "username",
      "repo": "my-repo",
      "labels": ["bug", "feature", "backend", "frontend"],
      "local_path": "/path/to/my-repo",
      "base_branch": "develop",
      "list_commands": {
        "bugs": ["bug"],
        "frontend": ["frontend", "bug"]
      }
    }
  }
}
```

#### Global settings

| Field | Description |
|-------|-------------|
| `coding_agent` | Agent for dispatch: `"claude"` or `"opencode"` (default: `"claude"`) |
| `ide_command` | IDE to open worktrees: `"code"`, `"cursor"`, etc. (auto-detected if not set) |
| `auto_format_comments` | Auto-format issue comments (default: `false`) |
| `last_project` | Auto-managed: remembers last selected project |

#### Project settings

| Field | Description |
|-------|-------------|
| `owner` | Repository owner (user or organization) |
| `repo` | Repository name |
| `labels` | Available labels for issue generation |
| `local_path` | Local repository path (required for worktree/dispatch features) |
| `base_branch` | Base branch for new branches (auto-detects main/master/develop if not set) |
| `list_commands` | Custom filter commands (see below) |

#### Custom filter commands

Define shortcuts to filter issues by labels:

```json
"list_commands": {
  "bugs": ["bug"],
  "urgent": ["bug", "priority:high"]
}
```

These become available as `/bugs`, `/urgent` commands in the TUI.

### 3. Ollama

Install [Ollama](https://ollama.ai) and pull the model:

```bash
ollama pull mistral:7b
```

## Usage

```bash
assistant              # Start the TUI
assistant --project my-project   # Start with a specific project
assistant --logout     # Remove GitHub authentication and exit
```

### GitHub Authentication

On first use, authenticate with GitHub:

```
〉/login
Starting GitHub authentication...

Open this URL in your browser:
  https://github.com/login/device

And enter the code: ABCD-1234

Waiting for authorization...
Successfully logged in to GitHub!
```

The token is stored in `~/.config/assistant.json`.

### Commands

Press `/` in the TUI to open the command palette.

| Command | Description |
|---------|-------------|
| `/login` | Authenticate with GitHub via browser |
| `/logout` | Remove GitHub authentication |
| `/repository` | Open interactive project selector (alias: `/repo`) |
| `/agent` | Select coding agent (Claude Code or Opencode) |
| `/worktrees` | Manage worktrees (view, delete, open in IDE) |
| `/prune` | Clean up orphaned worktrees |
| `/help` | Show help |
| `/quit` | Exit |
| `/<custom>` | Custom filter commands defined in config (e.g., `/bugs`) |

### Example session

```
$ assistant

# Select a project with /repo, then browse issues
# Generate new issues by typing a description
# Dispatch issues to a coding agent with Enter
# Create PRs from completed work
```

## Environment variables (optional)

```bash
# LLM endpoint (default: http://localhost:11434/api/chat)
LLM_ENDPOINT=http://localhost:11434/api/chat
```

## Project structure

```
src/
├── agents/           # Coding agent integrations
│   ├── claude.rs     # Claude Code dispatch
│   ├── opencode.rs   # Opencode dispatch
│   ├── worktree.rs   # Git worktree management
│   └── session.rs    # Agent session tracking
├── auth.rs           # OAuth Device Flow
├── config.rs         # JSON configuration
├── github.rs         # GitHub API (octocrab)
├── issues.rs         # Issue generation via LLM
├── llm.rs            # Ollama communication
├── tui.rs            # TUI application
├── tui_events.rs     # Event handling
├── tui_draw.rs       # UI rendering
└── main.rs           # Entry point
```
