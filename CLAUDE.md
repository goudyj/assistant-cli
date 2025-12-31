# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust CLI tool that generates GitHub issues using an LLM (Ollama with Mistral 7B). It provides an interactive REPL interface where users can describe work in natural language (English or French), and the assistant generates well-formatted GitHub issues with proper categorization (bug/task), labels, and structure.

## Architecture

### Core Modules

- **`llm`**: LLM interaction layer that communicates with Ollama API endpoints
  - Uses `reqwest` for HTTP requests to `/api/chat` endpoint
  - Expects JSON responses conforming to `ChatChunk` structure
  - Model: `mistral:7b` with JSON output format
  - Configurable endpoint via `LLM_ENDPOINT` environment variable (default: `http://localhost:11434/api/chat`)

- **`issues`**: Issue generation logic with conversational refinement
  - Contains `PROMPT_ISSUE` constant with detailed system prompt for GitHub issue formatting
  - Supports two issue types: `bug` and `task`, each with specific markdown structure
  - Main function: `generate_issue()` creates initial issue from description
  - Maintains conversation history (`Vec<llm::Message>`) for iterative refinement
  - Returns `IssueContent` struct with `type_`, `title`, `body`, and `labels` fields

- **`github`**: GitHub API integration using `octocrab` (currently stub implementation)
  - `GitHubConfig` struct stores token, owner, and repo
  - `create_issue()` method shows pattern for creating issues via GitHub API

- **`main.rs`**: REPL interface using `reedline` with session management
  - Commands: `/issue <desc>`, `/ok` (creates issue), `/quit`, `/help`
  - Session state: tracks current `IssueContent` and conversation `messages`
  - Feedback loop: users can provide feedback to refine generated issues before creation

### Key Flows

**Issue Generation Flow:**
1. User enters `/issue <description>`
2. `issues::generate_issue()` sends description + system prompt to LLM
3. LLM returns JSON-formatted `IssueContent`
4. Issue displayed to user with color-coded output
5. User can provide feedback to refine, or `/ok` to create (not yet implemented)
6. Feedback updates sent to LLM with full conversation history

**LLM Interaction Pattern:**
- All messages stored as `Vec<llm::Message>` with `role` (system/user/assistant) and `content`
- System prompt defines issue format and rules (see `PROMPT_ISSUE` in `issues.rs`)
- LLM configured with `"format": "json"` to enforce structured output
- Response content parsed as `IssueContent` struct

## Development Commands

### Build and Run
```bash
cargo build
cargo run
```

### Testing
```bash
# Run all tests
cargo test

# Run tests with output visible
cargo test -- --nocapture

# Run single test
cargo test test_name

# Run tests for specific module
cargo test llm::tests
cargo test issues::tests
```

### Test Infrastructure
- Uses `wiremock` for HTTP mocking (see `test_helpers::MockChatServer`)
- Tests use `#[tokio::test(flavor = "current_thread")]` for async
- `MockChatServer` provides `expect_json()` and `expect_status()` helpers

## Environment Configuration

- **`LLM_ENDPOINT`**: Override default Ollama endpoint (default: `http://localhost:11434/api/chat`)
- **`GITHUB_TOKEN`**: Required for GitHub API operations (loaded via `dotenvy`)

## Important Conventions

- Issue types must be either `"bug"` or `"task"` (field name is `type_` to avoid Rust keyword)
- Bug issues require sections: **Context**, **Steps to reproduce**, optionally **Expected/Actual behavior**
- Task issues require: **Context**, **Goal**, **Acceptance criteria** (checkboxes)
- Labels follow pattern: `["bug"]` or `["chapter:back", "chapter:front", "chapter:sre"]`
- All generated issue content must be in English, regardless of input language
- Conversation history preserved across refinement iterations
