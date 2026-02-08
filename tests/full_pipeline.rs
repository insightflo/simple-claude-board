//! P4-T1: Full pipeline integration test
//!
//! End-to-end validation of the complete data flow:
//! TASKS.md → parse → state → UI render, Hook events → parse → agent panel,
//! Error flow → analysis → retry → write-back, and keyboard interaction.

use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget, widgets::Widget};

use oh_my_claude_board::app::App;
use oh_my_claude_board::data::state::DashboardState;
use oh_my_claude_board::data::tasks_parser::TaskStatus;
use oh_my_claude_board::data::watcher::FileChange;
use oh_my_claude_board::event::{key_to_action, Action};
use oh_my_claude_board::ui::claude_output::AgentPanel;
use oh_my_claude_board::ui::detail::DetailWidget;
use oh_my_claude_board::ui::gantt::GanttWidget;
use oh_my_claude_board::ui::help::HelpOverlay;
use oh_my_claude_board::ui::layout::{DashboardLayout, FocusedPane};
use oh_my_claude_board::ui::retry_modal::RetryModal;
use oh_my_claude_board::ui::statusbar::StatusBar;

fn buffer_text(buf: &Buffer) -> String {
    let mut text = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            text.push_str(buf[(x, y)].symbol());
        }
        text.push('\n');
    }
    text
}

// ===== Pipeline 1: TASKS.md → parse → state → gantt =====

#[test]
fn tasks_file_to_gantt_render() {
    let tmp = tempfile::TempDir::new().unwrap();
    let tasks_file = tmp.path().join("TASKS.md");
    std::fs::write(
        &tasks_file,
        r#"# Phase 0: Setup

### [x] P0-T1: Init project
- **담당**: @backend-specialist

### [ ] P0-T2: CI pipeline
- **담당**: @backend-specialist

# Phase 1: Core

### [InProgress] P1-T1: Parser
- **담당**: @backend-specialist
- **blocked_by**: P0-T1
"#,
    )
    .unwrap();

    // Load via App (mimics startup)
    let content = std::fs::read_to_string(&tasks_file).unwrap();
    let dashboard = DashboardState::from_tasks_content(&content).unwrap();
    let mut app = App::new()
        .with_dashboard(dashboard)
        .with_tasks_path(tasks_file.clone());

    // Verify state
    assert_eq!(app.dashboard.phases.len(), 2);
    assert_eq!(app.dashboard.total_tasks, 3);
    assert_eq!(app.dashboard.completed_tasks, 1);

    // Render gantt
    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    let gantt = GanttWidget::new(&app.dashboard, true);
    gantt.render(area, &mut buf, &mut app.gantt_state);

    let text = buffer_text(&buf);
    assert!(text.contains("P0-T1"), "Gantt should show task P0-T1");
    assert!(text.contains("P1-T1"), "Gantt should show task P1-T1");
    assert!(text.contains("[x]"), "Gantt should show completed icon");
    assert!(text.contains("[/]"), "Gantt should show in-progress icon");
}

// ===== Pipeline 2: File change → detect → parse → state update =====

#[test]
fn file_change_updates_state() {
    let tmp = tempfile::TempDir::new().unwrap();
    let tasks_file = tmp.path().join("TASKS.md");
    std::fs::write(
        &tasks_file,
        "# Phase 0: Setup\n\n### [ ] T1: Pending task\n",
    )
    .unwrap();

    let content = std::fs::read_to_string(&tasks_file).unwrap();
    let dashboard = DashboardState::from_tasks_content(&content).unwrap();
    let mut app = App::new().with_dashboard(dashboard);

    assert_eq!(app.dashboard.completed_tasks, 0);
    assert_eq!(app.dashboard.phases[0].tasks[0].status, TaskStatus::Pending);

    // Simulate file modification (external editor changes status)
    std::fs::write(
        &tasks_file,
        "# Phase 0: Setup\n\n### [x] T1: Pending task\n",
    )
    .unwrap();

    // Handle file change event (mimics watcher)
    app.handle_file_change(&FileChange::TasksModified(tasks_file));

    assert_eq!(app.dashboard.completed_tasks, 1);
    assert_eq!(
        app.dashboard.phases[0].tasks[0].status,
        TaskStatus::Completed
    );
}

// ===== Pipeline 3: Hook events → parse → agent panel =====

#[test]
fn hook_events_to_agent_panel() {
    let tmp = tempfile::TempDir::new().unwrap();
    let events_file = tmp.path().join("events.jsonl");
    std::fs::write(
        &events_file,
        r#"{"event_type":"tool_start","timestamp":"2026-02-08T12:00:00Z","agent_id":"main","task_id":"T1","session_id":"s1","tool_name":"Edit"}
{"event_type":"tool_end","timestamp":"2026-02-08T12:00:01Z","agent_id":"main","task_id":"T1","session_id":"s1","tool_name":"Edit"}
{"event_type":"tool_start","timestamp":"2026-02-08T12:00:02Z","agent_id":"main","task_id":"T1","session_id":"s1","tool_name":"Bash"}
"#,
    )
    .unwrap();

    let mut app = App::new();
    app.handle_file_change(&FileChange::HookEventCreated(events_file));

    // Agent should be Running with Bash
    let agent = app.dashboard.agents.get("main").unwrap();
    assert_eq!(
        agent.status,
        oh_my_claude_board::data::state::AgentStatus::Running
    );
    assert_eq!(agent.current_tool.as_deref(), Some("Bash"));

    // Render agent panel
    let area = Rect::new(0, 0, 60, 10);
    let mut buf = Buffer::empty(area);
    let panel = AgentPanel::new(&app.dashboard);
    panel.render(area, &mut buf);

    let text = buffer_text(&buf);
    assert!(text.contains("main"), "Agent panel should show 'main'");
    assert!(text.contains(">>"), "Agent panel should show Running (>>)");
    assert!(
        text.contains("Bash"),
        "Agent panel should show current tool"
    );
}

// ===== Pipeline 4: Error → analysis → retry → write-back =====

#[test]
fn error_to_retry_to_writeback() {
    let tmp = tempfile::TempDir::new().unwrap();
    let tasks_file = tmp.path().join("TASKS.md");
    std::fs::write(
        &tasks_file,
        "# Phase 1: Core\n\n### [Failed] P1-T1: Parser module\n- body\n",
    )
    .unwrap();

    let events_file = tmp.path().join("events.jsonl");
    std::fs::write(
        &events_file,
        r#"{"event_type":"agent_start","timestamp":"2026-02-08T12:00:00Z","agent_id":"backend","task_id":"P1-T1","session_id":"s1"}
{"event_type":"error","timestamp":"2026-02-08T12:00:30Z","agent_id":"backend","task_id":"P1-T1","session_id":"s1","error_message":"connection refused: localhost:5432"}
{"event_type":"agent_end","timestamp":"2026-02-08T12:01:00Z","agent_id":"backend","task_id":"P1-T1","session_id":"s1"}
"#,
    )
    .unwrap();

    // Setup app with tasks and events
    let content = std::fs::read_to_string(&tasks_file).unwrap();
    let dashboard = DashboardState::from_tasks_content(&content).unwrap();
    let mut app = App::new()
        .with_dashboard(dashboard)
        .with_tasks_path(tasks_file.clone());

    app.handle_file_change(&FileChange::HookEventCreated(events_file));

    // Verify error was analyzed
    assert_eq!(app.dashboard.recent_errors.len(), 1);
    let err = &app.dashboard.recent_errors[0];
    assert_eq!(
        err.category,
        oh_my_claude_board::analysis::rules::ErrorCategory::Network
    );
    assert!(err.retryable);

    // Navigate to the Failed task and open retry modal
    app.gantt_state.total_items = 2; // 1 phase header + 1 task
    app.gantt_state.selected = 1; // select task
    app.open_retry_modal();

    assert!(app.show_retry_modal);
    let target = app.retry_target.as_ref().unwrap();
    assert_eq!(target.task_id, "P1-T1");
    assert!(target.retryable);

    // Confirm retry → writes InProgress to file
    app.confirm_retry();
    assert!(!app.show_retry_modal);

    let result = std::fs::read_to_string(&tasks_file).unwrap();
    assert!(
        result.contains("[InProgress] P1-T1:"),
        "File should be updated"
    );

    // Dashboard should reflect the change
    assert_eq!(
        app.dashboard.phases[0].tasks[0].status,
        TaskStatus::InProgress
    );
}

// ===== Pipeline 5: Full keyboard interaction scenario =====

#[test]
fn keyboard_interaction_scenario() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    let input = include_str!("fixtures/sample_tasks.md");
    let dashboard = DashboardState::from_tasks_content(input).unwrap();
    let mut app = App::new().with_dashboard(dashboard);
    app.gantt_state.total_items = 11;

    // Initial state
    assert_eq!(app.focused, FocusedPane::TaskList);
    assert!(!app.show_help);
    assert_eq!(app.gantt_state.selected, 0);

    // j → move down
    let action = key_to_action(KeyEvent {
        code: KeyCode::Char('j'),
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    });
    assert_eq!(action, Action::MoveDown);
    app.move_down();
    assert_eq!(app.gantt_state.selected, 1);
    assert_eq!(app.selected_task(), Some((0, 0)));

    // Tab → toggle focus
    app.toggle_focus();
    assert_eq!(app.focused, FocusedPane::Detail);

    // Tab → back to task list
    app.toggle_focus();
    assert_eq!(app.focused, FocusedPane::TaskList);

    // ? → help overlay
    app.toggle_help();
    assert!(app.show_help);

    // ? → close help
    app.toggle_help();
    assert!(!app.show_help);

    // Navigate to phase header, Space → collapse
    app.gantt_state.selected = 0;
    app.toggle_collapse();
    // Phase 0 should be collapsed (verify via gantt state)
    assert!(app.gantt_state.collapsed.contains(&0));

    // Space → expand
    app.toggle_collapse();
    assert!(!app.gantt_state.collapsed.contains(&0));

    // v → toggle view
    app.toggle_view();
    // View mode should have changed (just verify no panic)

    // q → quit
    app.quit();
    assert!(!app.running);
}

// ===== Pipeline 6: Full render pipeline — all panels together =====

#[test]
fn full_render_pipeline_no_panic() {
    let tmp = tempfile::TempDir::new().unwrap();
    let tasks_file = tmp.path().join("TASKS.md");
    std::fs::write(&tasks_file, include_str!("fixtures/sample_tasks.md")).unwrap();

    let events_file = tmp.path().join("events.jsonl");
    std::fs::write(
        &events_file,
        include_str!("fixtures/sample_hooks/agent_events.jsonl"),
    )
    .unwrap();

    // Build full state
    let content = std::fs::read_to_string(&tasks_file).unwrap();
    let dashboard = DashboardState::from_tasks_content(&content).unwrap();
    let mut app = App::new()
        .with_dashboard(dashboard)
        .with_tasks_path(tasks_file);

    app.handle_file_change(&FileChange::HookEventCreated(events_file));
    app.gantt_state.total_items = 11;
    app.move_down(); // select first task

    let area = Rect::new(0, 0, 120, 40);
    let layout = DashboardLayout::compute(area);
    let mut buf = Buffer::empty(area);

    // Gantt
    let gantt = GanttWidget::new(&app.dashboard, true);
    gantt.render(layout.task_list, &mut buf, &mut app.gantt_state);

    // Detail
    let detail = DetailWidget::from_selection(
        &app.dashboard,
        app.selected_task(),
        app.gantt_state.selected,
        false,
    );
    detail.render(layout.detail, &mut buf);

    // Agents
    let agents = AgentPanel::new(&app.dashboard);
    agents.render(layout.agents, &mut buf);

    // StatusBar
    let statusbar = StatusBar::new(&app.dashboard, app.start_time);
    statusbar.render(layout.status_bar, &mut buf);

    // Help overlay
    HelpOverlay.render(area, &mut buf);

    // Retry modal
    let modal = RetryModal {
        task_id: "T1".to_string(),
        task_name: "Test".to_string(),
        retryable: true,
    };
    modal.render(area, &mut buf);

    // Verify basic content present
    let text = buffer_text(&buf);
    assert!(text.contains("P0-T0.1"), "Should have task IDs");
}

// ===== Pipeline 7: Multiple file changes accumulate correctly =====

#[test]
fn sequential_file_changes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let tasks_file = tmp.path().join("TASKS.md");

    // Initial: 1 pending task
    std::fs::write(&tasks_file, "# Phase 0: Setup\n\n### [ ] T1: Task one\n").unwrap();

    let content = std::fs::read_to_string(&tasks_file).unwrap();
    let dashboard = DashboardState::from_tasks_content(&content).unwrap();
    let mut app = App::new().with_dashboard(dashboard);

    assert_eq!(app.dashboard.total_tasks, 1);
    assert_eq!(app.dashboard.completed_tasks, 0);

    // Change 1: mark task completed
    std::fs::write(&tasks_file, "# Phase 0: Setup\n\n### [x] T1: Task one\n").unwrap();
    app.handle_file_change(&FileChange::TasksModified(tasks_file.clone()));
    assert_eq!(app.dashboard.completed_tasks, 1);

    // Change 2: add a new task
    std::fs::write(
        &tasks_file,
        "# Phase 0: Setup\n\n### [x] T1: Task one\n\n### [ ] T2: Task two\n",
    )
    .unwrap();
    app.handle_file_change(&FileChange::TasksModified(tasks_file.clone()));
    assert_eq!(app.dashboard.total_tasks, 2);
    assert_eq!(app.dashboard.completed_tasks, 1);

    // Change 3: complete second task
    std::fs::write(
        &tasks_file,
        "# Phase 0: Setup\n\n### [x] T1: Task one\n\n### [x] T2: Task two\n",
    )
    .unwrap();
    app.handle_file_change(&FileChange::TasksModified(tasks_file));
    assert_eq!(app.dashboard.total_tasks, 2);
    assert_eq!(app.dashboard.completed_tasks, 2);
    assert!((app.dashboard.overall_progress - 1.0).abs() < f32::EPSILON);
}

// ===== Pipeline 8: Hook events reload doesn't duplicate =====

#[test]
fn hook_reload_no_duplicates() {
    let tmp = tempfile::TempDir::new().unwrap();
    let events_file = tmp.path().join("events.jsonl");

    // Initial: 1 event
    std::fs::write(
        &events_file,
        r#"{"event_type":"tool_start","timestamp":"2026-02-08T12:00:00Z","agent_id":"main","task_id":"T1","session_id":"s1","tool_name":"Edit"}
"#,
    )
    .unwrap();

    let mut app = App::new();
    app.handle_file_change(&FileChange::HookEventCreated(events_file.clone()));
    assert_eq!(app.dashboard.agents.get("main").unwrap().event_count, 1);

    // File grows (append another event) — simulate file modification
    std::fs::write(
        &events_file,
        r#"{"event_type":"tool_start","timestamp":"2026-02-08T12:00:00Z","agent_id":"main","task_id":"T1","session_id":"s1","tool_name":"Edit"}
{"event_type":"tool_start","timestamp":"2026-02-08T12:00:01Z","agent_id":"main","task_id":"T1","session_id":"s1","tool_name":"Bash"}
"#,
    )
    .unwrap();

    app.handle_file_change(&FileChange::HookEventModified(events_file));
    // Should be 2, not 3 (reload, not accumulate)
    assert_eq!(app.dashboard.agents.get("main").unwrap().event_count, 2);
}

// ===== Pipeline 9: Layout panels don't overlap =====

#[test]
fn layout_panels_no_overlap() {
    for (w, h) in [(80, 24), (120, 40), (160, 50)] {
        let area = Rect::new(0, 0, w, h);
        let layout = DashboardLayout::compute(area);

        // All panels have positive dimensions
        assert!(layout.task_list.width > 0, "task_list width at {w}x{h}");
        assert!(layout.task_list.height > 0, "task_list height at {w}x{h}");
        assert!(layout.detail.width > 0, "detail width at {w}x{h}");
        assert!(layout.detail.height > 0, "detail height at {w}x{h}");
        assert!(layout.agents.width > 0, "agents width at {w}x{h}");
        assert!(layout.agents.height > 0, "agents height at {w}x{h}");
        assert_eq!(layout.status_bar.height, 1, "statusbar height at {w}x{h}");

        // task_list and detail don't overlap horizontally
        assert!(
            layout.task_list.x + layout.task_list.width <= layout.detail.x
                || layout.detail.x + layout.detail.width <= layout.task_list.x,
            "task_list and detail overlap at {w}x{h}"
        );

        // status_bar at the bottom
        assert!(
            layout.status_bar.y >= layout.task_list.y + layout.task_list.height
                || layout.status_bar.y >= layout.detail.y + layout.detail.height,
            "statusbar not at bottom at {w}x{h}"
        );
    }
}
