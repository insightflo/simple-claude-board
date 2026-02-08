//! Status bar widget
//!
//! Shows per-status counters, progress %, uptime, and keybinding hints.

use std::time::Instant;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::data::state::DashboardState;
use crate::data::tasks_parser::TaskStatus;

/// Status bar at the bottom of the screen
pub struct StatusBar<'a> {
    state: &'a DashboardState,
    start_time: Instant,
}

impl<'a> StatusBar<'a> {
    pub fn new(state: &'a DashboardState, start_time: Instant) -> Self {
        Self { state, start_time }
    }

    /// Count tasks by status across all phases
    fn count_by_status(&self) -> (usize, usize, usize, usize) {
        let mut completed = 0;
        let mut in_progress = 0;
        let mut failed = 0;
        let mut rest = 0; // pending + blocked

        for phase in &self.state.phases {
            for task in &phase.tasks {
                match task.status {
                    TaskStatus::Completed => completed += 1,
                    TaskStatus::InProgress => in_progress += 1,
                    TaskStatus::Failed => failed += 1,
                    TaskStatus::Pending | TaskStatus::Blocked => rest += 1,
                }
            }
        }

        (completed, in_progress, failed, rest)
    }

    /// Format elapsed duration as HH:MM:SS
    fn format_uptime(&self) -> String {
        let elapsed = self.start_time.elapsed().as_secs();
        let hours = elapsed / 3600;
        let minutes = (elapsed % 3600) / 60;
        let seconds = elapsed % 60;
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    }
}

impl<'a> Widget for StatusBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let (completed, in_progress, failed, rest) = self.count_by_status();
        let pct = (self.state.overall_progress * 100.0) as u8;
        let uptime = self.format_uptime();

        let counters = format!(" \u{2714}{completed} \u{25C0}{in_progress} \u{2718}{failed} \u{2298}{rest} ");
        let progress = format!(" {pct}% ");
        let uptime_str = format!(" uptime: {uptime} ");
        let hints = " j/k Tab Space v ? q ";

        let mut spans = vec![
            Span::styled(
                counters,
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                progress,
                Style::default().fg(Color::Black).bg(Color::Yellow),
            ),
            Span::styled(
                uptime_str,
                Style::default().fg(Color::Black).bg(Color::Cyan),
            ),
        ];

        // Fill remaining width with keybinding hints
        let used_width: usize = spans.iter().map(|s| s.content.len()).sum();
        let remaining = (area.width as usize).saturating_sub(used_width);
        if remaining > hints.len() {
            let padding = remaining - hints.len();
            spans.push(Span::raw(" ".repeat(padding)));
        }
        spans.push(Span::styled(hints, Style::default().fg(Color::DarkGray)));

        let line = Line::from(spans);
        Widget::render(line, area, buf);
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
    fn statusbar_renders() {
        let state = sample_state();
        let bar = StatusBar::new(&state, Instant::now());
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        bar.render(area, &mut buf);
    }

    #[test]
    fn statusbar_narrow_renders() {
        let state = sample_state();
        let bar = StatusBar::new(&state, Instant::now());
        let area = Rect::new(0, 0, 20, 1);
        let mut buf = Buffer::empty(area);
        bar.render(area, &mut buf);
    }

    #[test]
    fn count_by_status() {
        let state = sample_state();
        let bar = StatusBar::new(&state, Instant::now());
        let (completed, in_progress, failed, rest) = bar.count_by_status();
        assert_eq!(completed, 2);
        assert_eq!(failed, 1);
        // remaining 5 tasks are pending/blocked
        assert_eq!(completed + in_progress + failed + rest, state.total_tasks);
    }

    #[test]
    fn format_uptime_zero() {
        let state = DashboardState::default();
        let bar = StatusBar::new(&state, Instant::now());
        let uptime = bar.format_uptime();
        assert_eq!(uptime, "00:00:00");
    }
}
