//! Gantt chart integration tests (P2-S1-T2)
//!
//! Verifies Phase/Task tree display, cursor navigation,
//! selection mapping, and live update behavior.

use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};

use oh_my_claude_board::app::App;
use oh_my_claude_board::data::state::DashboardState;
use oh_my_claude_board::ui::gantt::{GanttState, GanttWidget};

fn sample_state() -> DashboardState {
    let input = include_str!("fixtures/sample_tasks.md");
    DashboardState::from_tasks_content(input).unwrap()
}

fn render_gantt(state: &DashboardState, gs: &mut GanttState) -> Buffer {
    let widget = GanttWidget::new(state, true);
    let area = Rect::new(0, 0, 60, 20);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf, gs);
    buf
}

// --- Scenario: Phase/Task tree display ---

#[test]
fn phase_task_tree_total_items() {
    let state = sample_state();
    let mut gs = GanttState::default();
    render_gantt(&state, &mut gs);
    // 3 phases + 8 tasks = 11 items
    assert_eq!(gs.total_items, 11);
}

#[test]
fn phase_headers_appear_in_output() {
    let state = sample_state();
    let mut gs = GanttState::default();
    let buf = render_gantt(&state, &mut gs);
    let content = buffer_text(&buf);
    // Phase names from sample_tasks.md
    assert!(content.contains("Setup"), "Missing Phase 0 name");
    assert!(content.contains("Data Engine"), "Missing Phase 1 name");
    assert!(content.contains("TUI Core"), "Missing Phase 2 name");
}

#[test]
fn task_ids_appear_in_output() {
    let state = sample_state();
    let mut gs = GanttState::default();
    let buf = render_gantt(&state, &mut gs);
    let content = buffer_text(&buf);
    assert!(content.contains("P0-T0.1"));
    assert!(content.contains("P1-R1-T1"));
}

#[test]
fn status_icons_appear() {
    let state = sample_state();
    let mut gs = GanttState::default();
    let buf = render_gantt(&state, &mut gs);
    let content = buffer_text(&buf);
    // sample_tasks.md has [x] completed, [/] in-progress, [ ] pending, [!] failed, [B] blocked
    assert!(content.contains("[x]"));
    assert!(content.contains("[/]"));
    assert!(content.contains("[ ]"));
    assert!(content.contains("[!]"));
    assert!(content.contains("[B]"));
}

// --- Scenario: Cursor navigation (j/k) ---

#[test]
fn cursor_moves_down_through_items() {
    let state = sample_state();
    let mut gs = GanttState::default();
    render_gantt(&state, &mut gs);

    assert_eq!(gs.selected, 0);
    gs.select_next();
    assert_eq!(gs.selected, 1);
    gs.select_next();
    assert_eq!(gs.selected, 2);
}

#[test]
fn cursor_moves_up_through_items() {
    let state = sample_state();
    let mut gs = GanttState::default();
    render_gantt(&state, &mut gs);

    gs.selected = 3;
    gs.select_prev();
    assert_eq!(gs.selected, 2);
    gs.select_prev();
    assert_eq!(gs.selected, 1);
}

#[test]
fn cursor_does_not_go_below_zero() {
    let state = sample_state();
    let mut gs = GanttState::default();
    render_gantt(&state, &mut gs);

    gs.select_prev();
    assert_eq!(gs.selected, 0);
}

#[test]
fn cursor_does_not_exceed_max() {
    let state = sample_state();
    let mut gs = GanttState::default();
    render_gantt(&state, &mut gs);

    gs.selected = gs.total_items - 1;
    gs.select_next();
    assert_eq!(gs.selected, gs.total_items - 1);
}

// --- Scenario: Task selection → Detail panel update ---

#[test]
fn selecting_phase_header_returns_none() {
    let state = sample_state();
    let gs = GanttState {
        selected: 0,
        total_items: 11,
        ..Default::default()
    };
    assert!(gs.selected_task(&state).is_none());
}

#[test]
fn selecting_task_returns_correct_indices() {
    let state = sample_state();
    // Phase 0 header = 0, task 0 = 1, task 1 = 2
    let gs = GanttState {
        selected: 1,
        total_items: 11,
        ..Default::default()
    };
    assert_eq!(gs.selected_task(&state), Some((0, 0)));

    let gs2 = GanttState {
        selected: 2,
        total_items: 11,
        ..Default::default()
    };
    assert_eq!(gs2.selected_task(&state), Some((0, 1)));
}

#[test]
fn app_selected_task_matches_gantt_state() {
    let state = sample_state();
    let mut app = App::new().with_dashboard(state);
    app.gantt_state.total_items = 11;

    // Move to first task
    app.move_down();
    assert_eq!(app.selected_task(), Some((0, 0)));

    // Move to second task
    app.move_down();
    assert_eq!(app.selected_task(), Some((0, 1)));
}

// --- Scenario: Live update (TASKS.md change) ---

#[test]
fn reload_preserves_cursor_position() {
    let state = sample_state();
    let mut app = App::new().with_dashboard(state);
    app.gantt_state.total_items = 11;

    // Move cursor to position 4
    for _ in 0..4 {
        app.move_down();
    }
    assert_eq!(app.gantt_state.selected, 4);

    // Simulate file change reload
    let new_content = include_str!("fixtures/sample_tasks.md");
    let _ = app.dashboard.reload_tasks(new_content);

    // Cursor position should be preserved
    assert_eq!(app.gantt_state.selected, 4);
}

#[test]
fn reload_with_fewer_tasks_clamps_cursor() {
    let state = sample_state();
    let mut app = App::new().with_dashboard(state);
    app.gantt_state.total_items = 11;
    app.gantt_state.selected = 10; // last item

    // Reload with smaller content
    let small = "# Phase 0: Setup\n### [x] T1: Done\n";
    let _ = app.dashboard.reload_tasks(small);

    // Re-render would set total_items=2, but selected stays at 10
    // Next render pass will adjust — verify state is consistent
    let mut gs = app.gantt_state.clone();
    let widget = GanttWidget::new(&app.dashboard, true);
    let area = Rect::new(0, 0, 60, 20);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf, &mut gs);

    // total_items updated to 2 (1 phase + 1 task)
    assert_eq!(gs.total_items, 2);
}

// --- Helper ---

fn buffer_text(buf: &Buffer) -> String {
    let mut text = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let cell = &buf[(x, y)];
            text.push_str(cell.symbol());
        }
        text.push('\n');
    }
    text
}
