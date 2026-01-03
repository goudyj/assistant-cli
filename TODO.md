# TODO: TUI-First Refactoring

## Status: COMPLETED

All implementation steps have been completed. The application now launches directly into TUI mode.

## Changes Made

### Phase 1: Login Screen ✅
- Added `run_login_screen()` in `src/tui.rs`
- Shows "No GitHub connection detected" message
- Initiates device flow on Enter, waits for authorization
- Made `device_code` and `client_id` fields public in `DeviceFlowAuth`

### Phase 2: Modified main.rs Entry Point ✅
- Removed REPL loop, replaced with direct TUI launch
- Added `--logout` CLI flag support
- Added `--project <name>` CLI flag support
- Auto-uses `last_project` from config if available
- Shows project selection TUI if no project is set
- Legacy REPL code preserved as `run_legacy_repl()` (marked `#[allow(dead_code)]`)

### Phase 3: Command Palette ✅
- Added `TuiView::Command` variant with input, suggestions, and selection
- Added `CommandSuggestion` struct for palette items
- Press `:` in List view to open command palette
- Supports `/logout`, `/project`, and custom list commands from config
- Autocomplete filtering as you type

### Phase 4: Issue Creation Flow ✅
- Added `TuiView::CreateIssue` and `TuiView::PreviewIssue` variants
- Added `CreateStage` enum (Description, Generating)
- Press `C` in List view to create new issue
- Type description, press Enter to generate via LLM
- Preview shows formatted issue, allows feedback refinement
- Press Enter with empty feedback to create on GitHub
- Added `Clone` derive to `IssueContent`

### Phase 5: Project Selection in TUI ✅
- Added `TuiView::ProjectSelect` variant
- Added `run_project_select()` standalone function
- Up/down navigation, Enter to select, Esc to cancel

### Phase 6: Updated Keybindings Help ✅
- Title bar now shows: `C create │ d dispatch │ t tmux │ : cmd │ / search │ q quit`
- Changed worktree cleanup from `C` to `W` to avoid conflict

## New Fields in IssueBrowser
- `project_labels: Vec<String>` - for issue creation
- `available_commands: Vec<CommandSuggestion>` - for command palette

## Testing
- All 75 tests pass
- Build succeeds with no warnings

## Next Steps (Optional)
- Remove legacy REPL code after validation in production
- Add more built-in commands to the palette (refresh, state filter, etc.)
- Improve project selection with local_path display
