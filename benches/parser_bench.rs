use criterion::{black_box, criterion_group, criterion_main, Criterion};

use oh_my_claude_board::data::state::DashboardState;
use oh_my_claude_board::data::tasks_parser;

/// Generate a large TASKS.md with N phases, each containing M tasks.
fn generate_large_tasks_md(phases: usize, tasks_per_phase: usize) -> String {
    let mut md = String::new();
    for p in 0..phases {
        md.push_str(&format!("# Phase {p}: Phase {p} Name\n\n"));
        for t in 0..tasks_per_phase {
            let status = match t % 5 {
                0 => "x",
                1 => " ",
                2 => "InProgress",
                3 => "Failed",
                _ => "Blocked",
            };
            md.push_str(&format!(
                "### [{status}] P{p}-T{t}: Task {t} description here\n"
            ));
            md.push_str(&format!("- **담당**: @backend-specialist\n"));
            if t > 0 {
                md.push_str(&format!("- **blocked_by**: P{p}-T{}\n", t - 1));
            }
            md.push('\n');
        }
    }
    md
}

fn bench_parse_sample(c: &mut Criterion) {
    let input = include_str!("../tests/fixtures/sample_tasks.md");
    c.bench_function("parse_sample_tasks_md (8 tasks)", |b| {
        b.iter(|| tasks_parser::parse_tasks_md(black_box(input)).unwrap())
    });
}

fn bench_parse_100_tasks(c: &mut Criterion) {
    let input = generate_large_tasks_md(10, 10);
    c.bench_function("parse_100_tasks (10 phases x 10)", |b| {
        b.iter(|| tasks_parser::parse_tasks_md(black_box(&input)).unwrap())
    });
}

fn bench_parse_1000_tasks(c: &mut Criterion) {
    let input = generate_large_tasks_md(20, 50);
    c.bench_function("parse_1000_tasks (20 phases x 50)", |b| {
        b.iter(|| tasks_parser::parse_tasks_md(black_box(&input)).unwrap())
    });
}

fn bench_state_from_content(c: &mut Criterion) {
    let input = generate_large_tasks_md(20, 50);
    c.bench_function("state_from_1000_tasks", |b| {
        b.iter(|| DashboardState::from_tasks_content(black_box(&input)).unwrap())
    });
}

fn bench_hook_events_parse(c: &mut Criterion) {
    let input = include_str!("../tests/fixtures/sample_hooks/agent_events.jsonl");
    c.bench_function("parse_hook_events (6 events)", |b| {
        b.iter(|| oh_my_claude_board::data::hook_parser::parse_hook_events(black_box(input)))
    });
}

fn bench_hook_events_large(c: &mut Criterion) {
    // Generate 1000 hook events
    let mut jsonl = String::new();
    for i in 0..1000 {
        jsonl.push_str(&format!(
            r#"{{"event_type":"tool_start","timestamp":"2026-02-08T12:{:02}:{:02}.000Z","agent_id":"main","task_id":"T1","session_id":"s1","tool_name":"Edit"}}"#,
            i / 60 % 60,
            i % 60
        ));
        jsonl.push('\n');
    }
    c.bench_function("parse_hook_events (1000 events)", |b| {
        b.iter(|| oh_my_claude_board::data::hook_parser::parse_hook_events(black_box(&jsonl)))
    });
}

fn bench_error_analysis(c: &mut Criterion) {
    use oh_my_claude_board::analysis::rules::analyze_error;

    let messages = [
        "permission denied: /etc/shadow",
        "connection refused: localhost:5432",
        "request timed out after 30s",
        "type error: expected i32 got &str",
        "something completely unexpected",
    ];

    c.bench_function("analyze_error (5 patterns)", |b| {
        b.iter(|| {
            for msg in &messages {
                black_box(analyze_error(msg));
            }
        })
    });
}

criterion_group!(
    benches,
    bench_parse_sample,
    bench_parse_100_tasks,
    bench_parse_1000_tasks,
    bench_state_from_content,
    bench_hook_events_parse,
    bench_hook_events_large,
    bench_error_analysis,
);
criterion_main!(benches);
