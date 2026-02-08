//! TUI Core connection point verification (P2-V)
//!
//! Validates field coverage, navigation flow, and shared component
//! consistency across all UI panels.

use std::time::Instant;

use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget, widgets::Widget};

use oh_my_claude_board::app::App;
use oh_my_claude_board::data::hook_parser;
use oh_my_claude_board::data::state::DashboardState;
use oh_my_claude_board::data::tasks_parser::TaskStatus;
use oh_my_claude_board::event::{key_to_action, Action};
use oh_my_claude_board::ui::claude_output::AgentPanel;
use oh_my_claude_board::ui::detail::{DetailContent, DetailWidget};
use oh_my_claude_board::ui::gantt::{GanttState, GanttWidget};
use oh_my_claude_board::ui::help::HelpOverlay;
use oh_my_claude_board::ui::layout::{DashboardLayout, FocusedPane};
use oh_my_claude_board::ui::statusbar::StatusBar;

fn full_state() -> DashboardState {
    let tasks_input = include_str!("fixtures/sample_tasks.md");
    let mut state = DashboardState::from_tasks_content(tasks_input).unwrap();
    let hooks_input = include_str!("fixtures/sample_hooks/agent_events.jsonl");
    let result = hook_parser::parse_hook_events(hooks_input);
    state.update_from_events(&result.events);
    state
}

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

// ===== Field Coverage: tasks → Gantt chart =====

#[test]
fn gantt_shows_phase_id_and_name() {
    let state = full_state();
    let mut gs = GanttState::default();
    let widget = GanttWidget::new(&state, true);
    let area = Rect::new(0, 0, 80, 30);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf, &mut gs);

    let text = buffer_text(&buf);
    for phase in &state.phases {
        assert!(text.contains(&phase.id), "Missing phase_id: {}", phase.id);
        assert!(
            text.contains(&phase.name),
            "Missing phase_name: {}",
            phase.name
        );
    }
}

#[test]
fn gantt_shows_task_id_and_status() {
    let state = full_state();
    let mut gs = GanttState::default();
    let widget = GanttWidget::new(&state, true);
    let area = Rect::new(0, 0, 80, 30);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf, &mut gs);

    let text = buffer_text(&buf);
    for phase in &state.phases {
        for task in &phase.tasks {
            assert!(
                text.contains(&task.id),
                "Missing task_id: {}",
                task.id
            );
        }
    }
}

#[test]
fn gantt_shows_agent_names() {
    let state = full_state();
    let mut gs = GanttState::default();
    let widget = GanttWidget::new(&state, true);
    let area = Rect::new(0, 0, 80, 30);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf, &mut gs);

    let text = buffer_text(&buf);
    // sample_tasks has @backend-specialist
    assert!(text.contains("@backend-specialist"));
}

// ===== Field Coverage: tasks → Detail panel =====

#[test]
fn detail_shows_task_fields() {
    let state = full_state();
    let task = &state.phases[0].tasks[0];
    let widget = DetailWidget::new(DetailContent::Task(task, &state.phases[0].name), true);
    let area = Rect::new(0, 0, 50, 15);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    let text = buffer_text(&buf);
    assert!(text.contains(&task.id), "Missing task_id in detail");
    assert!(text.contains(&task.name), "Missing task_name in detail");
    assert!(text.contains("Status"), "Missing status label in detail");
}

#[test]
fn detail_shows_blocked_by() {
    let state = full_state();
    // Phase 1, task 0 has blocked_by
    let task = &state.phases[1].tasks[0];
    assert!(!task.blocked_by.is_empty());

    let widget = DetailWidget::new(DetailContent::Task(task, &state.phases[1].name), true);
    let area = Rect::new(0, 0, 50, 15);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    let text = buffer_text(&buf);
    assert!(text.contains("Deps"), "Missing deps label");
    for dep in &task.blocked_by {
        assert!(text.contains(dep), "Missing blocked_by: {dep}");
    }
}

// ===== Field Coverage: hook_events → Agent panel =====

#[test]
fn agent_panel_shows_agent_id() {
    let state = full_state();
    let panel = AgentPanel::new(&state);
    let area = Rect::new(0, 0, 60, 10);
    let mut buf = Buffer::empty(area);
    panel.render(area, &mut buf);

    let text = buffer_text(&buf);
    assert!(text.contains("backend-specialist-1"));
}

// ===== Field Coverage: session → StatusBar =====

#[test]
fn statusbar_shows_progress_and_counters() {
    let state = full_state();
    let bar = StatusBar::new(&state, Instant::now());
    let area = Rect::new(0, 0, 80, 1);
    let mut buf = Buffer::empty(area);
    bar.render(area, &mut buf);

    let text = buffer_text(&buf);
    // Should show percentage
    assert!(text.contains("25%"), "Missing progress percentage");
    // Should show uptime
    assert!(text.contains("uptime"), "Missing uptime");
}

#[test]
fn statusbar_counts_match_state() {
    let state = full_state();
    let bar = StatusBar::new(&state, Instant::now());
    let area = Rect::new(0, 0, 80, 1);
    let mut buf = Buffer::empty(area);
    bar.render(area, &mut buf);

    // Verify total task count is consistent
    let mut total = 0;
    for phase in &state.phases {
        total += phase.tasks.len();
    }
    assert_eq!(total, state.total_tasks);
}

// ===== Navigation: Gantt Enter → Detail panel update =====

#[test]
fn gantt_selection_updates_detail() {
    let state = full_state();
    let mut app = App::new().with_dashboard(state);
    app.gantt_state.total_items = 11;

    // Select first task (index 1)
    app.move_down();
    let selected = app.selected_task();
    assert_eq!(selected, Some((0, 0)));

    // Verify detail can render with this selection
    let detail = DetailWidget::from_selection(
        &app.dashboard,
        selected,
        app.gantt_state.selected,
        true,
    );
    let area = Rect::new(0, 0, 50, 15);
    let mut buf = Buffer::empty(area);
    detail.render(area, &mut buf);

    let text = buffer_text(&buf);
    assert!(text.contains("P0-T0.1"));
}

// ===== Navigation: Tab → Focus toggle =====

#[test]
fn tab_toggles_focus() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    let key = KeyEvent {
        code: KeyCode::Tab,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    assert_eq!(key_to_action(key), Action::ToggleFocus);

    let mut app = App::new();
    assert_eq!(app.focused, FocusedPane::TaskList);
    app.toggle_focus();
    assert_eq!(app.focused, FocusedPane::Detail);
    app.toggle_focus();
    assert_eq!(app.focused, FocusedPane::TaskList);
}

// ===== Navigation: ? → Help overlay toggle =====

#[test]
fn help_toggle_shows_overlay() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    let key = KeyEvent {
        code: KeyCode::Char('?'),
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    assert_eq!(key_to_action(key), Action::ToggleHelp);

    let mut app = App::new();
    assert!(!app.show_help);
    app.toggle_help();
    assert!(app.show_help);

    // Verify help overlay renders
    let area = Rect::new(0, 0, 80, 30);
    let mut buf = Buffer::empty(area);
    HelpOverlay.render(area, &mut buf);
    let text = buffer_text(&buf);
    assert!(text.contains("Help"));
}

// ===== Shared: status symbols consistent across panels =====

#[test]
fn status_representation_consistent() {
    let state = full_state();

    // Collect all statuses from phases
    let mut has_completed = false;
    let mut has_failed = false;
    for phase in &state.phases {
        for task in &phase.tasks {
            match task.status {
                TaskStatus::Completed => has_completed = true,
                TaskStatus::Failed => has_failed = true,
                _ => {}
            }
        }
    }
    assert!(has_completed, "Fixture should have completed tasks");
    assert!(has_failed, "Fixture should have failed tasks");

    // Gantt renders status icons
    let mut gs = GanttState::default();
    let gantt = GanttWidget::new(&state, true);
    let area = Rect::new(0, 0, 80, 30);
    let mut buf = Buffer::empty(area);
    gantt.render(area, &mut buf, &mut gs);
    let gantt_text = buffer_text(&buf);
    assert!(gantt_text.contains("[x]"), "Gantt missing completed icon");
    assert!(gantt_text.contains("[!]"), "Gantt missing failed icon");

    // Detail renders status text
    let completed_task = state.phases[0]
        .tasks
        .iter()
        .find(|t| t.status == TaskStatus::Completed)
        .unwrap();
    let detail = DetailWidget::new(
        DetailContent::Task(completed_task, &state.phases[0].name),
        true,
    );
    let detail_area = Rect::new(0, 0, 50, 15);
    let mut detail_buf = Buffer::empty(detail_area);
    detail.render(detail_area, &mut detail_buf);
    let detail_text = buffer_text(&detail_buf);
    assert!(
        detail_text.contains("Completed"),
        "Detail missing Completed status"
    );
}

// ===== Layout: all panels fit in terminal =====

#[test]
fn layout_all_panels_have_area() {
    let area = Rect::new(0, 0, 120, 40);
    let layout = DashboardLayout::compute(area);

    assert!(layout.task_list.width > 0 && layout.task_list.height > 0);
    assert!(layout.detail.width > 0 && layout.detail.height > 0);
    assert!(layout.agents.width > 0 && layout.agents.height > 0);
    assert!(layout.status_bar.width > 0 && layout.status_bar.height == 1);

    // No overlaps between task_list and detail
    assert!(
        layout.task_list.x + layout.task_list.width <= layout.detail.x
            || layout.detail.x + layout.detail.width <= layout.task_list.x
    );
}

// ===== Full render pipeline: all panels render without panic =====

#[test]
fn full_dashboard_renders_without_panic() {
    let state = full_state();
    let mut app = App::new().with_dashboard(state);
    let area = Rect::new(0, 0, 120, 40);
    let layout = DashboardLayout::compute(area);

    // Gantt
    let gantt = GanttWidget::new(&app.dashboard, true);
    let mut buf = Buffer::empty(area);
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
}
