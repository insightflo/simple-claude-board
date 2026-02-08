//! App state management and event loop

use std::path::PathBuf;
use std::time::Instant;

use crate::data::state::DashboardState;
use crate::data::tasks_parser::TaskStatus;
use crate::data::tasks_writer;
use crate::data::watcher::FileChange;
use crate::ui::gantt::GanttState;
use crate::ui::layout::FocusedPane;

/// Information about a retry target task
#[derive(Debug, Clone)]
pub struct RetryTarget {
    pub task_id: String,
    pub task_name: String,
    pub retryable: bool,
}

/// Main application state
pub struct App {
    pub running: bool,
    pub dashboard: DashboardState,
    pub gantt_state: GanttState,
    pub focused: FocusedPane,
    pub show_help: bool,
    pub show_retry_modal: bool,
    pub retry_target: Option<RetryTarget>,
    pub tasks_path: Option<PathBuf>,
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
            show_retry_modal: false,
            retry_target: None,
            tasks_path: None,
            start_time: Instant::now(),
        }
    }

    pub fn with_dashboard(mut self, dashboard: DashboardState) -> Self {
        self.dashboard = dashboard;
        self
    }

    pub fn with_tasks_path(mut self, path: PathBuf) -> Self {
        self.tasks_path = Some(path);
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

    /// Toggle collapse on the currently selected phase header
    pub fn toggle_collapse(&mut self) {
        if let Some(pi) = self.gantt_state.selected_phase_index(&self.dashboard) {
            self.gantt_state.toggle_collapse(pi);
        }
    }

    /// Toggle between Tree and HorizontalBar view modes
    pub fn toggle_view(&mut self) {
        self.gantt_state.toggle_view();
    }

    /// Open the retry modal for the currently selected task
    pub fn open_retry_modal(&mut self) {
        if let Some((pi, ti)) = self.selected_task() {
            let task = &self.dashboard.phases[pi].tasks[ti];
            // Only allow retry for Failed or Blocked tasks
            if task.status != TaskStatus::Failed && task.status != TaskStatus::Blocked {
                return;
            }
            // Check if there's a matching error with retryable info
            let retryable = self
                .dashboard
                .recent_errors
                .iter()
                .rfind(|e| e.task_id == task.id)
                .map_or(true, |e| e.retryable); // default to retryable if no error record

            self.retry_target = Some(RetryTarget {
                task_id: task.id.clone(),
                task_name: task.name.clone(),
                retryable,
            });
            self.show_retry_modal = true;
        }
    }

    /// Confirm retry: update TASKS.md status to InProgress
    pub fn confirm_retry(&mut self) {
        if let Some(ref target) = self.retry_target.clone() {
            if target.retryable {
                if let Some(ref path) = self.tasks_path {
                    if let Ok(true) =
                        tasks_writer::update_task_status(path, &target.task_id, "InProgress")
                    {
                        // Reload the tasks to reflect the change
                        if let Ok(content) = std::fs::read_to_string(path) {
                            let _ = self.dashboard.reload_tasks(&content);
                        }
                    }
                }
            }
        }
        self.show_retry_modal = false;
        self.retry_target = None;
    }

    /// Cancel the retry modal
    pub fn cancel_retry(&mut self) {
        self.show_retry_modal = false;
        self.retry_target = None;
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
                    self.dashboard.reload_from_events(&result.events);
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
    fn open_retry_modal_on_failed_task() {
        let input = include_str!("../tests/fixtures/sample_tasks.md");
        let dashboard = DashboardState::from_tasks_content(input).unwrap();
        let mut app = App::new().with_dashboard(dashboard);
        app.gantt_state.total_items = 11;

        // Navigate to a Failed task: P1-R3-T1 (Phase 1, task index 2)
        // Phase 0 header(0) + 2 tasks(1,2) + Phase 1 header(3) + task(4) + task(5) + task(6=Failed)
        app.gantt_state.selected = 6;
        app.open_retry_modal();
        assert!(app.show_retry_modal);
        assert!(app.retry_target.is_some());
        let target = app.retry_target.as_ref().unwrap();
        assert_eq!(target.task_id, "P1-R3-T1");
    }

    #[test]
    fn open_retry_modal_ignored_for_completed_task() {
        let input = include_str!("../tests/fixtures/sample_tasks.md");
        let dashboard = DashboardState::from_tasks_content(input).unwrap();
        let mut app = App::new().with_dashboard(dashboard);
        app.gantt_state.total_items = 11;

        // Navigate to a Completed task: P0-T0.1 (index 1)
        app.gantt_state.selected = 1;
        app.open_retry_modal();
        assert!(!app.show_retry_modal);
        assert!(app.retry_target.is_none());
    }

    #[test]
    fn cancel_retry_closes_modal() {
        let mut app = App::new();
        app.show_retry_modal = true;
        app.retry_target = Some(super::RetryTarget {
            task_id: "T1".to_string(),
            task_name: "Test".to_string(),
            retryable: true,
        });
        app.cancel_retry();
        assert!(!app.show_retry_modal);
        assert!(app.retry_target.is_none());
    }

    #[test]
    fn confirm_retry_updates_tasks_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let tasks_file = tmp.path().join("TASKS.md");
        std::fs::write(
            &tasks_file,
            "# Phase 1\n\n### [Failed] T1: Test task\n- body\n",
        )
        .unwrap();

        let content = std::fs::read_to_string(&tasks_file).unwrap();
        let dashboard = DashboardState::from_tasks_content(&content).unwrap();
        let mut app = App::new()
            .with_dashboard(dashboard)
            .with_tasks_path(tasks_file.clone());

        app.show_retry_modal = true;
        app.retry_target = Some(super::RetryTarget {
            task_id: "T1".to_string(),
            task_name: "Test task".to_string(),
            retryable: true,
        });

        app.confirm_retry();
        assert!(!app.show_retry_modal);
        assert!(app.retry_target.is_none());

        let result = std::fs::read_to_string(&tasks_file).unwrap();
        assert!(result.contains("[InProgress] T1:"));
    }

    #[test]
    fn confirm_retry_non_retryable_does_not_write() {
        let tmp = tempfile::TempDir::new().unwrap();
        let tasks_file = tmp.path().join("TASKS.md");
        std::fs::write(&tasks_file, "# Phase 1\n\n### [Failed] T1: Test task\n").unwrap();

        let content = std::fs::read_to_string(&tasks_file).unwrap();
        let dashboard = DashboardState::from_tasks_content(&content).unwrap();
        let mut app = App::new()
            .with_dashboard(dashboard)
            .with_tasks_path(tasks_file.clone());

        app.show_retry_modal = true;
        app.retry_target = Some(super::RetryTarget {
            task_id: "T1".to_string(),
            task_name: "Test task".to_string(),
            retryable: false,
        });

        app.confirm_retry();
        assert!(!app.show_retry_modal);

        let result = std::fs::read_to_string(&tasks_file).unwrap();
        assert!(result.contains("[Failed] T1:"));
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
