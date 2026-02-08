# oh-my-claude-board

> Claude Code orchestration TUI dashboard

Real-time visualization of Claude Code agent activity and task progress in your terminal.

```
+----------- Tasks -----------+---------- Detail ----------+
| # Phase 0: Setup            | P1-R1-T1: Parser impl     |
|   [x] P0-T0.1: Cargo setup  | Status: InProgress        |
|   [x] P0-T0.2: CI setup     | Agent: @backend-specialist|
| # Phase 1: Data Engine      | Blocked by: P0-T0.1       |
| > [/] P1-R1-T1: Parser impl +------ Agents --------------+
|   [ ] P1-R2-T1: Hook parser | >> backend-specialist [T1] |
|   [!] P1-R3-T1: Watcher     |    -> Edit                 |
| # Phase 2: TUI Core         | -- test-specialist         |
|   [B] P2-S1-T1: Gantt       +----------------------------+
| Tasks: 8 | Done: 2 | Progress: 25% ==================    |
+-----------------------------------------------------------+
```

## Features

- **Live task tracking** -- Watches `TASKS.md` and updates the Gantt chart on every save
- **Agent activity panel** -- Shows which Claude Code agents are running, their current tools, and errors
- **Hook event bridge** -- Includes `event-logger.js` hook that logs tool use to JSONL for the dashboard to consume
- **File watcher** -- Uses `notify` for filesystem events (FSEvents on macOS, inotify on Linux)
- **Vim-style navigation** -- `j`/`k` to navigate, `Tab` to switch panes, `?` for help
- **~1MB binary** -- Optimized release build with LTO and symbol stripping

## Installation

```bash
# From source
cargo install --path .

# Or build locally
cargo build --release
# Binary at target/release/oh-my-claude-board
```

**Requirements:** Rust 1.75+, Node.js (for hook script)

## CLI Reference

```
oh-my-claude-board [OPTIONS] [COMMAND]
```

| Option | Default | Description |
|---|---|---|
| `--tasks <PATH>` | `./TASKS.md` (fallback: `./docs/planning/06-tasks.md`) | Path to TASKS.md file |
| `--hooks <PATH>` | `.claude/hooks` (fallback: `~/.claude/hooks`) | Directory containing hook JSONL event files |
| `--events <PATH>` | `~/.claude/dashboard` | Directory for dashboard JSONL events (written by `event-logger.js`) |

| Command | Description |
|---|---|
| `watch` (default) | Watch files and display live TUI dashboard |
| `init` | Initialize configuration (placeholder) |

## File Paths

The dashboard reads from three locations:

```
./TASKS.md                          <-- --tasks (project task definitions)
.claude/hooks/*.jsonl               <-- --hooks (legacy hook event files)
~/.claude/dashboard/events.jsonl    <-- --events (event-logger.js output)
```

- `--tasks` points to a single file. The watcher monitors its parent directory.
- `--hooks` and `--events` are directories. All `*.jsonl` files inside are parsed at startup, and new writes are detected via `notify`.
- `--events` defaults to `$HOME/.claude/dashboard`. The directory is created automatically by `event-logger.js` on first tool use.
- Session ID is stored at `/tmp/claude-dashboard-session-id` and shared across all hook invocations within a session.

## Quick Start

### 1. Install the hook

Copy the event logger hook and register it in Claude Code settings:

```bash
# Create the events directory
mkdir -p ~/.claude/dashboard

# Copy the hook script (if not already at ~/.claude/hooks/)
cp hooks/event-logger.js ~/.claude/hooks/event-logger.js
```

Add to `~/.claude/settings.json` under both `PreToolUse` and `PostToolUse`:

```json
{
  "matcher": "Task|Edit|Write|Read|Bash|Grep|Glob",
  "hooks": [
    {
      "type": "command",
      "command": "node \"${HOME}/.claude/hooks/event-logger.js\"",
      "timeout": 3
    }
  ]
}
```

### 2. Run the dashboard

```bash
# Default: watches ./TASKS.md + ~/.claude/dashboard/events.jsonl
oh-my-claude-board

# Custom paths
oh-my-claude-board watch --tasks ./TASKS.md --hooks .claude/hooks --events ~/.claude/dashboard
```

### 3. Use Claude Code normally

Open another terminal and run Claude Code. The dashboard will show agent activity in real time.

## How It Works

```
Claude Code (tool use)
       |
       v
 event-logger.js          <-- PreToolUse / PostToolUse hook
       |
       v  (fs.appendFileSync)
 ~/.claude/dashboard/
   events.jsonl            <-- JSONL append-only log
       |
       v  (notify file watcher)
 oh-my-claude-board        <-- TUI dashboard
       |
       v
 Terminal (ratatui)
```

**Event flow:**
1. Claude Code calls a tool (Edit, Bash, Task, etc.)
2. `settings.json` triggers `event-logger.js` as a Pre/Post hook
3. The hook appends a JSONL line to `~/.claude/dashboard/events.jsonl`
4. The dashboard's file watcher detects the change
5. The hook parser reads new events and updates `DashboardState`
6. The Agents panel renders live agent status

**JSONL format:**
```json
{"event_type":"agent_start","timestamp":"2026-02-08T10:00:00Z","agent_id":"backend-specialist","task_id":"P1-R1-T1","session_id":"sess-abc123","tool_name":"backend-specialist"}
{"event_type":"tool_start","timestamp":"2026-02-08T10:00:01Z","agent_id":"main","task_id":"unknown","session_id":"sess-abc123","tool_name":"Edit"}
```

**TASKS.md format** (parsed by `nom`):

```markdown
# Phase 0: Setup

### [x] P0-T0.1: Project init
- **blocked_by**: (none)

### [InProgress] P1-R1-T1: Parser
- **blocked_by**: P0-T0.1
```

Status tags: `[x]` completed, `[ ]` pending, `[InProgress]` or `[/]` in progress, `[Failed]` or `[!]` failed, `[Blocked]` or `[B]` blocked

## Keybindings

| Key | Action |
|---|---|
| `j` / `Down` | Move down |
| `k` / `Up` | Move up |
| `Tab` | Switch focus (Task List / Detail) |
| `?` | Toggle help overlay |
| `q` / `Esc` | Quit |

## Layout

```
+------ 55% ------+------ 45% ------+
|                  |     Detail      |
|    Task List     |     (70%)       |
|                  +-----------------+
|                  |     Agents      |
|                  |     (30%)       |
+------------------+-----------------+
|            Status Bar              |
+------------------------------------+
```

## Architecture

```
src/
  main.rs              CLI entry point (clap)
  app.rs               App state + event handling
  event.rs             Keyboard/file/timer event unification
  lib.rs               Crate root
  data/
    tasks_parser.rs    TASKS.md parser (nom combinators)
    hook_parser.rs     JSONL event parser (serde_json)
    watcher.rs         File watcher (notify 6)
    state.rs           Unified DashboardState model
  ui/
    layout.rs          Screen split computation
    gantt.rs           Gantt chart / task tree widget
    detail.rs          Task detail panel
    claude_output.rs   Agent activity panel
    statusbar.rs       Bottom status bar
    help.rs            Help overlay popup
  analysis/
    rules.rs           Error pattern matching rules
    api.rs             Optional AI analysis (feature: ai-analysis)
```

## Dependencies

| Crate | Version | Role |
|---|---|---|
| `ratatui` | 0.28 | TUI rendering framework |
| `crossterm` | 0.28 | Terminal I/O backend |
| `tokio` | 1 | Async runtime (channels for file watcher) |
| `clap` | 4 | CLI argument parsing |
| `serde` + `serde_json` | 1 | JSONL deserialization |
| `nom` | 7 | TASKS.md parser combinators |
| `notify` | 6 | Cross-platform file watcher (FSEvents/inotify) |
| `chrono` | 0.4 | Timestamp parsing with serde support |
| `anyhow` + `thiserror` | 1 / 2 | Error handling |
| `tracing` | 0.1 | Structured logging |
| `reqwest` | 0.12 | HTTP client (optional, feature: `ai-analysis`) |

## Development

```bash
# Run tests (106 tests)
cargo test --lib

# Run with ignored tests (macOS watcher flaky tests)
cargo test --lib -- --include-ignored

# Clippy
cargo clippy -- -D warnings

# Benchmarks
cargo bench
```

## License

MIT
