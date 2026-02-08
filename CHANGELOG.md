# Changelog

All notable changes to this project will be documented in this file.

## [0.1.0] - 2026-02-08

### Added
- TASKS.md parser using `nom` combinators (Phase/Task extraction with status, agent, blocked_by)
- Hook events JSONL parser for Claude Code tool use tracking
- File watcher with `notify` 6.x (FSEvents on macOS, inotify on Linux)
- Unified `DashboardState` merging tasks and hook event data
- Gantt chart panel with tree view (collapsible phases) and horizontal bar view
- Task detail panel showing status, agent, dependencies, body, and errors
- Agent activity panel with live Running/Idle status and current tool display
- Status bar with progress percentage, task counters, and uptime
- Help overlay with keybinding reference
- Rule-based error analysis engine (12 patterns: Permission, Network, Type, Runtime)
- Retry confirmation modal for Failed/Blocked tasks (`r` key)
- TASKS.md write-back to update task status on retry
- Hook event logger (`event-logger.js`) bridging Claude Code hooks to JSONL
- Vim-style navigation with Korean IME support
- Dual Gantt view toggle (`v` key)
- GitHub Actions CI (macOS + Linux) and release workflow (cross-compile)
- Criterion benchmarks for parser and render performance
