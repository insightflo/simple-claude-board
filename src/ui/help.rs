//! Help overlay
//!
//! Shows keybinding help as a centered popup overlay.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

/// Help overlay widget
pub struct HelpOverlay;

impl HelpOverlay {
    /// Calculate a centered rect for the help popup
    fn centered_rect(area: Rect) -> Rect {
        let width = 40.min(area.width.saturating_sub(4));
        let height = 15.min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        Rect::new(x, y, width, height)
    }

    fn help_lines() -> Vec<Line<'static>> {
        let version = env!("CARGO_PKG_VERSION");
        vec![
            Line::from(vec![Span::styled(
                format!(" oh-my-claude-board v{version} "),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::raw(""),
            Line::from(vec![Span::styled(
                " Keybindings ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::raw(""),
            Line::from(vec![
                Span::styled("  j / Down  ", Style::default().fg(Color::Yellow)),
                Span::raw("Move down"),
            ]),
            Line::from(vec![
                Span::styled("  k / Up    ", Style::default().fg(Color::Yellow)),
                Span::raw("Move up"),
            ]),
            Line::from(vec![
                Span::styled("  Tab       ", Style::default().fg(Color::Yellow)),
                Span::raw("Switch focus"),
            ]),
            Line::from(vec![
                Span::styled("  Space     ", Style::default().fg(Color::Yellow)),
                Span::raw("Collapse/expand phase"),
            ]),
            Line::from(vec![
                Span::styled("  v         ", Style::default().fg(Color::Yellow)),
                Span::raw("Switch view (Tree/Gantt)"),
            ]),
            Line::from(vec![
                Span::styled("  ?         ", Style::default().fg(Color::Yellow)),
                Span::raw("Close help"),
            ]),
            Line::from(vec![
                Span::styled("  q / Esc   ", Style::default().fg(Color::Yellow)),
                Span::raw("Quit"),
            ]),
        ]
    }
}

impl Widget for HelpOverlay {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup_area = Self::centered_rect(area);

        // Clear the area behind the popup
        Clear.render(popup_area, buf);

        let block = Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let lines = Self::help_lines();
        let paragraph = Paragraph::new(lines).block(block);
        paragraph.render(popup_area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_overlay_renders() {
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        HelpOverlay.render(area, &mut buf);
    }

    #[test]
    fn help_centered_rect() {
        let area = Rect::new(0, 0, 80, 30);
        let popup = HelpOverlay::centered_rect(area);
        assert!(popup.x > 0);
        assert!(popup.y > 0);
        assert!(popup.width <= 40);
        assert!(popup.height <= 15);
    }

    #[test]
    fn help_small_terminal() {
        let area = Rect::new(0, 0, 20, 8);
        let mut buf = Buffer::empty(area);
        HelpOverlay.render(area, &mut buf);
    }

    #[test]
    fn help_lines_not_empty() {
        let lines = HelpOverlay::help_lines();
        assert!(lines.len() >= 5);
    }
}
