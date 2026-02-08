use criterion::{black_box, criterion_group, criterion_main, Criterion};

use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget, widgets::Widget};

use oh_my_claude_board::data::state::DashboardState;
use oh_my_claude_board::ui::claude_output::AgentPanel;
use oh_my_claude_board::ui::detail::DetailWidget;
use oh_my_claude_board::ui::gantt::{GanttState, GanttWidget};
use oh_my_claude_board::ui::help::HelpOverlay;
use oh_my_claude_board::ui::layout::DashboardLayout;
use oh_my_claude_board::ui::statusbar::StatusBar;

fn sample_state() -> DashboardState {
    let tasks_input = include_str!("../tests/fixtures/sample_tasks.md");
    let mut state = DashboardState::from_tasks_content(tasks_input).unwrap();
    let hooks_input = include_str!("../tests/fixtures/sample_hooks/agent_events.jsonl");
    let result = oh_my_claude_board::data::hook_parser::parse_hook_events(hooks_input);
    state.update_from_events(&result.events);
    state
}

/// Generate a large state with many tasks for stress testing.
fn large_state() -> DashboardState {
    let mut md = String::new();
    for p in 0..20 {
        md.push_str(&format!("# Phase {p}: Phase {p} Name\n\n"));
        for t in 0..50 {
            let status = match t % 5 {
                0 => "x",
                1 => " ",
                2 => "InProgress",
                3 => "Failed",
                _ => "Blocked",
            };
            md.push_str(&format!(
                "### [{status}] P{p}-T{t}: Task {t} description\n"
            ));
            md.push_str("- **담당**: @backend-specialist\n\n");
        }
    }
    DashboardState::from_tasks_content(&md).unwrap()
}

fn bench_gantt_render(c: &mut Criterion) {
    let state = sample_state();
    let area = Rect::new(0, 0, 80, 30);

    c.bench_function("gantt_render (8 tasks)", |b| {
        b.iter(|| {
            let mut gs = GanttState::default();
            let widget = GanttWidget::new(&state, true);
            let mut buf = Buffer::empty(area);
            widget.render(black_box(area), &mut buf, &mut gs);
            black_box(buf);
        })
    });
}

fn bench_gantt_render_large(c: &mut Criterion) {
    let state = large_state();
    let area = Rect::new(0, 0, 120, 50);

    c.bench_function("gantt_render (1000 tasks)", |b| {
        b.iter(|| {
            let mut gs = GanttState::default();
            let widget = GanttWidget::new(&state, true);
            let mut buf = Buffer::empty(area);
            widget.render(black_box(area), &mut buf, &mut gs);
            black_box(buf);
        })
    });
}

fn bench_detail_render(c: &mut Criterion) {
    let state = sample_state();
    let area = Rect::new(0, 0, 50, 20);

    c.bench_function("detail_render", |b| {
        b.iter(|| {
            let widget =
                DetailWidget::from_selection(&state, Some((0, 0)), 1, true);
            let mut buf = Buffer::empty(area);
            widget.render(black_box(area), &mut buf);
            black_box(buf);
        })
    });
}

fn bench_agent_panel_render(c: &mut Criterion) {
    let state = sample_state();
    let area = Rect::new(0, 0, 60, 10);

    c.bench_function("agent_panel_render", |b| {
        b.iter(|| {
            let panel = AgentPanel::new(&state);
            let mut buf = Buffer::empty(area);
            panel.render(black_box(area), &mut buf);
            black_box(buf);
        })
    });
}

fn bench_statusbar_render(c: &mut Criterion) {
    let state = sample_state();
    let start = std::time::Instant::now();
    let area = Rect::new(0, 0, 120, 1);

    c.bench_function("statusbar_render", |b| {
        b.iter(|| {
            let bar = StatusBar::new(&state, start);
            let mut buf = Buffer::empty(area);
            bar.render(black_box(area), &mut buf);
            black_box(buf);
        })
    });
}

fn bench_help_overlay_render(c: &mut Criterion) {
    let area = Rect::new(0, 0, 80, 30);

    c.bench_function("help_overlay_render", |b| {
        b.iter(|| {
            let mut buf = Buffer::empty(area);
            HelpOverlay.render(black_box(area), &mut buf);
            black_box(buf);
        })
    });
}

fn bench_full_frame_render(c: &mut Criterion) {
    let state = sample_state();
    let start = std::time::Instant::now();
    let area = Rect::new(0, 0, 120, 40);

    c.bench_function("full_frame_render (all panels)", |b| {
        b.iter(|| {
            let layout = DashboardLayout::compute(area);
            let mut buf = Buffer::empty(area);

            let mut gs = GanttState::default();
            let gantt = GanttWidget::new(&state, true);
            gantt.render(layout.task_list, &mut buf, &mut gs);

            let detail = DetailWidget::from_selection(&state, Some((0, 0)), 1, false);
            detail.render(layout.detail, &mut buf);

            let agents = AgentPanel::new(&state);
            agents.render(layout.agents, &mut buf);

            let statusbar = StatusBar::new(&state, start);
            statusbar.render(layout.status_bar, &mut buf);

            black_box(buf);
        })
    });
}

fn bench_layout_compute(c: &mut Criterion) {
    let area = Rect::new(0, 0, 120, 40);

    c.bench_function("layout_compute", |b| {
        b.iter(|| {
            black_box(DashboardLayout::compute(black_box(area)));
        })
    });
}

criterion_group!(
    benches,
    bench_gantt_render,
    bench_gantt_render_large,
    bench_detail_render,
    bench_agent_panel_render,
    bench_statusbar_render,
    bench_help_overlay_render,
    bench_full_frame_render,
    bench_layout_compute,
);
criterion_main!(benches);
