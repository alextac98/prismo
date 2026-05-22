use std::io;
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Result, anyhow};
use base64::Engine;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use prismo_core::{RuntimeEvent, TelemetryStore};
use prismo_plugin_host::PluginHost;
use prismo_tui::{FocusPane, UiAction, UiState, selected_text};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .without_time()
        .init();

    let plugins_dir = plugins_dir_from_args(&args)?;
    let (tx, rx) = mpsc::channel();
    let host = PluginHost::start(tx, std::env::current_exe()?, plugins_dir)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, event::EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = run_app(&mut terminal, &rx);
    host.shutdown();

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn plugins_dir_from_args(args: &[String]) -> Result<Option<PathBuf>> {
    optional_arg_value(args, "--plugins").map(|value| value.map(PathBuf::from))
}

fn optional_arg_value<'a>(args: &'a [String], flag: &str) -> Result<Option<&'a str>> {
    let mut index = 0;
    while index < args.len() {
        if args[index] == flag {
            return args
                .get(index + 1)
                .map(|value| Some(value.as_str()))
                .ok_or_else(|| anyhow!("missing value for {}", flag));
        }
        index += 1;
    }

    Ok(None)
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    rx: &mpsc::Receiver<RuntimeEvent>,
) -> Result<()> {
    let tick_rate = Duration::from_millis(100);
    let mut store = TelemetryStore::new();
    let mut ui = UiState::new();

    loop {
        while let Ok(event) = rx.try_recv() {
            store.apply_event(event);
        }

        let snapshot = store.snapshot();
        terminal.draw(|frame| prismo_tui::draw(frame, &snapshot, &mut ui))?;

        if event::poll(tick_rate)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match ui.on_key(key) {
                    UiAction::Quit => break,
                    UiAction::RunCommand(command) => match command.as_str() {
                        "q" | "Q" | "quit" => break,
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
