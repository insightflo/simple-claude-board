# simple-claude-board

> Claude Code orchestration TUI dashboard

[한국어](README_ko.md)

Real-time visualization of Claude Code agent activity and task progress in your terminal.

![simple-claude-board screenshot](assets/screenshot.png)

## Features

- **Live task tracking** -- Watches `TASKS.md` and updates the Gantt chart on every save
- **Agent activity panel** -- Shows which Claude Code agents are running, their current tools, and errors
- **Rich agent detail** -- Tool usage stats, recent tool sequence (last 10), session ID, and task name cross-reference
- **Hook event bridge** -- Includes `event-logger.js` hook that logs tool use to JSONL for the dashboard to consume
- **Error analysis & retry** -- Rule-based error categorization (12 patterns) with retry modal (`r` key)
- **File watcher** -- Uses `notify` for filesystem events (FSEvents on macOS, inotify on Linux)
- **Dual Gantt view** -- Tree view with `▼`/`▶` collapse and `├─`/`└─` connectors, plus horizontal bar chart; toggle with `v`
- **Vim-style navigation** -- `j`/`k` to navigate, `Tab` to switch panes, `Space` to collapse/expand, `?` for help
- **Korean IME support** -- Korean jamo keys (`ㅓ`=j, `ㅏ`=k, `ㅂ`=q) work as vim navigation
- **~1MB binary** -- Optimized release build with LTO and symbol stripping

## Prerequisites

### Rust (1.75+)

```bash
# macOS / Linux
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# Windows — download and run the installer:
# https://rustup.rs (rustup-init.exe)

# Verify
rustc --version
```

### Node.js (18+, for hook script)

```bash
# macOS (Homebrew)
brew install node

# Linux (via nvm - recommended)
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
nvm install --lts

# Windows — download the installer:
# https://nodejs.org

# Verify
node --version
```

## Installation

```bash
# From crates.io
cargo install simple-claude-board

# Or from source
git clone https://github.com/insightflo/simple-claude-board.git
cd simple-claude-board
cargo install --path .
```

## CLI Reference

```
simple-claude-board [OPTIONS] [COMMAND]
```

| Option | Default | Description |
|---|---|---|
| `--tasks <PATH>` | `./TASKS.md` (fallback: `./docs/planning/06-tasks.md`) | Path to TASKS.md file |
| `--hooks <PATH>` | `.claude/hooks` (fallback: `~/.claude/hooks`) | Directory containing hook JSONL event files |
| `--events <PATH>` | `~/.claude/dashboard` | Directory for dashboard JSONL events (written by `event-logger.js`) |

| Command | Description |
|---|---|
| `watch` (default) | Watch files and display live TUI dashboard |
| `init` | Auto-configure hooks and settings |

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

```bash
cargo install simple-claude-board
simple-claude-board init    # auto-configure hooks & settings
simple-claude-board         # launch the dashboard
```

The `init` command automatically:
- Creates `~/.claude/dashboard/` and `~/.claude/hooks/`
- Deploys the `event-logger.js` hook script
- Patches `~/.claude/settings.json` with Pre/PostToolUse hook entries

Then open another terminal and use Claude Code normally. The dashboard shows agent activity in real time.

### Advanced usage

```bash
# Custom paths
simple-claude-board watch --tasks ./TASKS.md --hooks .claude/hooks --events ~/.claude/dashboard
```

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
 simple-claude-board        <-- TUI dashboard
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
| `j` / `Down` (`ㅓ`) | Move down |
| `k` / `Up` (`ㅏ`) | Move up |
| `Tab` | Switch focus (Task List / Detail) |
| `Space` | Collapse/expand phase |
| `v` | Switch view (Tree / Gantt bar) |
| `r` (`ㄱ`) | Retry failed task |
| `?` | Toggle help overlay |
| `q` / `Esc` (`ㅂ`) | Quit |

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
    tasks_writer.rs    TASKS.md write-back (status update)
  ui/
    layout.rs          Screen split computation
    gantt.rs           Dual Gantt view (tree + horizontal bar)
    detail.rs          Task detail panel
    claude_output.rs   Agent activity panel
    statusbar.rs       Bottom status bar
    help.rs            Help overlay popup
    retry_modal.rs     Retry confirmation modal
  analysis/
    rules.rs           Error pattern matching rules
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

## Development

```bash
# Run tests (192 lib + 62 integration tests)
cargo test

# Run with ignored tests (macOS watcher flaky tests)
cargo test --lib -- --include-ignored

# Clippy
cargo clippy -- -D warnings

# Benchmarks
cargo bench
```

### Performance

| Metric | Result | Target |
|---|---|---|
| 1000 tasks parse | ~745us | <100ms |
| Full frame render | ~55us | <16ms (60fps) |
| 1000 hook events | ~332us | <100ms |
| Release binary | ~1.1MB | <10MB |

## License

MIT
