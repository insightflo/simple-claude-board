//! Keyboard, file, and timer event integration
//!
//! Merges crossterm keyboard events with file-watcher events into a unified
//! event stream for the main loop.

use std::time::Duration;

use crossterm::event::{
    self, Event as CrosstermEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};

use crate::data::watcher::FileChange;

/// Unified application event
#[derive(Debug)]
pub enum AppEvent {
    /// Keyboard input
    Key(KeyEvent),
    /// File change detected
    FileChanged(FileChange),
    /// Periodic tick for UI refresh
    Tick,
    /// Terminal resize
    Resize(u16, u16),
}

/// Polls for crossterm events with a timeout.
/// Returns `Some(AppEvent)` if an event occurred, `None` on timeout.
pub fn poll_event(timeout: Duration) -> anyhow::Result<Option<AppEvent>> {
    if event::poll(timeout)? {
        match event::read()? {
            CrosstermEvent::Key(key) if key.kind == KeyEventKind::Press => {
                Ok(Some(AppEvent::Key(key)))
            }
            CrosstermEvent::Resize(w, h) => Ok(Some(AppEvent::Resize(w, h))),
            _ => Ok(None),
        }
    } else {
        Ok(None)
    }
}

/// Map a key event to an application action
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Quit,
    MoveUp,
    MoveDown,
    ToggleFocus,
    ToggleHelp,
    None,
}

/// Convert a key event into an action
/// Supports Korean IME fallback: ㅂ=q, ㅓ=j, ㅏ=k
pub fn key_to_action(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('q' | 'ㅂ') | KeyCode::Esc => Action::Quit,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::Quit,
        KeyCode::Char('j' | 'ㅓ') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k' | 'ㅏ') | KeyCode::Up => Action::MoveUp,
        KeyCode::Tab => Action::ToggleFocus,
        KeyCode::Char('?') => Action::ToggleHelp,
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState};

    fn make_key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn quit_on_q() {
        assert_eq!(
            key_to_action(make_key(KeyCode::Char('q'), KeyModifiers::NONE)),
            Action::Quit
        );
    }

    #[test]
    fn quit_on_esc() {
        assert_eq!(
            key_to_action(make_key(KeyCode::Esc, KeyModifiers::NONE)),
            Action::Quit
        );
    }

    #[test]
    fn quit_on_ctrl_c() {
        assert_eq!(
            key_to_action(make_key(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Action::Quit
        );
    }

    #[test]
    fn move_down_j() {
        assert_eq!(
            key_to_action(make_key(KeyCode::Char('j'), KeyModifiers::NONE)),
            Action::MoveDown
        );
    }

    #[test]
    fn move_down_arrow() {
        assert_eq!(
            key_to_action(make_key(KeyCode::Down, KeyModifiers::NONE)),
            Action::MoveDown
        );
    }

    #[test]
    fn move_up_k() {
        assert_eq!(
            key_to_action(make_key(KeyCode::Char('k'), KeyModifiers::NONE)),
            Action::MoveUp
        );
    }

    #[test]
    fn move_up_arrow() {
        assert_eq!(
            key_to_action(make_key(KeyCode::Up, KeyModifiers::NONE)),
            Action::MoveUp
        );
    }

    #[test]
    fn toggle_focus_tab() {
        assert_eq!(
            key_to_action(make_key(KeyCode::Tab, KeyModifiers::NONE)),
            Action::ToggleFocus
        );
    }

    #[test]
    fn toggle_help_question() {
        assert_eq!(
            key_to_action(make_key(KeyCode::Char('?'), KeyModifiers::NONE)),
            Action::ToggleHelp
        );
    }

    #[test]
    fn unmapped_key_is_none() {
        assert_eq!(
            key_to_action(make_key(KeyCode::Char('x'), KeyModifiers::NONE)),
            Action::None
        );
    }
}
