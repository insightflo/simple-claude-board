//! Unified State Model
//!
//! Combines parsed TASKS.md data, hook events, and file watcher
//! into a single dashboard state for the TUI to consume.

use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Utc};

use crate::data::hook_parser::{self, EventType, HookEvent};
use crate::data::tasks_parser::{self, ParsedPhase, TaskStatus};

/// Agent activity status derived from hook events
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentStatus {
    Idle,
    Running,
    Error,
}

/// A snapshot of one agent's current state
#[derive(Debug, Clone)]
pub struct AgentState {
    pub agent_id: String,
    pub status: AgentStatus,
    pub current_task: Option<String>,
    pub current_tool: Option<String>,
    pub event_count: usize,
    pub error_count: usize,
}

/// Timing info for a task derived from hook events
#[derive(Debug, Clone, Default)]
pub struct TaskTiming {
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// The complete dashboard state
#[derive(Debug, Clone)]
pub struct DashboardState {
    pub phases: Vec<ParsedPhase>,
    pub agents: HashMap<String, AgentState>,
    pub task_times: HashMap<String, TaskTiming>,
    pub total_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub overall_progress: f32,
}

impl Default for DashboardState {
    fn default() -> Self {
        Self {
            phases: Vec::new(),
            agents: HashMap::new(),
            task_times: HashMap::new(),
            total_tasks: 0,
            completed_tasks: 0,
            failed_tasks: 0,
            overall_progress: 0.0,
        }
    }
}

impl DashboardState {
    /// Build state from a TASKS.md file path
    pub fn from_tasks_file(path: &Path) -> Result<Self, String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("failed to read tasks: {e}"))?;
        Self::from_tasks_content(&content)
    }

    /// Build state from TASKS.md content string
    pub fn from_tasks_content(content: &str) -> Result<Self, String> {
        let phases = tasks_parser::parse_tasks_md(content)?;
        let mut state = Self::default();
        state.update_from_phases(phases);
        Ok(state)
    }

    /// Update task-related fields from parsed phases
    fn update_from_phases(&mut self, phases: Vec<ParsedPhase>) {
        let mut total = 0;
        let mut completed = 0;
        let mut failed = 0;

        for phase in &phases {
            for task in &phase.tasks {
                total += 1;
                match task.status {
                    TaskStatus::Completed => completed += 1,
                    TaskStatus::Failed => failed += 1,
                    _ => {}
                }
            }
        }

        self.phases = phases;
        self.total_tasks = total;
        self.completed_tasks = completed;
        self.failed_tasks = failed;
        self.overall_progress = if total > 0 {
            completed as f32 / total as f32
        } else {
            0.0
        };
    }

    /// Update agent states from hook events
    pub fn update_from_events(&mut self, events: &[HookEvent]) {
        for event in events {
            let agent = self
                .agents
                .entry(event.agent_id.clone())
                .or_insert_with(|| AgentState {
                    agent_id: event.agent_id.clone(),
                    status: AgentStatus::Idle,
                    current_task: None,
                    current_tool: None,
                    event_count: 0,
                    error_count: 0,
                });

            agent.event_count += 1;

            match event.event_type {
                EventType::AgentStart => {
                    agent.status = AgentStatus::Running;
                    agent.current_task = Some(event.task_id.clone());
                    let timing = self
                        .task_times
                        .entry(event.task_id.clone())
                        .or_default();
                    if timing.started_at.is_none() {
                        timing.started_at = Some(event.timestamp);
                    }
                }
                EventType::AgentEnd => {
                    agent.status = AgentStatus::Idle;
                    if let Some(ref task_id) = agent.current_task {
                        let timing = self.task_times.entry(task_id.clone()).or_default();
                        timing.completed_at = Some(event.timestamp);
                    }
                    agent.current_task = None;
                    agent.current_tool = None;
                }
                EventType::ToolStart => {
                    agent.current_tool = event.tool_name.clone();
                }
                EventType::ToolEnd => {
                    agent.current_tool = None;
                }
                EventType::Error => {
                    agent.status = AgentStatus::Error;
                    agent.error_count += 1;
                }
            }
        }
    }

    /// Load hook events from a directory and update agent states
    pub fn load_hook_events(&mut self, hooks_dir: &Path) -> Result<(), String> {
        let entries =
            std::fs::read_dir(hooks_dir).map_err(|e| format!("failed to read hooks dir: {e}"))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("failed to read entry: {e}"))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                let result = hook_parser::parse_hook_file(&path)
                    .map_err(|e| format!("failed to parse {}: {e}", path.display()))?;
                self.update_from_events(&result.events);
            }
        }
        Ok(())
    }

    /// Reload tasks from content (used when file watcher detects changes)
    pub fn reload_tasks(&mut self, content: &str) -> Result<(), String> {
        let phases = tasks_parser::parse_tasks_md(content)?;
        self.update_from_phases(phases);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state() {
        let state = DashboardState::default();
        assert!(state.phases.is_empty());
        assert!(state.agents.is_empty());
        assert_eq!(state.total_tasks, 0);
        assert_eq!(state.completed_tasks, 0);
        assert_eq!(state.overall_progress, 0.0);
    }

    #[test]
    fn from_tasks_content() {
        let input = include_str!("../../tests/fixtures/sample_tasks.md");
        let state = DashboardState::from_tasks_content(input).unwrap();
        assert_eq!(state.phases.len(), 3);
        assert_eq!(state.total_tasks, 8);
        assert_eq!(state.completed_tasks, 2);
        assert_eq!(state.failed_tasks, 1);
        assert!((state.overall_progress - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn from_tasks_file() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample_tasks.md");
        let state = DashboardState::from_tasks_file(&path).unwrap();
        assert_eq!(state.total_tasks, 8);
    }

    #[test]
    fn from_tasks_file_missing() {
        let result = DashboardState::from_tasks_file(Path::new("/nonexistent.md"));
        assert!(result.is_err());
    }

    #[test]
    fn update_from_agent_events() {
        let input = include_str!("../../tests/fixtures/sample_hooks/agent_events.jsonl");
        let result = hook_parser::parse_hook_events(input);

        let mut state = DashboardState::default();
        state.update_from_events(&result.events);

        assert_eq!(state.agents.len(), 1);
        let agent = state.agents.get("backend-specialist-1").unwrap();
        assert_eq!(agent.status, AgentStatus::Idle); // ended
        assert_eq!(agent.event_count, 6);
        assert_eq!(agent.error_count, 0);
        assert!(agent.current_task.is_none());
        assert!(agent.current_tool.is_none());
    }

    #[test]
    fn update_from_error_events() {
        let input = include_str!("../../tests/fixtures/sample_hooks/error_events.jsonl");
        let result = hook_parser::parse_hook_events(input);

        let mut state = DashboardState::default();
        state.update_from_events(&result.events);

        let agent = state.agents.get("backend-specialist-2").unwrap();
        assert_eq!(agent.error_count, 2);
        // Last event is agent_end, so status is Idle
        assert_eq!(agent.status, AgentStatus::Idle);
    }

    #[test]
    fn agent_running_state() {
        let input = include_str!("../../tests/fixtures/sample_hooks/agent_events.jsonl");
        let result = hook_parser::parse_hook_events(input);

        let mut state = DashboardState::default();
        // Feed only agent_start
        state.update_from_events(&result.events[..1]);

        let agent = state.agents.get("backend-specialist-1").unwrap();
        assert_eq!(agent.status, AgentStatus::Running);
        assert_eq!(agent.current_task.as_deref(), Some("P1-R1-T1"));
    }

    #[test]
    fn agent_tool_tracking() {
        let input = include_str!("../../tests/fixtures/sample_hooks/agent_events.jsonl");
        let result = hook_parser::parse_hook_events(input);

        let mut state = DashboardState::default();
        // Feed agent_start + tool_start
        state.update_from_events(&result.events[..2]);

        let agent = state.agents.get("backend-specialist-1").unwrap();
        assert_eq!(agent.current_tool.as_deref(), Some("Read"));
    }

    #[test]
    fn load_hook_events_from_dir() {
        let hooks_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample_hooks");
        let mut state = DashboardState::default();
        state.load_hook_events(&hooks_dir).unwrap();

        // Should have agents from both agent_events.jsonl and error_events.jsonl
        assert!(state.agents.len() >= 2);
    }

    #[test]
    fn reload_tasks() {
        let mut state = DashboardState::default();
        let content = "# Phase 0: Setup\n### [x] T1: Done\n### [ ] T2: Pending\n";
        state.reload_tasks(content).unwrap();
        assert_eq!(state.total_tasks, 2);
        assert_eq!(state.completed_tasks, 1);
        assert!((state.overall_progress - 0.5).abs() < f32::EPSILON);

        // Reload with different content
        let content2 = "# Phase 0: Setup\n### [x] T1: Done\n### [x] T2: Done\n";
        state.reload_tasks(content2).unwrap();
        assert_eq!(state.completed_tasks, 2);
        assert!((state.overall_progress - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn full_pipeline() {
        let tasks_input = include_str!("../../tests/fixtures/sample_tasks.md");
        let mut state = DashboardState::from_tasks_content(tasks_input).unwrap();

        let hooks_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample_hooks");
        state.load_hook_events(&hooks_dir).unwrap();

        // Verify tasks loaded
        assert_eq!(state.phases.len(), 3);
        assert_eq!(state.total_tasks, 8);

        // Verify agents loaded
        assert!(state.agents.len() >= 2);
        assert!(state.agents.contains_key("backend-specialist-1"));
        assert!(state.agents.contains_key("backend-specialist-2"));
    }
}
