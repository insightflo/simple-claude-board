//! Gantt chart widget
//!
//! Two view modes:
//! - Tree: phases with `▼`/`▶` collapse, tree connectors `├─`/`└─`, progress bars
//! - HorizontalBar: time-based horizontal bar chart per task

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, StatefulWidget, Widget},
};

use crate::data::state::DashboardState;
use crate::data::tasks_parser::TaskStatus;

/// View mode for the gantt panel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GanttViewMode {
    #[default]
    Tree,
    HorizontalBar,
}

/// Selection state for the gantt view
#[derive(Debug, Default, Clone)]
pub struct GanttState {
    /// Index into the flattened visible list (phases + visible tasks)
    pub selected: usize,
    /// Total number of selectable items
    pub total_items: usize,
    /// Scroll offset for vertical scrolling
    pub offset: usize,
    /// Collapsed phase indices
    pub collapsed: HashSet<usize>,
    /// Current view mode
    pub view_mode: GanttViewMode,
}

impl GanttState {
    pub fn select_next(&mut self) {
        if self.total_items > 0 {
            self.selected = (self.selected + 1).min(self.total_items - 1);
        }
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Toggle collapse for a phase at the given phase_index
    pub fn toggle_collapse(&mut self, phase_index: usize) {
        if self.collapsed.contains(&phase_index) {
            self.collapsed.remove(&phase_index);
        } else {
            self.collapsed.insert(phase_index);
        }
    }

    /// Toggle the view mode between Tree and HorizontalBar
    pub fn toggle_view(&mut self) {
        self.view_mode = match self.view_mode {
            GanttViewMode::Tree => GanttViewMode::HorizontalBar,
            GanttViewMode::HorizontalBar => GanttViewMode::Tree,
        };
    }

    /// Get the phase index if the current selection is a phase header.
    /// Accounts for collapsed phases hiding their tasks.
    pub fn selected_phase_index(&self, state: &DashboardState) -> Option<usize> {
        let mut idx = 0;
        for (pi, phase) in state.phases.iter().enumerate() {
            if idx == self.selected {
                return Some(pi);
            }
            idx += 1;
            if !self.collapsed.contains(&pi) {
                idx += phase.tasks.len();
            }
        }
        None
    }

    /// Get the (phase_idx, task_idx) for the current selection.
    /// Returns None if a phase header is selected or out of range.
    pub fn selected_task(&self, state: &DashboardState) -> Option<(usize, usize)> {
        let mut idx = 0;
        for (pi, phase) in state.phases.iter().enumerate() {
            if idx == self.selected {
                return None; // phase header selected
            }
            idx += 1;
            if !self.collapsed.contains(&pi) {
                for ti in 0..phase.tasks.len() {
                    if idx == self.selected {
                        return Some((pi, ti));
                    }
                    idx += 1;
                }
            }
        }
        None
    }
}

/// Color for a task status
fn status_color(status: &TaskStatus) -> Color {
    match status {
        TaskStatus::Completed => Color::Green,
        TaskStatus::InProgress => Color::Yellow,
        TaskStatus::Pending => Color::DarkGray,
        TaskStatus::Failed => Color::Red,
        TaskStatus::Blocked => Color::Magenta,
    }
}

/// Status icon character
fn status_icon(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Completed => "[x]",
        TaskStatus::InProgress => "[/]",
        TaskStatus::Pending => "[ ]",
        TaskStatus::Failed => "[!]",
        TaskStatus::Blocked => "[B]",
    }
}

/// Build a small progress bar string like `████░░`
fn progress_bar(ratio: f32, width: usize) -> String {
    let filled = (ratio * width as f32).round() as usize;
    let empty = width.saturating_sub(filled);
    let mut bar = String::with_capacity(width * 3);
    for _ in 0..filled {
        bar.push('\u{2588}'); // █
    }
    for _ in 0..empty {
        bar.push('\u{2591}'); // ░
    }
    bar
}

/// The Gantt widget renders the dashboard state as a scrollable task list
pub struct GanttWidget<'a> {
    state: &'a DashboardState,
    focused: bool,
}

impl<'a> GanttWidget<'a> {
    pub fn new(state: &'a DashboardState, focused: bool) -> Self {
        Self { state, focused }
    }

    /// Build lines for the tree view (with collapse, connectors, progress bars)
    fn build_tree_lines(&self, gantt_state: &GanttState) -> Vec<(Line<'static>, bool)> {
        let mut lines = Vec::new();
        let mut idx = 0;

        for (pi, phase) in self.state.phases.iter().enumerate() {
            let is_selected = idx == gantt_state.selected;
            let is_collapsed = gantt_state.collapsed.contains(&pi);
            let progress = phase.progress();
            let pct = (progress * 100.0) as u8;
            let arrow = if is_collapsed { "\u{25B6}" } else { "\u{25BC}" };
            let bar = progress_bar(progress, 6);

            let header = Line::from(vec![
                Span::styled(
                    format!(" {arrow} "),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!("{} ", phase.id),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    phase.name.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(bar, Style::default().fg(Color::Green)),
                Span::styled(format!(" {pct}%"), Style::default().fg(Color::DarkGray)),
            ]);
            lines.push((header, is_selected));
            idx += 1;

            if is_collapsed {
                continue;
            }

            let task_count = phase.tasks.len();
            for (ti, task) in phase.tasks.iter().enumerate() {
                let is_selected = idx == gantt_state.selected;
                let icon = status_icon(&task.status);
                let color = status_color(&task.status);
                let connector = if ti == task_count - 1 {
                    "\u{2514}\u{2500}"
                } else {
                    "\u{251C}\u{2500}"
                };
                let agent_str = task
                    .agent
                    .as_deref()
                    .map(|a| format!(" @{a}"))
                    .unwrap_or_default();

                let line = Line::from(vec![
                    Span::styled(
                        format!("  {connector} "),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(icon.to_string(), Style::default().fg(color)),
                    Span::raw(" "),
                    Span::styled(
                        task.id.clone(),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(": "),
                    Span::raw(task.name.clone()),
                    Span::styled(agent_str, Style::default().fg(Color::Blue)),
                ]);
                lines.push((line, is_selected));
                idx += 1;
            }
        }
        lines
    }

    /// Build lines for the horizontal bar view
    fn build_bar_lines(&self, gantt_state: &GanttState) -> Vec<(Line<'static>, bool)> {
        // Collect all tasks with their timing info
        type TaskRow<'b> = (&'b str, &'b TaskStatus, Option<DateTime<Utc>>, Option<DateTime<Utc>>);
        let mut rows: Vec<TaskRow<'_>> = Vec::new();

        for phase in &self.state.phases {
            for task in &phase.tasks {
                let timing = self.state.task_times.get(&task.id);
                let started = timing.and_then(|t| t.started_at);
                let completed = timing.and_then(|t| t.completed_at);
                rows.push((&task.id, &task.status, started, completed));
            }
        }

        if rows.is_empty() {
            return vec![(Line::raw("  No tasks"), false)];
        }

        // Find time bounds from all tasks with timing data
        let now = Utc::now();
        let earliest = rows
            .iter()
            .filter_map(|(_, _, s, _)| *s)
            .min()
            .unwrap_or(now);
        let latest = rows
            .iter()
            .filter_map(|(_, _, _, c)| *c)
            .max()
            .unwrap_or(now);
        let total_secs = (latest - earliest).num_seconds().max(1) as f64;

        // Determine label width (max task id length + padding)
        let label_width = rows.iter().map(|(id, _, _, _)| id.len()).max().unwrap_or(8) + 1;

        // Build header with time scale
        let bar_area_width = 30usize;
        let duration_mins = total_secs / 60.0;
        let time_header = build_time_header(label_width, bar_area_width, duration_mins);
        let mut lines = vec![(time_header, false)];

        // Build bar rows
        for (ri, (task_id, status, started, completed)) in rows.iter().enumerate() {
            // +1 for time header row
            let is_selected = (ri + 1) == gantt_state.selected;
            let color = status_color(status);

            // Pad task id to label width
            let label = format!("{:>width$} ", task_id, width = label_width);

            // Calculate bar position and length
            let (bar_start, bar_len) = match (started, completed) {
                (Some(s), Some(c)) => {
                    let start_offset = (*s - earliest).num_seconds().max(0) as f64 / total_secs;
                    let end_offset = (*c - earliest).num_seconds().max(0) as f64 / total_secs;
                    let col = (start_offset * bar_area_width as f64) as usize;
                    let len =
                        ((end_offset - start_offset) * bar_area_width as f64).ceil() as usize;
                    (col, len.max(1))
                }
                (Some(s), None) => {
                    // In progress: bar from start to now
                    let start_offset = (*s - earliest).num_seconds().max(0) as f64 / total_secs;
                    let end_offset = (now - earliest).num_seconds().max(0) as f64 / total_secs;
                    let col = (start_offset * bar_area_width as f64) as usize;
                    let len =
                        ((end_offset - start_offset) * bar_area_width as f64).ceil() as usize;
                    (col, len.max(1))
                }
                _ => {
                    // No timing: place at estimated position by row order
                    let pos =
                        (ri as f64 / rows.len().max(1) as f64 * bar_area_width as f64) as usize;
                    (pos, 2)
                }
            };

            let bar_char = match status {
                TaskStatus::Completed | TaskStatus::InProgress => '\u{2588}', // █
                _ => '\u{2591}',                                              // ░
            };

            let mut bar = String::new();
            for i in 0..bar_area_width {
                if i >= bar_start && i < bar_start + bar_len {
                    bar.push(bar_char);
                } else {
                    bar.push(' ');
                }
            }

            let line = Line::from(vec![
                Span::styled(label, Style::default().fg(Color::White)),
                Span::styled(bar, Style::default().fg(color)),
            ]);
            lines.push((line, is_selected));
        }

        lines
    }
}

/// Build a time header for the horizontal bar view
fn build_time_header(label_width: usize, bar_width: usize, total_mins: f64) -> Line<'static> {
    let padding = " ".repeat(label_width + 1);
    if total_mins < 1.0 {
        let secs = (total_mins * 60.0) as u64;
        let mid = secs / 2;
        let mut scale = "0s".to_string();
        let mid_pos = bar_width / 2;
        while scale.len() < mid_pos {
            scale.push(' ');
        }
        scale.push_str(&format!("{mid}s"));
        while scale.len() < bar_width {
            scale.push(' ');
        }
        scale.push_str(&format!("{secs}s"));
        Line::from(vec![
            Span::raw(padding),
            Span::styled(scale, Style::default().fg(Color::DarkGray)),
        ])
    } else {
        let total = total_mins.ceil() as u64;
        let mid = total / 2;
        let mut scale = "0m".to_string();
        let mid_pos = bar_width / 2;
        while scale.len() < mid_pos {
            scale.push(' ');
        }
        scale.push_str(&format!("{mid}m"));
        while scale.len() < bar_width {
            scale.push(' ');
        }
        scale.push_str(&format!("{total}m"));
        Line::from(vec![
            Span::raw(padding),
            Span::styled(scale, Style::default().fg(Color::DarkGray)),
        ])
    }
}

/// Shared rendering logic for both view modes
fn render_lines(
    lines: &[(Line<'_>, bool)],
    inner: Rect,
    buf: &mut Buffer,
    gantt_state: &mut GanttState,
    focused: bool,
) {
    gantt_state.total_items = lines.len();

    // Adjust scroll offset to keep selection visible
    let visible_height = inner.height as usize;
    if gantt_state.selected < gantt_state.offset {
        gantt_state.offset = gantt_state.selected;
    } else if gantt_state.selected >= gantt_state.offset + visible_height {
        gantt_state.offset = gantt_state.selected - visible_height + 1;
    }

    for (i, (line, is_selected)) in lines
        .iter()
        .skip(gantt_state.offset)
        .enumerate()
        .take(visible_height)
    {
        let y = inner.y + i as u16;
        if y >= inner.y + inner.height {
            break;
        }

        if *is_selected && focused {
            buf.set_style(
                Rect::new(inner.x, y, inner.width, 1),
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );
        }

        let line_area = Rect::new(inner.x, y, inner.width, 1);
        Widget::render(line.clone(), line_area, buf);
    }
}

impl<'a> StatefulWidget for GanttWidget<'a> {
    type State = GanttState;

    fn render(self, area: Rect, buf: &mut Buffer, gantt_state: &mut Self::State) {
        let border_style = if self.focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let view_label = match gantt_state.view_mode {
            GanttViewMode::Tree => " Tasks (Tree) ",
            GanttViewMode::HorizontalBar => " Tasks (Gantt) ",
        };

        let block = Block::default()
            .title(view_label)
            .borders(Borders::ALL)
            .border_style(border_style);
        let inner = block.inner(area);
        block.render(area, buf);

        let lines = match gantt_state.view_mode {
            GanttViewMode::Tree => self.build_tree_lines(gantt_state),
            GanttViewMode::HorizontalBar => self.build_bar_lines(gantt_state),
        };

        render_lines(&lines, inner, buf, gantt_state, self.focused);
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
    fn gantt_state_navigation() {
        let mut gs = GanttState {
            selected: 0,
            total_items: 5,
            ..Default::default()
        };
        gs.select_next();
        assert_eq!(gs.selected, 1);
        gs.select_prev();
        assert_eq!(gs.selected, 0);
        gs.select_prev(); // should not go below 0
        assert_eq!(gs.selected, 0);
    }

    #[test]
    fn gantt_state_max_bound() {
        let mut gs = GanttState {
            selected: 4,
            total_items: 5,
            ..Default::default()
        };
        gs.select_next(); // should cap at 4
        assert_eq!(gs.selected, 4);
    }

    #[test]
    fn selected_task_phase_header() {
        let state = sample_state();
        let gs = GanttState {
            selected: 0,
            total_items: 11,
            ..Default::default()
        };
        assert!(gs.selected_task(&state).is_none());
    }

    #[test]
    fn selected_task_first_task() {
        let state = sample_state();
        let gs = GanttState {
            selected: 1,
            total_items: 11,
            ..Default::default()
        };
        assert_eq!(gs.selected_task(&state), Some((0, 0)));
    }

    #[test]
    fn selected_task_second_phase() {
        let state = sample_state();
        // Phase 0: header(0) + 2 tasks(1,2) = 3 items
        // Phase 1: header(3)
        let gs = GanttState {
            selected: 3,
            total_items: 11,
            ..Default::default()
        };
        assert!(gs.selected_task(&state).is_none()); // phase 1 header
        let gs2 = GanttState {
            selected: 4,
            total_items: 11,
            ..Default::default()
        };
        assert_eq!(gs2.selected_task(&state), Some((1, 0)));
    }

    #[test]
    fn status_colors_all_mapped() {
        assert_eq!(status_color(&TaskStatus::Completed), Color::Green);
        assert_eq!(status_color(&TaskStatus::InProgress), Color::Yellow);
        assert_eq!(status_color(&TaskStatus::Pending), Color::DarkGray);
        assert_eq!(status_color(&TaskStatus::Failed), Color::Red);
        assert_eq!(status_color(&TaskStatus::Blocked), Color::Magenta);
    }

    #[test]
    fn status_icons_all_mapped() {
        assert_eq!(status_icon(&TaskStatus::Completed), "[x]");
        assert_eq!(status_icon(&TaskStatus::InProgress), "[/]");
        assert_eq!(status_icon(&TaskStatus::Pending), "[ ]");
        assert_eq!(status_icon(&TaskStatus::Failed), "[!]");
        assert_eq!(status_icon(&TaskStatus::Blocked), "[B]");
    }

    #[test]
    fn build_tree_lines_count() {
        let state = sample_state();
        let widget = GanttWidget::new(&state, true);
        let gs = GanttState::default();
        let lines = widget.build_tree_lines(&gs);
        // 3 phases + 8 tasks = 11 lines
        assert_eq!(lines.len(), 11);
    }

    #[test]
    fn build_tree_lines_collapsed() {
        let state = sample_state();
        let widget = GanttWidget::new(&state, true);
        let mut gs = GanttState::default();
        gs.collapsed.insert(0); // collapse phase 0 (2 tasks hidden)
        let lines = widget.build_tree_lines(&gs);
        // 3 phases + (0 + 3 + 3) tasks = 9 lines
        assert_eq!(lines.len(), 9);
    }

    #[test]
    fn selected_task_with_collapse() {
        let state = sample_state();
        let mut gs = GanttState {
            selected: 1,
            total_items: 9,
            ..Default::default()
        };
        gs.collapsed.insert(0); // collapse phase 0
        // selected=0 is phase 0 header
        // selected=1 is phase 1 header (tasks of phase 0 hidden)
        assert!(gs.selected_task(&state).is_none());
        assert_eq!(gs.selected_phase_index(&state), Some(1));

        gs.selected = 2;
        assert_eq!(gs.selected_task(&state), Some((1, 0)));
    }

    #[test]
    fn toggle_collapse() {
        let mut gs = GanttState::default();
        assert!(!gs.collapsed.contains(&0));
        gs.toggle_collapse(0);
        assert!(gs.collapsed.contains(&0));
        gs.toggle_collapse(0);
        assert!(!gs.collapsed.contains(&0));
    }

    #[test]
    fn toggle_view() {
        let mut gs = GanttState::default();
        assert_eq!(gs.view_mode, GanttViewMode::Tree);
        gs.toggle_view();
        assert_eq!(gs.view_mode, GanttViewMode::HorizontalBar);
        gs.toggle_view();
        assert_eq!(gs.view_mode, GanttViewMode::Tree);
    }

    #[test]
    fn progress_bar_full() {
        let bar = progress_bar(1.0, 6);
        assert_eq!(bar, "\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}");
    }

    #[test]
    fn progress_bar_empty() {
        let bar = progress_bar(0.0, 6);
        assert_eq!(bar, "\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}");
    }

    #[test]
    fn progress_bar_half() {
        let bar = progress_bar(0.5, 6);
        assert_eq!(
            bar,
            "\u{2588}\u{2588}\u{2588}\u{2591}\u{2591}\u{2591}"
        );
    }

    #[test]
    fn render_tree_does_not_panic() {
        let state = sample_state();
        let widget = GanttWidget::new(&state, true);
        let mut gs = GanttState::default();
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf, &mut gs);
        assert_eq!(gs.total_items, 11);
    }

    #[test]
    fn render_bar_does_not_panic() {
        let state = sample_state();
        let widget = GanttWidget::new(&state, true);
        let mut gs = GanttState::default();
        gs.view_mode = GanttViewMode::HorizontalBar;
        let area = Rect::new(0, 0, 80, 20);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf, &mut gs);
        // 1 header + 8 tasks = 9 lines
        assert_eq!(gs.total_items, 9);
    }

    #[test]
    fn render_bar_empty_state() {
        let state = DashboardState::default();
        let widget = GanttWidget::new(&state, true);
        let mut gs = GanttState::default();
        gs.view_mode = GanttViewMode::HorizontalBar;
        let area = Rect::new(0, 0, 80, 20);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf, &mut gs);
        assert_eq!(gs.total_items, 1); // "No tasks" line
    }
}
