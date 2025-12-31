# Assistant

CLI to generate GitHub issues using an LLM (Ollama/Mistral).

## Installation

```bash
cargo build --release
```

## Configuration

### 1. Create a GitHub OAuth App

1. Go to https://github.com/settings/developers
2. Click "New OAuth App"
3. Fill in:
   - **Application name**: `assistant-cli` (or any name)
   - **Homepage URL**: `https://github.com` (or your repo URL)
   - **Authorization callback URL**: `https://github.com` (not used for device flow)
   - **☑️ Enable Device Flow**: **Check this box!**
4. Click "Register application"
5. Copy the **Client ID** (format: `Ov23li...`)

### 2. Configuration file

Create `~/.config/assistant.json`:

```json
{
  "github_client_id": "Ov23liXXXXXXXXXXXXXX",
  "projects": {
    "my-project": {
      "owner": "username",
      "repo": "my-repo",
      "labels": ["bug", "feature", "backend", "frontend"]
    },
    "other-project": {
      "owner": "org",
      "repo": "other-repo",
      "labels": ["bug", "enhancement", "priority:high"]
    }
  }
}
```

| Field | Description |
|-------|-------------|
| `github_client_id` | Client ID from your GitHub OAuth App |
| `projects.*.owner` | Repository owner (user or organization) |
| `projects.*.repo` | Repository name |
| `projects.*.labels` | Available labels for this project |

### 3. Ollama

Install [Ollama](https://ollama.ai) and pull the model:

```bash
ollama pull mistral:7b
```

## Usage

```bash
cargo run
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

The token is securely stored in the system keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service).

### Commands

| Command | Description |
|---------|-------------|
| `/login` | Authenticate with GitHub via browser |
| `/logout` | Remove GitHub authentication |
| `/repository <name>` | Select a project from config |
| `/repo <name>` | Alias for `/repository` |
| `/issue <description>` | Generate an issue from a description |
| `/ok` | Create the issue on GitHub |
| `/help` | Show help |
| `/quit` | Exit |

### Example session

```
$ cargo run

Config loaded. Projects: my-project, other-project
GitHub: not logged in. Use /login to authenticate.
Commands: /login, /repository <name>, /issue <desc>, /ok, /quit

〉/login
Starting GitHub authentication...
[Browser opens automatically]
Successfully logged in to GitHub!

〉/repository my-project
Selected project: username/my-repo
Labels: bug, feature, backend, frontend

〉/issue fix the OAuth connection bug

--- Generated Issue ---
Type: bug
Labels: bug, backend
Title: Fix OAuth connection bug

**Context**
- The OAuth authentication flow is failing.

**Steps to reproduce**
1. Attempt to log in via OAuth
2. Observe the error
-----------------------

Give feedback to adapt the issue or type /ok to create it.

〉/ok
Issue created: https://github.com/username/my-repo/issues/42
```

## Environment variables (optional)

```bash
# LLM endpoint (default: http://localhost:11434/api/chat)
LLM_ENDPOINT=http://localhost:11434/api/chat
```

## Project structure

```
src/
├── auth.rs      # OAuth Device Flow + keyring
├── config.rs    # JSON configuration
├── github.rs    # GitHub API (octocrab)
├── issues.rs    # Issue generation via LLM
├── llm.rs       # Ollama communication
├── lib.rs       # Module exports
└── main.rs      # CLI REPL
```
