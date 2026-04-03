use std::io;
use std::io::Write;
use std::time::Duration;

use anyhow::Result;
use base64::Engine;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use telemetry_core::TelemetryStore;
use telemetry_runtime::SourcePlugin;
use telemetry_synthetic::SyntheticPlugin;
use telemetry_tui::{FocusPane, UiAction, UiState, selected_text};
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .without_time()
        .init();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let (tx, mut rx) = mpsc::channel(64);
    let plugin = Box::new(SyntheticPlugin::default());
    let runtime_guard = runtime.enter();
    let _plugin_handle = plugin.spawn(tx);
    drop(runtime_guard);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, event::EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = run_app(&mut terminal, &mut rx);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    rx: &mut mpsc::Receiver<telemetry_core::TelemetryUpdate>,
) -> Result<()> {
    let tick_rate = Duration::from_millis(100);
    let mut store = TelemetryStore::new();
    let mut ui = UiState::new();

    loop {
        while let Ok(update) = rx.try_recv() {
            store.apply_update(update);
        }

        let snapshot = store.snapshot();
        terminal.draw(|frame| telemetry_tui::draw(frame, &snapshot, &mut ui))?;

        if event::poll(tick_rate)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match ui.on_key(key) {
                    UiAction::Quit => break,
                    UiAction::RunCommand(command) => match command.as_str() {
                        "q" | "quit" => break,
                        _ => ui.set_status_notice(format!("unknown command: :{}", command)),
                    },
                    UiAction::YankSelectedValue => {
                        if ui.focus == FocusPane::Channels {
                            if let Some(channel) = ui.selected_channel(&snapshot) {
                                match &channel.latest {
                                    Some(sample) if !channel.is_stale => {
                                        let value = sample.value.to_string();
                                        match yank_to_terminal_clipboard(&value) {
                                            Ok(()) => ui.set_status_notice(format!(
                                                "yanked {} = {}",
                                                channel.descriptor.path, value
                                            )),
                                            Err(error) => ui.set_status_notice(format!(
                                                "yank failed: {}",
                                                error
                                            )),
                                        }
                                    }
                                    Some(_) => ui.set_status_notice(
                                        "selected channel is stale; nothing yanked",
                                    ),
                                    None => {
                                        ui.set_status_notice("selected channel has no value yet")
                                    }
                                }
                            } else if let Some(path) = ui.selected_namespace_path(&snapshot) {
                                ui.set_status_notice(format!(
                                    "namespace {} has no single live value to copy",
                                    path
                                ));
                            } else {
                                ui.set_status_notice("no channel selected");
                            }
                        } else if let Some(payload) = selected_text(&snapshot, &ui) {
                            match yank_to_terminal_clipboard(&payload.text) {
                                Ok(()) => ui.set_status_notice(format!("yanked {}", payload.label)),
                                Err(error) => {
                                    ui.set_status_notice(format!("yank failed: {}", error))
                                }
                            }
                        } else {
                            ui.set_status_notice("nothing selected to copy");
                        }
                    }
                    UiAction::ToggleSelectedNamespace => {
                        if ui.toggle_selected_namespace(&snapshot) {
                            ui.set_status_notice("toggled namespace");
                        }
                    }
                    UiAction::ToggleAllNamespaces => {
                        let (collapsed, count) = ui.toggle_all_namespaces(&snapshot);
                        if count == 0 {
                            ui.set_status_notice("no namespaces to toggle");
                        } else if collapsed {
                            ui.set_status_notice(format!("collapsed {} namespaces", count));
                        } else {
                            ui.set_status_notice(format!("expanded {} namespaces", count));
                        }
                    }
                    UiAction::None => {}
                },
                Event::Mouse(mouse) => ui.on_mouse(mouse),
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }

    Ok(())
}

fn yank_to_terminal_clipboard(text: &str) -> io::Result<()> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    let mut stdout = io::stdout();
    stdout.write_all(b"\x1b]52;c;")?;
    stdout.write_all(encoded.as_bytes())?;
    stdout.write_all(b"\x07")?;
    stdout.flush()
}
