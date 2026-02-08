use std::io;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::mpsc;

use oh_my_claude_board::app::App;
use oh_my_claude_board::data::state::DashboardState;
use oh_my_claude_board::data::watcher::{self, FileChange, WatchConfig};
use oh_my_claude_board::event::{key_to_action, poll_event, Action, AppEvent};
use oh_my_claude_board::ui::claude_output::AgentPanel;
use oh_my_claude_board::ui::detail::DetailWidget;
use oh_my_claude_board::ui::gantt::GanttWidget;
use oh_my_claude_board::ui::help::HelpOverlay;
use oh_my_claude_board::ui::layout::{DashboardLayout, FocusedPane};
use oh_my_claude_board::ui::statusbar::StatusBar;

/// Claude Code orchestration TUI dashboard
#[derive(Parser, Debug)]
#[command(name = "oh-my-claude-board", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to TASKS.md (default: ./TASKS.md, fallback: ./docs/planning/06-tasks.md)
    #[arg(long, global = true)]
    tasks: Option<String>,

    /// Path to Hook events directory
    #[arg(long, global = true)]
    hooks: Option<String>,

    /// Path to dashboard JSONL events directory (default: ~/.claude/dashboard)
    #[arg(long, global = true)]
    events: Option<String>,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Watch TASKS.md and Hook events in real-time (default)
    Watch,
    /// Initialize configuration
    Init,
}

/// Resolve the hooks directory: .claude/hooks > ~/.claude/hooks
fn resolve_hooks_path() -> PathBuf {
    let local = PathBuf::from(".claude/hooks");
    if local.is_dir() {
        return local;
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let global = PathBuf::from(home).join(".claude").join("hooks");
    if global.is_dir() {
        return global;
    }
    local
}

/// Resolve the tasks file path: explicit CLI arg > ./TASKS.md > ./docs/planning/06-tasks.md
fn resolve_tasks_path(explicit: Option<&str>) -> String {
    if let Some(path) = explicit {
        return path.to_string();
    }
    let primary = "./TASKS.md";
    if std::path::Path::new(primary).exists() {
        return primary.to_string();
    }
    let fallback = "./docs/planning/06-tasks.md";
    if std::path::Path::new(fallback).exists() {
        return fallback.to_string();
    }
    primary.to_string()
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let tasks_path = resolve_tasks_path(cli.tasks.as_deref());

    match cli.command.unwrap_or(Commands::Watch) {
        Commands::Watch => run_tui(&tasks_path, cli.hooks.as_deref(), cli.events.as_deref()),
        Commands::Init => {
            println!("oh-my-claude-board init (not yet implemented)");
            Ok(())
        }
    }
}

/// Install a panic hook that restores the terminal before printing the panic
fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));
}

fn run_tui(tasks_path: &str, hooks_dir: Option<&str>, events_dir: Option<&str>) -> Result<()> {
    // Load initial state
    let dashboard = match std::fs::read_to_string(tasks_path) {
        Ok(content) => DashboardState::from_tasks_content(&content)
            .unwrap_or_else(|_| DashboardState::default()),
        Err(_) => DashboardState::default(),
    };

    let mut dashboard = dashboard;
    let hooks_path = hooks_dir
        .map(PathBuf::from)
        .unwrap_or_else(resolve_hooks_path);

    // Resolve events directory: CLI arg > default ~/.claude/dashboard
    let events_path = events_dir.map(PathBuf::from).unwrap_or_else(|| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".claude").join("dashboard")
    });

    // Load existing hook events at startup
    if hooks_path.is_dir() {
        let _ = dashboard.load_hook_events(&hooks_path);
    }
    // Also load events from the dashboard events directory
    if events_path.is_dir() {
        let _ = dashboard.load_hook_events(&events_path);
    }

    let mut app = App::new().with_dashboard(dashboard);
    let mut watch_config = WatchConfig::new(PathBuf::from(tasks_path), hooks_path);
    if events_path.is_dir() {
        watch_config = watch_config.with_events_dir(events_path);
    }
    let watcher_rx = if watch_config.validate().is_ok() {
        match watcher::start_watching(watch_config) {
            Ok((_watcher, rx)) => {
                let watcher = _watcher;
                std::mem::forget(watcher);
                Some(rx)
            }
            Err(_) => None,
        }
    } else {
        None
    };

    // Install panic hook before entering raw mode
    install_panic_hook();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = run_loop(&mut terminal, &mut app, watcher_rx);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::cursor::MoveTo(0, 0),
        crossterm::cursor::Show
    )?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    mut watcher_rx: Option<mpsc::UnboundedReceiver<FileChange>>,
) -> Result<()> {
    let tick_rate = Duration::from_millis(250);

    while app.running {
        // Draw
        terminal.draw(|frame| {
            let area = frame.area();
            let layout = DashboardLayout::compute(area);

            // Left panel: Gantt chart
            let gantt = GanttWidget::new(&app.dashboard, app.focused == FocusedPane::TaskList);
            frame.render_stateful_widget(gantt, layout.task_list, &mut app.gantt_state);

            // Right panel: Detail view
            let selected_task = app.selected_task();
            let detail = DetailWidget::from_selection(
                &app.dashboard,
                selected_task,
                app.gantt_state.selected,
                app.focused == FocusedPane::Detail,
            );
            frame.render_widget(detail, layout.detail);

            // Right bottom: Agent activity
            let agents = AgentPanel::new(&app.dashboard);
            frame.render_widget(agents, layout.agents);

            // Bottom: Status bar
            let statusbar = StatusBar::new(&app.dashboard);
            frame.render_widget(statusbar, layout.status_bar);

            // Help overlay (on top if active)
            if app.show_help {
                frame.render_widget(HelpOverlay, area);
            }
        })?;

        // Process file watcher events (non-blocking)
        if let Some(ref mut rx) = watcher_rx {
            while let Ok(change) = rx.try_recv() {
                app.handle_file_change(&change);
            }
        }

        // Handle keyboard events
        if let Some(event) = poll_event(tick_rate)? {
            match event {
                AppEvent::Key(key) => match key_to_action(key) {
                    Action::Quit => app.quit(),
                    Action::MoveDown => app.move_down(),
                    Action::MoveUp => app.move_up(),
                    Action::ToggleFocus => app.toggle_focus(),
                    Action::ToggleHelp => app.toggle_help(),
                    Action::None => {}
                },
                AppEvent::Resize(_, _) => {} // terminal auto-handles resize
                AppEvent::FileChanged(change) => app.handle_file_change(&change),
                AppEvent::Tick => {}
            }
        }
    }

    Ok(())
}
