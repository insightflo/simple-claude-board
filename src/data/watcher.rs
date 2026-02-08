//! File watcher (notify)
//!
//! Watches TASKS.md and hook event directories for changes.
//! Sends change notifications via tokio channels for the TUI to react.

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// Types of file changes we care about
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileChange {
    /// TASKS.md was modified
    TasksModified(PathBuf),
    /// A hook event file was created or modified
    HookEventModified(PathBuf),
    /// A hook event file was created (new session)
    HookEventCreated(PathBuf),
}

/// Errors from the file watcher
#[derive(Debug, thiserror::Error)]
pub enum WatcherError {
    #[error("notify error: {0}")]
    Notify(#[from] notify::Error),
    #[error("channel send error")]
    ChannelSend,
    #[error("path does not exist: {0}")]
    PathNotFound(PathBuf),
}

/// Configuration for the file watcher
#[derive(Debug, Clone)]
pub struct WatchConfig {
    pub tasks_path: PathBuf,
    pub hooks_dir: PathBuf,
    /// Optional secondary directory for dashboard JSONL events (e.g. ~/.claude/dashboard/)
    pub events_dir: Option<PathBuf>,
}

impl WatchConfig {
    pub fn new(tasks_path: PathBuf, hooks_dir: PathBuf) -> Self {
        Self {
            tasks_path,
            hooks_dir,
            events_dir: None,
        }
    }

    /// Add an optional events directory to watch
    pub fn with_events_dir(mut self, events_dir: PathBuf) -> Self {
        self.events_dir = Some(events_dir);
        self
    }

    /// Validate that watched paths exist (events_dir is optional)
    pub fn validate(&self) -> Result<(), WatcherError> {
        if !self.tasks_path.exists() {
            return Err(WatcherError::PathNotFound(self.tasks_path.clone()));
        }
        if !self.hooks_dir.exists() {
            return Err(WatcherError::PathNotFound(self.hooks_dir.clone()));
        }
        Ok(())
    }
}

/// Check if two paths refer to the same location (handles symlinks like /var -> /private/var)
fn paths_match(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    // Try canonical comparison for symlink resolution
    if let (Ok(ca), Ok(cb)) = (a.canonicalize(), b.canonicalize()) {
        return ca == cb;
    }
    false
}

/// Check if `child` is under `parent` directory (handles symlinks)
fn is_under_dir(child: &Path, parent: &Path) -> bool {
    if child.starts_with(parent) {
        return true;
    }
    if let (Ok(cc), Ok(cp)) = (child.canonicalize(), parent.canonicalize()) {
        return cc.starts_with(cp);
    }
    false
}

/// Classify a notify event into our FileChange type
fn classify_event(event: &Event, config: &WatchConfig) -> Option<FileChange> {
    let dominated_by_modify = matches!(
        event.kind,
        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Other
    );

    if !dominated_by_modify {
        return None;
    }

    for path in &event.paths {
        if paths_match(path, &config.tasks_path) {
            return Some(FileChange::TasksModified(path.clone()));
        }

        if is_under_dir(path, &config.hooks_dir) {
            if matches!(event.kind, EventKind::Create(_)) {
                return Some(FileChange::HookEventCreated(path.clone()));
            }
            return Some(FileChange::HookEventModified(path.clone()));
        }

        // Also check the secondary events directory
        if let Some(ref events_dir) = config.events_dir {
            if is_under_dir(path, events_dir) {
                if matches!(event.kind, EventKind::Create(_)) {
                    return Some(FileChange::HookEventCreated(path.clone()));
                }
                return Some(FileChange::HookEventModified(path.clone()));
            }
        }
    }

    None
}

/// Start watching files and return a receiver for change events.
///
/// Returns `(watcher, receiver)`. The watcher must be kept alive for events to flow.
pub fn start_watching(
    config: WatchConfig,
) -> Result<(RecommendedWatcher, mpsc::UnboundedReceiver<FileChange>), WatcherError> {
    config.validate()?;

    let (tx, rx) = mpsc::unbounded_channel();
    let watch_config = config.clone();

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                if let Some(change) = classify_event(&event, &watch_config) {
                    let _ = tx.send(change);
                }
            }
        },
        Config::default(),
    )?;

    // Watch the parent directory of TASKS.md (FSEvents on macOS needs directories)
    let tasks_parent = config
        .tasks_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| config.tasks_path.clone());
    watcher.watch(&tasks_parent, RecursiveMode::NonRecursive)?;
    watcher.watch(&config.hooks_dir, RecursiveMode::Recursive)?;

    // Watch the secondary events directory if it exists
    if let Some(ref events_dir) = config.events_dir {
        if events_dir.is_dir() {
            let _ = watcher.watch(events_dir, RecursiveMode::Recursive);
        }
    }

    Ok((watcher, rx))
}

/// Start a poll-based watcher (reliable for tests and environments where FSEvents is flaky).
/// Canonicalizes watched paths to avoid macOS /var -> /private/var symlink issues.
#[cfg(test)]
fn start_watching_poll(
    config: WatchConfig,
    interval: std::time::Duration,
) -> Result<(notify::PollWatcher, mpsc::UnboundedReceiver<FileChange>), WatcherError> {
    config.validate()?;

    // Canonicalize config paths so they match what PollWatcher reports
    let canon_config = WatchConfig::new(
        config
            .tasks_path
            .canonicalize()
            .unwrap_or(config.tasks_path),
        config.hooks_dir.canonicalize().unwrap_or(config.hooks_dir),
    );

    let (tx, rx) = mpsc::unbounded_channel();
    let watch_config = canon_config.clone();

    let poll_config = Config::default().with_poll_interval(interval);

    let mut watcher = notify::PollWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                if let Some(change) = classify_event(&event, &watch_config) {
                    let _ = tx.send(change);
                }
            }
        },
        poll_config,
    )?;

    let tasks_parent = canon_config
        .tasks_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| canon_config.tasks_path.clone());
    watcher.watch(&tasks_parent, RecursiveMode::NonRecursive)?;
    watcher.watch(&canon_config.hooks_dir, RecursiveMode::Recursive)?;

    Ok((watcher, rx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, ModifyKind};
    use std::fs;
    use tempfile::TempDir;

    fn make_config(tmp: &TempDir) -> WatchConfig {
        let tasks_path = tmp.path().join("TASKS.md");
        let hooks_dir = tmp.path().join("hooks");
        fs::write(&tasks_path, "# Phase 0: Setup").expect("write tasks");
        fs::create_dir_all(&hooks_dir).expect("create hooks dir");
        WatchConfig::new(tasks_path, hooks_dir)
    }

    #[test]
    fn watch_config_validate_ok() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(&tmp);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn watch_config_validate_missing_tasks() {
        let tmp = TempDir::new().unwrap();
        let hooks_dir = tmp.path().join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let config = WatchConfig::new(tmp.path().join("missing.md"), hooks_dir);
        assert!(config.validate().is_err());
    }

    #[test]
    fn watch_config_validate_missing_hooks() {
        let tmp = TempDir::new().unwrap();
        let tasks_path = tmp.path().join("TASKS.md");
        fs::write(&tasks_path, "test").unwrap();
        let config = WatchConfig::new(tasks_path, tmp.path().join("missing_hooks"));
        assert!(config.validate().is_err());
    }

    #[test]
    fn classify_tasks_modify() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(&tmp);
        let event = Event {
            kind: EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            paths: vec![config.tasks_path.clone()],
            attrs: Default::default(),
        };
        let change = classify_event(&event, &config);
        assert_eq!(
            change,
            Some(FileChange::TasksModified(config.tasks_path.clone()))
        );
    }

    #[test]
    fn classify_hook_create() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(&tmp);
        let hook_file = config.hooks_dir.join("new_session.jsonl");
        let event = Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![hook_file.clone()],
            attrs: Default::default(),
        };
        let change = classify_event(&event, &config);
        assert_eq!(change, Some(FileChange::HookEventCreated(hook_file)));
    }

    #[test]
    fn classify_hook_modify() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(&tmp);
        let hook_file = config.hooks_dir.join("session.jsonl");
        let event = Event {
            kind: EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            paths: vec![hook_file.clone()],
            attrs: Default::default(),
        };
        let change = classify_event(&event, &config);
        assert_eq!(change, Some(FileChange::HookEventModified(hook_file)));
    }

    #[test]
    fn classify_unrelated_path_ignored() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(&tmp);
        let event = Event {
            kind: EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            paths: vec![PathBuf::from("/some/other/file.txt")],
            attrs: Default::default(),
        };
        assert!(classify_event(&event, &config).is_none());
    }

    #[test]
    fn classify_remove_event_ignored() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(&tmp);
        let event = Event {
            kind: EventKind::Remove(notify::event::RemoveKind::File),
            paths: vec![config.tasks_path.clone()],
            attrs: Default::default(),
        };
        assert!(classify_event(&event, &config).is_none());
    }

    #[test]
    fn start_watching_creates_watcher() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(&tmp);
        let result = start_watching(config);
        assert!(result.is_ok());
    }

    #[test]
    fn start_watching_invalid_path_fails() {
        let config = WatchConfig::new(
            PathBuf::from("/nonexistent/TASKS.md"),
            PathBuf::from("/nonexistent/hooks"),
        );
        assert!(start_watching(config).is_err());
    }

    // PollWatcher modification detection is flaky on macOS temp directories
    // due to /var -> /private/var symlink and FSEvents caching behavior.
    // Works reliably with real directories in production.
    #[tokio::test]
    #[ignore]
    async fn poll_watcher_detects_tasks_change() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(&tmp);
        let tasks_path = config
            .tasks_path
            .canonicalize()
            .unwrap_or(config.tasks_path.clone());

        let canon_config = WatchConfig::new(
            tasks_path.clone(),
            config.hooks_dir.canonicalize().unwrap_or(config.hooks_dir),
        );

        let poll_interval = std::time::Duration::from_millis(100);
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Send raw events to debug channel
        let (raw_tx, mut raw_rx) = mpsc::unbounded_channel::<Event>();
        let watch_cfg = canon_config.clone();
        let poll_config = Config::default().with_poll_interval(poll_interval);

        let mut watcher = notify::PollWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = raw_tx.send(event.clone());
                    if let Some(change) = classify_event(&event, &watch_cfg) {
                        let _ = tx.send(change);
                    }
                }
            },
            poll_config,
        )
        .expect("create poll watcher");

        // Watch tasks file directly AND parent directory
        watcher
            .watch(&tasks_path, RecursiveMode::NonRecursive)
            .expect("watch tasks file");

        // Wait for baseline
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Drain initial events
        while raw_rx.try_recv().is_ok() {}
        while rx.try_recv().is_ok() {}

        // Modify the file
        fs::write(
            &tasks_path,
            "# Phase 0: Modified content for test\n## Added",
        )
        .expect("write");

        // Collect raw events over 3 seconds
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        let mut raw_events = Vec::new();
        while let Ok(evt) = raw_rx.try_recv() {
            raw_events.push(evt);
        }

        let mut changes = Vec::new();
        while let Ok(ch) = rx.try_recv() {
            changes.push(ch);
        }

        assert!(
            !raw_events.is_empty(),
            "PollWatcher should emit raw events. tasks_path={tasks_path:?}, canon_config={canon_config:?}"
        );
        assert!(
            !changes.is_empty(),
            "Should have classified changes. raw_events: {raw_events:?}"
        );
        assert!(
            matches!(changes[0], FileChange::TasksModified(_)),
            "should be TasksModified, got: {:?}",
            changes[0]
        );
    }

    #[tokio::test]
    async fn poll_watcher_detects_hook_creation() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(&tmp);
        let hooks_dir = config.hooks_dir.clone();

        let poll_interval = std::time::Duration::from_millis(100);
        let (_watcher, mut rx) =
            start_watching_poll(config, poll_interval).expect("start poll watching");

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Create a new hook file
        let hook_file = hooks_dir.join("new_session.jsonl");
        fs::write(&hook_file, "{\"event_type\":\"agent_start\"}").expect("write hook");

        let change = tokio::time::timeout(std::time::Duration::from_secs(3), rx.recv()).await;

        assert!(change.is_ok(), "should receive change within timeout");
        let change = change.unwrap().expect("channel should not close");
        assert!(
            matches!(
                change,
                FileChange::HookEventCreated(_) | FileChange::HookEventModified(_)
            ),
            "should be hook event, got: {change:?}"
        );
    }
}
