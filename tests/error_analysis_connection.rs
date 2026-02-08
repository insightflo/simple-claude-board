//! P3-S1-V: Error Analysis connection point verification
//!
//! Validates error analysis fields flow through detail panel and retry modal,
//! navigation between detail→modal→TASKS.md, and end-to-end error data flow.

use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

use oh_my_claude_board::analysis::rules::{analyze_error, ErrorCategory};
use oh_my_claude_board::app::{App, RetryTarget};
use oh_my_claude_board::data::hook_parser;
use oh_my_claude_board::data::state::{DashboardState, ErrorRecord};
use oh_my_claude_board::data::tasks_parser::TaskStatus;
use oh_my_claude_board::event::{key_to_action, Action};
use oh_my_claude_board::ui::detail::{DetailContent, DetailWidget};
use oh_my_claude_board::ui::retry_modal::RetryModal;

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

fn full_state() -> DashboardState {
    let tasks_input = include_str!("fixtures/sample_tasks.md");
    let mut state = DashboardState::from_tasks_content(tasks_input).unwrap();
    let hooks_input = include_str!("fixtures/sample_hooks/error_events.jsonl");
    let result = hook_parser::parse_hook_events(hooks_input);
    state.update_from_events(&result.events);
    state
}

// ===== 1. Field Coverage: error_analysis fields in detail panel =====

#[test]
fn error_analysis_fields_in_detail_panel() {
    let state = full_state();

    // P1-R3-T1 is Failed — error_events.jsonl has errors for this task
    let task = &state.phases[1].tasks[2]; // Phase 1, task index 2
    assert_eq!(task.id, "P1-R3-T1");

    let errors: Vec<&ErrorRecord> = state
        .recent_errors
        .iter()
        .filter(|e| e.task_id == "P1-R3-T1")
        .collect();
    assert!(!errors.is_empty(), "Should have errors for P1-R3-T1");

    let widget = DetailWidget::new(
        DetailContent::Task(task, &state.phases[1].name, errors),
        true,
    );
    let area = Rect::new(0, 0, 80, 25);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    let text = buffer_text(&buf);
    // error_message (truncated)
    assert!(
        text.contains("permission denied"),
        "Should show error message"
    );
    // category
    assert!(text.contains("Permission"), "Should show error category");
    // retryable
    assert!(text.contains("No retry"), "Should show retryable flag");
    // suggestion
    assert!(
        text.contains("Check file permissions"),
        "Should show suggestion"
    );
}

// ===== 2. Field Coverage: retryable vs non-retryable in detail =====

#[test]
fn error_analysis_retryable_in_detail() {
    let state = full_state();
    let task = &state.phases[1].tasks[2]; // P1-R3-T1

    // Permission error (not retryable)
    let err_perm = ErrorRecord {
        agent_id: "test".to_string(),
        task_id: "P1-R3-T1".to_string(),
        message: "permission denied: /etc/shadow".to_string(),
        category: ErrorCategory::Permission,
        retryable: false,
        suggestion: "Check file permissions",
        timestamp: Utc::now(),
    };
    let widget = DetailWidget::new(
        DetailContent::Task(task, "Data Engine", vec![&err_perm]),
        true,
    );
    let area = Rect::new(0, 0, 80, 25);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let text = buffer_text(&buf);
    assert!(
        text.contains("No retry"),
        "Permission error should show No retry"
    );

    // Network error (retryable)
    let err_net = ErrorRecord {
        agent_id: "test".to_string(),
        task_id: "P1-R3-T1".to_string(),
        message: "connection refused: localhost:5432".to_string(),
        category: ErrorCategory::Network,
        retryable: true,
        suggestion: "Check if service is running",
        timestamp: Utc::now(),
    };
    let mut buf2 = Buffer::empty(area);
    let widget2 = DetailWidget::new(
        DetailContent::Task(task, "Data Engine", vec![&err_net]),
        true,
    );
    widget2.render(area, &mut buf2);
    let text2 = buffer_text(&buf2);
    assert!(text2.contains("Retry"), "Network error should show Retry");
    assert!(
        !text2.contains("No retry"),
        "Network error should NOT show No retry"
    );
}

// ===== 3. Navigation: r key maps to RetryRequest =====

#[test]
fn r_key_maps_to_retry_request() {
    let key = KeyEvent {
        code: KeyCode::Char('r'),
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    assert_eq!(key_to_action(key), Action::RetryRequest);
}

// ===== 4. Navigation: r opens retry modal on Failed task =====

#[test]
fn r_key_opens_retry_modal_on_failed_task() {
    let state = full_state();
    let mut app = App::new().with_dashboard(state);
    app.gantt_state.total_items = 11;

    // Phase 0 header(0) + 2 tasks(1,2) + Phase 1 header(3) + task(4) + task(5) + task(6=P1-R3-T1 Failed)
    app.gantt_state.selected = 6;
    let selected = app.selected_task();
    assert_eq!(selected, Some((1, 2)), "Should select Phase 1, task 2");

    app.open_retry_modal();
    assert!(app.show_retry_modal, "Modal should be open");
    assert!(app.retry_target.is_some(), "Retry target should be set");
    let target = app.retry_target.as_ref().unwrap();
    assert_eq!(target.task_id, "P1-R3-T1");
}

// ===== 5. Navigation: r ignored on non-Failed task =====

#[test]
fn r_key_ignored_on_non_failed_task() {
    let state = full_state();
    let mut app = App::new().with_dashboard(state);
    app.gantt_state.total_items = 11;

    // Select P0-T0.1 (Completed) at index 1
    app.gantt_state.selected = 1;
    app.open_retry_modal();
    assert!(
        !app.show_retry_modal,
        "Modal should NOT open for Completed task"
    );
    assert!(app.retry_target.is_none());
}

// ===== 6. Navigation: modal y updates TASKS.md =====

#[test]
fn modal_y_updates_tasks_md() {
    let tmp = tempfile::TempDir::new().unwrap();
    let tasks_file = tmp.path().join("TASKS.md");
    std::fs::write(
        &tasks_file,
        "# Phase 1: Data Engine\n\n### [Failed] P1-R3-T1: File watcher module\n- body\n",
    )
    .unwrap();

    let content = std::fs::read_to_string(&tasks_file).unwrap();
    let dashboard = DashboardState::from_tasks_content(&content).unwrap();
    let mut app = App::new()
        .with_dashboard(dashboard)
        .with_tasks_path(tasks_file.clone());

    app.show_retry_modal = true;
    app.retry_target = Some(RetryTarget {
        task_id: "P1-R3-T1".to_string(),
        task_name: "File watcher module".to_string(),
        retryable: true,
    });

    app.confirm_retry();
    assert!(!app.show_retry_modal, "Modal should close after confirm");
    assert!(app.retry_target.is_none());

    let result = std::fs::read_to_string(&tasks_file).unwrap();
    assert!(
        result.contains("[InProgress] P1-R3-T1:"),
        "File should be updated to InProgress"
    );
    assert!(!result.contains("[Failed]"), "Failed status should be gone");
}

// ===== 7. Navigation: modal n closes without write =====

#[test]
fn modal_n_closes_without_write() {
    let tmp = tempfile::TempDir::new().unwrap();
    let tasks_file = tmp.path().join("TASKS.md");
    let original = "# Phase 1: Test\n\n### [Failed] T1: Test task\n";
    std::fs::write(&tasks_file, original).unwrap();

    let content = std::fs::read_to_string(&tasks_file).unwrap();
    let dashboard = DashboardState::from_tasks_content(&content).unwrap();
    let mut app = App::new()
        .with_dashboard(dashboard)
        .with_tasks_path(tasks_file.clone());

    app.show_retry_modal = true;
    app.retry_target = Some(RetryTarget {
        task_id: "T1".to_string(),
        task_name: "Test task".to_string(),
        retryable: true,
    });

    app.cancel_retry();
    assert!(!app.show_retry_modal, "Modal should close after cancel");
    assert!(app.retry_target.is_none());

    let result = std::fs::read_to_string(&tasks_file).unwrap();
    assert_eq!(result, original, "File should be unchanged after cancel");
}

// ===== 8. Modal y reflects in dashboard state =====

#[test]
fn modal_y_reflects_in_gantt_state() {
    let tmp = tempfile::TempDir::new().unwrap();
    let tasks_file = tmp.path().join("TASKS.md");
    std::fs::write(
        &tasks_file,
        "# Phase 1: Test\n\n### [Failed] T1: Test task\n- body\n",
    )
    .unwrap();

    let content = std::fs::read_to_string(&tasks_file).unwrap();
    let dashboard = DashboardState::from_tasks_content(&content).unwrap();
    assert_eq!(dashboard.phases[0].tasks[0].status, TaskStatus::Failed);

    let mut app = App::new()
        .with_dashboard(dashboard)
        .with_tasks_path(tasks_file.clone());

    app.show_retry_modal = true;
    app.retry_target = Some(RetryTarget {
        task_id: "T1".to_string(),
        task_name: "Test task".to_string(),
        retryable: true,
    });

    app.confirm_retry();

    // Dashboard should have reloaded — task status now InProgress
    assert_eq!(
        app.dashboard.phases[0].tasks[0].status,
        TaskStatus::InProgress,
        "Task status should be InProgress after retry"
    );
}

// ===== 9. End-to-end: Hook error → analysis → ErrorRecord → detail UI =====

#[test]
fn end_to_end_error_flow() {
    // 1. Parse error JSONL
    let jsonl = include_str!("fixtures/sample_hooks/error_events.jsonl");
    let result = hook_parser::parse_hook_events(jsonl);

    // 2. Update state
    let tasks_input = include_str!("fixtures/sample_tasks.md");
    let mut state = DashboardState::from_tasks_content(tasks_input).unwrap();
    state.update_from_events(&result.events);

    // 3. Verify ErrorRecords created with correct analysis
    assert_eq!(state.recent_errors.len(), 2);

    let err0 = &state.recent_errors[0];
    assert_eq!(err0.category, ErrorCategory::Permission);
    assert!(!err0.retryable);
    assert_eq!(err0.suggestion, "Check file permissions");

    let err1 = &state.recent_errors[1];
    assert_eq!(err1.category, ErrorCategory::Network);
    assert!(err1.retryable);
    assert_eq!(err1.suggestion, "Check if service is running");

    // 4. Verify analyze_error results match
    let analysis0 = analyze_error(&err0.message);
    assert_eq!(analysis0.category, err0.category);
    assert_eq!(analysis0.retryable, err0.retryable);

    let analysis1 = analyze_error(&err1.message);
    assert_eq!(analysis1.category, err1.category);
    assert_eq!(analysis1.retryable, err1.retryable);

    // 5. Render detail and verify fields in buffer
    let task = &state.phases[1].tasks[2];
    let errors: Vec<&ErrorRecord> = state
        .recent_errors
        .iter()
        .filter(|e| e.task_id == task.id)
        .collect();
    let widget = DetailWidget::new(
        DetailContent::Task(task, &state.phases[1].name, errors),
        true,
    );
    let area = Rect::new(0, 0, 80, 30);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    let text = buffer_text(&buf);
    assert!(
        text.contains("Permission"),
        "Detail should show Permission category"
    );
    assert!(
        text.contains("Network"),
        "Detail should show Network category"
    );
    assert!(text.contains("Errors"), "Detail should show Errors header");
}

// ===== 10. Retry modal renders with correct fields =====

#[test]
fn retry_modal_renders_with_error_fields() {
    // Retryable modal
    let modal = RetryModal {
        task_id: "P1-R3-T1".to_string(),
        task_name: "File watcher".to_string(),
        retryable: true,
    };
    let area = Rect::new(0, 0, 80, 30);
    let mut buf = Buffer::empty(area);
    modal.render(area, &mut buf);

    let text = buffer_text(&buf);
    assert!(text.contains("P1-R3-T1"), "Should show task_id");
    assert!(text.contains("File watcher"), "Should show task_name");
    assert!(text.contains("Retry"), "Should show Retry title");
    assert!(text.contains("Yes"), "Should show Yes option");
    assert!(text.contains("No"), "Should show No option");

    // Non-retryable modal
    let modal2 = RetryModal {
        task_id: "P1-R3-T1".to_string(),
        task_name: "File watcher".to_string(),
        retryable: false,
    };
    let mut buf2 = Buffer::empty(area);
    modal2.render(area, &mut buf2);

    let text2 = buffer_text(&buf2);
    assert!(text2.contains("P1-R3-T1"), "Should show task_id");
    assert!(text2.contains("Not retryable"), "Should show Not retryable");
    assert!(text2.contains("Press any key"), "Should show close hint");
}
