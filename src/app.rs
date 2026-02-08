//! App state management and event loop

use std::time::Instant;

use crate::data::state::DashboardState;
use crate::data::watcher::FileChange;
use crate::ui::gantt::GanttState;
use crate::ui::layout::FocusedPane;

/// Main application state
pub struct App {
    pub running: bool,
    pub dashboard: DashboardState,
    pub gantt_state: GanttState,
    pub focused: FocusedPane,
    pub show_help: bool,
    pub start_time: Instant,
}

impl App {
    pub fn new() -> Self {
        Self {
            running: true,
            dashboard: DashboardState::default(),
            gantt_state: GanttState::default(),
            focused: FocusedPane::TaskList,
            show_help: false,
            start_time: Instant::now(),
        }
    }

    pub fn with_dashboard(mut self, dashboard: DashboardState) -> Self {
        self.dashboard = dashboard;
        self
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    pub fn toggle_focus(&mut self) {
        self.focused = self.focused.toggle();
    }

    pub fn move_down(&mut self) {
        self.gantt_state.select_next();
    }

    pub fn move_up(&mut self) {
        self.gantt_state.select_prev();
    }

    /// Get the currently selected task as (phase_idx, task_idx)
    pub fn selected_task(&self) -> Option<(usize, usize)> {
        self.gantt_state.selected_task(&self.dashboard)
    }

    /// Handle a file change event from the watcher
    pub fn handle_file_change(&mut self, change: &FileChange) {
        match change {
            FileChange::TasksModified(path) => {
                if let Ok(content) = std::fs::read_to_string(path) {
                    let _ = self.dashboard.reload_tasks(&content);
                }
            }
            FileChange::HookEventCreated(path) | FileChange::HookEventModified(path) => {
                if let Ok(content) = std::fs::read_to_string(path) {
                    let result = crate::data::hook_parser::parse_hook_events(&content);
                    self.dashboard.update_from_events(&result.events);
                }
            }
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_default() {
        let app = App::new();
        assert!(app.running);
        assert!(!app.show_help);
        assert_eq!(app.focused, FocusedPane::TaskList);
    }

    #[test]
    fn app_quit() {
        let mut app = App::new();
        app.quit();
        assert!(!app.running);
    }

    #[test]
    fn app_toggle_help() {
        let mut app = App::new();
        assert!(!app.show_help);
        app.toggle_help();
        assert!(app.show_help);
        app.toggle_help();
        assert!(!app.show_help);
    }

    #[test]
    fn app_toggle_focus() {
        let mut app = App::new();
        assert_eq!(app.focused, FocusedPane::TaskList);
        app.toggle_focus();
        assert_eq!(app.focused, FocusedPane::Detail);
        app.toggle_focus();
        assert_eq!(app.focused, FocusedPane::TaskList);
    }

    #[test]
    fn app_navigation() {
        let input = include_str!("../tests/fixtures/sample_tasks.md");
        let dashboard = DashboardState::from_tasks_content(input).unwrap();
        let mut app = App::new().with_dashboard(dashboard);
        app.gantt_state.total_items = 11;

        app.move_down();
        assert_eq!(app.gantt_state.selected, 1);
        assert_eq!(app.selected_task(), Some((0, 0)));

        app.move_up();
        assert_eq!(app.gantt_state.selected, 0);
        assert!(app.selected_task().is_none()); // phase header
    }

    #[test]
    fn app_with_dashboard() {
        let input = include_str!("../tests/fixtures/sample_tasks.md");
        let dashboard = DashboardState::from_tasks_content(input).unwrap();
        let app = App::new().with_dashboard(dashboard);
        assert_eq!(app.dashboard.total_tasks, 8);
    }

    #[test]
    fn handle_file_change_tasks() {
        let tmp = tempfile::TempDir::new().unwrap();
        let tasks_file = tmp.path().join("TASKS.md");
        std::fs::write(
            &tasks_file,
            "# Phase 0: Setup\n\n### [x] P0-T0.1: Init project\n",
        )
        .unwrap();

        let mut app = App::new();
        assert_eq!(app.dashboard.total_tasks, 0);

        let change = FileChange::TasksModified(tasks_file);
        app.handle_file_change(&change);
        assert_eq!(app.dashboard.total_tasks, 1);
    }

    #[test]
    fn handle_file_change_hook() {
        let tmp = tempfile::TempDir::new().unwrap();
        let hook_file = tmp.path().join("session.jsonl");
        std::fs::write(
            &hook_file,
            r#"{"event_type":"agent_start","agent_id":"main","task_id":"T1","session_id":"s1","timestamp":"2026-02-08T00:00:00Z"}"#,
        )
        .unwrap();

        let mut app = App::new();
        assert!(app.dashboard.agents.is_empty());

        let change = FileChange::HookEventCreated(hook_file);
        app.handle_file_change(&change);
        assert!(!app.dashboard.agents.is_empty());
    }
}
