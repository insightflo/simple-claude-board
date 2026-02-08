//! Retry confirmation modal
//!
//! Shows a centered popup asking the user to confirm retrying a failed task.
//! Follows the same pattern as `HelpOverlay`.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

/// Retry confirmation modal widget
pub struct RetryModal {
    pub task_id: String,
    pub task_name: String,
    pub retryable: bool,
}

impl RetryModal {
    fn centered_rect(area: Rect) -> Rect {
        let width = 36.min(area.width.saturating_sub(4));
        let height = 10.min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        Rect::new(x, y, width, height)
    }

    fn build_lines(&self) -> Vec<Line<'static>> {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("  Task: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    self.task_id.clone(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("  Name: ", Style::default().fg(Color::DarkGray)),
                Span::raw(self.task_name.clone()),
            ]),
            Line::raw(""),
        ];

        if self.retryable {
            lines.push(Line::styled(
                "  Retry this task?",
                Style::default().fg(Color::Yellow),
            ));
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::styled("  [y]", Style::default().fg(Color::Green)),
                Span::raw(" Yes  "),
                Span::styled("[n]", Style::default().fg(Color::Red)),
                Span::raw(" No"),
            ]));
        } else {
            lines.push(Line::styled(
                "  Not retryable",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ));
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                "  Press any key to close",
                Style::default().fg(Color::DarkGray),
            ));
        }

        lines
    }
}

impl Widget for RetryModal {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup_area = Self::centered_rect(area);
        Clear.render(popup_area, buf);

        let block = Block::default()
            .title(" Retry ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));

        let lines = self.build_lines();
        let paragraph = Paragraph::new(lines).block(block);
        paragraph.render(popup_area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_modal_retryable_renders() {
        let modal = RetryModal {
            task_id: "P1-R3-T1".to_string(),
            task_name: "File watcher".to_string(),
            retryable: true,
        };
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        modal.render(area, &mut buf);
    }

    #[test]
    fn retry_modal_not_retryable_renders() {
        let modal = RetryModal {
            task_id: "P1-R3-T1".to_string(),
            task_name: "File watcher".to_string(),
            retryable: false,
        };
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        modal.render(area, &mut buf);
    }

    #[test]
    fn retry_modal_small_terminal() {
        let modal = RetryModal {
            task_id: "T1".to_string(),
            task_name: "Test".to_string(),
            retryable: true,
        };
        let area = Rect::new(0, 0, 20, 8);
        let mut buf = Buffer::empty(area);
        modal.render(area, &mut buf);
    }

    #[test]
    fn retryable_lines_contain_yes_no() {
        let modal = RetryModal {
            task_id: "T1".to_string(),
            task_name: "Test".to_string(),
            retryable: true,
        };
        let lines = modal.build_lines();
        let has_yes = lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("Yes")));
        assert!(has_yes);
    }

    #[test]
    fn not_retryable_lines_show_warning() {
        let modal = RetryModal {
            task_id: "T1".to_string(),
            task_name: "Test".to_string(),
            retryable: false,
        };
        let lines = modal.build_lines();
        let has_warning = lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("Not retryable")));
        assert!(has_warning);
    }
}
