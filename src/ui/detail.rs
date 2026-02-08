//! Task detail panel
//!
//! Shows detailed information about the currently selected task or phase.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::data::state::{DashboardState, ErrorRecord};
use crate::data::tasks_parser::{ParsedPhase, ParsedTask, TaskStatus};

/// Parse a markdown line into styled spans.
/// Handles **bold**, `code`, and plain text segments.
fn parse_md_spans(line: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut rest = line;

    while !rest.is_empty() {
        // Bold: **text**
        if let Some(start) = rest.find("**") {
            if start > 0 {
                spans.push(Span::raw(rest[..start].to_string()));
            }
            let after = &rest[start + 2..];
            if let Some(end) = after.find("**") {
                spans.push(Span::styled(
                    after[..end].to_string(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ));
                rest = &after[end + 2..];
            } else {
                spans.push(Span::raw(rest[start..].to_string()));
                return spans;
            }
        // Code: `text`
        } else if let Some(start) = rest.find('`') {
            if start > 0 {
                spans.push(Span::raw(rest[..start].to_string()));
            }
            let after = &rest[start + 1..];
            if let Some(end) = after.find('`') {
                spans.push(Span::styled(
                    after[..end].to_string(),
                    Style::default().fg(Color::Yellow),
                ));
                rest = &after[end + 1..];
            } else {
                spans.push(Span::raw(rest[start..].to_string()));
                return spans;
            }
        } else {
            spans.push(Span::raw(rest.to_string()));
            return spans;
        }
    }
    spans
}

/// What the detail panel is showing
pub enum DetailContent<'a> {
    Phase(&'a ParsedPhase),
    Task(&'a ParsedTask, &'a str, Vec<&'a ErrorRecord>), // task + phase name + errors
    None,
}

/// The detail panel widget
pub struct DetailWidget<'a> {
    content: DetailContent<'a>,
    focused: bool,
}

impl<'a> DetailWidget<'a> {
    pub fn new(content: DetailContent<'a>, focused: bool) -> Self {
        Self { content, focused }
    }

    pub fn from_selection(
        state: &'a DashboardState,
        selected_task: Option<(usize, usize)>,
        selected_index: usize,
        focused: bool,
    ) -> Self {
        let content = if let Some((pi, ti)) = selected_task {
            let phase = &state.phases[pi];
            let task = &phase.tasks[ti];
            let errors: Vec<&ErrorRecord> = state
                .recent_errors
                .iter()
                .filter(|e| e.task_id == task.id)
                .rev()
                .take(3)
                .collect();
            DetailContent::Task(task, &phase.name, errors)
        } else {
            // Check if a phase header is selected
            let mut idx = 0;
            let mut found_phase = None;
            for phase in &state.phases {
                if idx == selected_index {
                    found_phase = Some(phase);
                    break;
                }
                idx += 1 + phase.tasks.len();
            }
            match found_phase {
                Some(phase) => DetailContent::Phase(phase),
                None => DetailContent::None,
            }
        };
        Self { content, focused }
    }

    fn build_lines(&self) -> Vec<Line<'static>> {
        match &self.content {
            DetailContent::None => {
                vec![Line::styled(
                    "Select a task to view details",
                    Style::default().fg(Color::DarkGray),
                )]
            }
            DetailContent::Phase(phase) => {
                let pct = (phase.progress() * 100.0) as u8;
                let completed = phase
                    .tasks
                    .iter()
                    .filter(|t| t.status == TaskStatus::Completed)
                    .count();
                vec![
                    Line::from(vec![
                        Span::styled("Phase: ", Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            format!("{} - {}", phase.id, phase.name),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::raw(""),
                    Line::from(vec![
                        Span::styled("Progress: ", Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            format!("{completed}/{} ({pct}%)", phase.tasks.len()),
                            Style::default().fg(Color::Green),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled("Tasks:    ", Style::default().fg(Color::DarkGray)),
                        Span::raw(format!("{}", phase.tasks.len())),
                    ]),
                ]
            }
            DetailContent::Task(task, phase_name, errors) => {
                let status_str = format!("{:?}", task.status);
                let status_color = match task.status {
                    TaskStatus::Completed => Color::Green,
                    TaskStatus::InProgress => Color::Yellow,
                    TaskStatus::Pending => Color::DarkGray,
                    TaskStatus::Failed => Color::Red,
                    TaskStatus::Blocked => Color::Magenta,
                };

                let mut lines = vec![
                    Line::from(vec![
                        Span::styled("Task:   ", Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            task.id.clone(),
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled("Name:   ", Style::default().fg(Color::DarkGray)),
                        Span::raw(task.name.clone()),
                    ]),
                    Line::from(vec![
                        Span::styled("Phase:  ", Style::default().fg(Color::DarkGray)),
                        Span::raw(phase_name.to_string()),
                    ]),
                    Line::from(vec![
                        Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
                        Span::styled(status_str, Style::default().fg(status_color)),
                    ]),
                ];

                if let Some(ref agent) = task.agent {
                    lines.push(Line::from(vec![
                        Span::styled("Agent:  ", Style::default().fg(Color::DarkGray)),
                        Span::styled(format!("@{agent}"), Style::default().fg(Color::Blue)),
                    ]));
                }

                if !task.blocked_by.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("Deps:   ", Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            task.blocked_by.join(", "),
                            Style::default().fg(Color::Magenta),
                        ),
                    ]));
                }

                if !task.body.is_empty() {
                    lines.push(Line::raw(""));
                    for body_line in task.body.lines() {
                        lines.push(Line::from(parse_md_spans(body_line)));
                    }
                }

                if !errors.is_empty() {
                    lines.push(Line::raw(""));
                    lines.push(Line::styled(
                        "Errors:",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ));
                    for err in errors {
                        let msg_short = if err.message.len() > 50 {
                            format!("{}...", &err.message[..47])
                        } else {
                            err.message.clone()
                        };
                        lines.push(Line::from(vec![
                            Span::styled("  !! ", Style::default().fg(Color::Red)),
                            Span::styled(msg_short, Style::default().fg(Color::White)),
                        ]));
                        let retry_str = if err.retryable { "Retry" } else { "No retry" };
                        lines.push(Line::from(vec![
                            Span::styled("     ", Style::default()),
                            Span::styled(
                                format!("{}", err.category),
                                Style::default().fg(Color::Yellow),
                            ),
                            Span::styled(
                                format!(" | {retry_str} | {}", err.suggestion),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]));
                    }
                }

                lines
            }
        }
    }
}

impl<'a> Widget for DetailWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .title(" Detail ")
            .borders(Borders::ALL)
            .border_style(border_style);

        let lines = self.build_lines();
        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });
        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_state() -> DashboardState {
        let input = include_str!("../../tests/fixtures/sample_tasks.md");
        DashboardState::from_tasks_content(input).unwrap()
    }

    #[test]
    fn detail_none_renders() {
        let widget = DetailWidget::new(DetailContent::None, false);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
    }

    #[test]
    fn detail_phase_renders() {
        let state = sample_state();
        let widget = DetailWidget::new(DetailContent::Phase(&state.phases[0]), true);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
    }

    #[test]
    fn detail_task_renders() {
        let state = sample_state();
        let task = &state.phases[0].tasks[0];
        let widget = DetailWidget::new(DetailContent::Task(task, "Setup", vec![]), true);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
    }

    #[test]
    fn from_selection_task() {
        let state = sample_state();
        let widget = DetailWidget::from_selection(&state, Some((0, 0)), 1, true);
        let lines = widget.build_lines();
        assert!(lines.len() >= 4);
    }

    #[test]
    fn from_selection_phase() {
        let state = sample_state();
        let widget = DetailWidget::from_selection(&state, None, 0, true);
        let lines = widget.build_lines();
        assert!(lines.len() >= 3);
    }

    #[test]
    fn from_selection_none() {
        let state = sample_state();
        let widget = DetailWidget::from_selection(&state, None, 999, false);
        let lines = widget.build_lines();
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn task_with_errors_shows_error_section() {
        use crate::analysis::rules::ErrorCategory;
        use crate::data::state::ErrorRecord;
        use chrono::Utc;

        let state = sample_state();
        let task = &state.phases[0].tasks[0];
        let err = ErrorRecord {
            agent_id: "test-agent".to_string(),
            task_id: task.id.clone(),
            message: "permission denied: /etc/shadow".to_string(),
            category: ErrorCategory::Permission,
            retryable: false,
            suggestion: "Check file permissions",
            timestamp: Utc::now(),
        };
        let widget = DetailWidget::new(DetailContent::Task(task, "Setup", vec![&err]), false);
        let lines = widget.build_lines();
        let has_errors_header = lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("Errors")));
        assert!(has_errors_header, "should show Errors header");
        let has_permission = lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("Permission")));
        assert!(has_permission, "should show Permission category");
    }

    #[test]
    fn from_selection_with_errors() {
        use crate::data::hook_parser;

        let tasks_input = include_str!("../../tests/fixtures/sample_tasks.md");
        let mut state = DashboardState::from_tasks_content(tasks_input).unwrap();
        let hooks_input = include_str!("../../tests/fixtures/sample_hooks/error_events.jsonl");
        let result = hook_parser::parse_hook_events(hooks_input);
        state.update_from_events(&result.events);

        // error_events.jsonl targets task "P1-R3-T1" which may not be in sample_tasks.md
        // Verify no panic when task has no matching errors
        let widget = DetailWidget::from_selection(&state, Some((0, 0)), 1, true);
        let lines = widget.build_lines();
        assert!(lines.len() >= 4);
    }

    #[test]
    fn task_with_deps_shows_deps() {
        let state = sample_state();
        // Phase 1, task 0 has blocked_by
        let task = &state.phases[1].tasks[0];
        assert!(!task.blocked_by.is_empty());
        let widget = DetailWidget::new(DetailContent::Task(task, "Data Engine", vec![]), false);
        let lines = widget.build_lines();
        let has_deps = lines.iter().any(|l| {
            l.spans
                .iter()
                .any(|s| s.content.contains("Deps") || s.content.contains("P0-T0.1"))
        });
        assert!(has_deps);
    }

    #[test]
    fn task_with_body_shows_body_lines() {
        let state = sample_state();
        let task = &state.phases[0].tasks[0];
        assert!(!task.body.is_empty(), "fixture task should have body");
        let widget = DetailWidget::new(DetailContent::Task(task, "Setup", vec![]), false);
        let lines = widget.build_lines();
        let has_spec = lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("스펙")));
        assert!(has_spec, "detail should show body with spec line");
    }
}
