use std::collections::HashMap;
use std::io::{self, Write};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use prismo_core::{RuntimeEvent, StoreSnapshot, TelemetryStore};
use prismo_example_rust::ExampleTelemetrySource;
use prismo_tui::{UiAction, UiState, selected_text};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::{Buffer, Cell};
use ratatui::style::{Color, Modifier};
use serde::Serialize;

const DEMO_WIDTH: u16 = 118;
const DEMO_HEIGHT: u16 = 32;
const DEMO_TICK_MS: u64 = 80;
const DEMO_SEED: u64 = 0xC0DE_5151;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DemoDocument {
    width: u16,
    height: u16,
    styles: Vec<DemoStyle>,
    frames: Vec<DemoFrame>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct DemoStyle {
    fg: Option<String>,
    bg: Option<String>,
    bold: bool,
    dim: bool,
    italic: bool,
    underlined: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DemoFrame {
    duration_ms: u64,
    lines: Vec<Vec<DemoSegment>>,
}

#[derive(Serialize)]
struct DemoSegment {
    text: String,
    style: usize,
}

struct StylePalette {
    styles: Vec<DemoStyle>,
    indices: HashMap<DemoStyle, usize>,
}

impl StylePalette {
    fn new() -> Self {
        Self {
            styles: Vec::new(),
            indices: HashMap::new(),
        }
    }

    fn index_for(&mut self, cell: &Cell) -> usize {
        let style = DemoStyle {
            fg: color_to_css(cell.fg),
            bg: color_to_css(cell.bg),
            bold: cell.modifier.contains(Modifier::BOLD),
            dim: cell.modifier.contains(Modifier::DIM),
            italic: cell.modifier.contains(Modifier::ITALIC),
            underlined: cell.modifier.contains(Modifier::UNDERLINED),
        };

        if let Some(index) = self.indices.get(&style) {
            *index
        } else {
            let index = self.styles.len();
            self.styles.push(style.clone());
            self.indices.insert(style, index);
            index
        }
    }
}

struct DemoRunner {
    source: ExampleTelemetrySource,
    store: TelemetryStore,
    ui: UiState,
    terminal: Terminal<TestBackend>,
    palette: StylePalette,
    frames: Vec<DemoFrame>,
    base_instant: Instant,
    elapsed: Duration,
    descriptors_sent: bool,
}

impl DemoRunner {
    fn new() -> Result<Self> {
        let mut store = TelemetryStore::new();
        let source =
            ExampleTelemetrySource::from_seed(Duration::from_millis(DEMO_TICK_MS), DEMO_SEED);
        store.apply_event(RuntimeEvent::PluginStatus(source.initial_status()));

        let mut terminal = Terminal::new(TestBackend::new(DEMO_WIDTH, DEMO_HEIGHT))?;
        terminal.clear()?;

        Ok(Self {
            source,
            store,
            ui: UiState::new(),
            terminal,
            palette: StylePalette::new(),
            frames: Vec::new(),
            base_instant: Instant::now(),
            elapsed: Duration::ZERO,
            descriptors_sent: false,
        })
    }

    fn build(mut self) -> Result<DemoDocument> {
        self.advance_ticks(12);
        self.capture(1200)?;

        self.press_key(KeyCode::Char('/'));
        self.capture(650)?;

        for ch in "battery".chars() {
            self.press_key(KeyCode::Char(ch));
        }
        self.capture(800)?;

        self.press_key(KeyCode::Enter);
        self.capture(900)?;

        for _ in 0..3 {
            self.press_key(KeyCode::Char('j'));
        }
        self.capture(850)?;

        self.press_key(KeyCode::Tab);
        self.capture(700)?;

        self.press_key(KeyCode::Tab);
        self.capture(850)?;

        for duration_ms in [450, 450, 450, 650] {
            self.advance_ticks(3);
            self.capture(duration_ms)?;
        }

        self.press_key(KeyCode::Char('?'));
        self.capture(1200)?;
        self.press_key(KeyCode::Char('?'));

        self.press_key(KeyCode::Char('/'));
        self.press_key(KeyCode::Esc);
        self.capture(800)?;

        self.press_key(KeyCode::Char('/'));
        for ch in "secondary".chars() {
            self.press_key(KeyCode::Char(ch));
        }
        self.capture(850)?;

        self.press_key(KeyCode::Enter);
        self.advance_ticks(23);
        for _ in 0..4 {
            self.press_key(KeyCode::Char('j'));
        }
        self.capture(1300)?;

        Ok(DemoDocument {
            width: DEMO_WIDTH,
            height: DEMO_HEIGHT,
            styles: self.palette.styles,
            frames: self.frames,
        })
    }

    fn advance_ticks(&mut self, count: usize) {
        for _ in 0..count {
            self.elapsed += self.source.period();
            let observed_at = self.base_instant + self.elapsed;
            let mut update = self.source.next_update(observed_at);
            if !self.descriptors_sent {
                update.descriptors = self.source.descriptors();
                self.descriptors_sent = true;
            }
            self.store.apply_event(RuntimeEvent::Telemetry(update));
        }
    }

    fn snapshot(&self) -> StoreSnapshot {
        self.store.snapshot_at(self.base_instant + self.elapsed)
    }

    fn press_key(&mut self, code: KeyCode) {
        let snapshot = self.snapshot();
        let action = self.ui.on_key(KeyEvent::new(code, KeyModifiers::NONE));
        self.apply_action(action, &snapshot);
    }

    fn apply_action(&mut self, action: UiAction, snapshot: &StoreSnapshot) {
        match action {
            UiAction::None => {}
            UiAction::Quit => {
                self.ui.set_status_notice("quit ignored in homepage demo");
            }
            UiAction::RunCommand(command) => match command.as_str() {
                "q" | "quit" => self.ui.set_status_notice("quit ignored in homepage demo"),
                _ => self
                    .ui
                    .set_status_notice(format!("unknown command: :{command}")),
            },
            UiAction::YankSelectedValue => {
                if let Some(payload) = selected_text(snapshot, &self.ui) {
                    self.ui
                        .set_status_notice(format!("copy disabled in demo: {}", payload.label));
                } else {
                    self.ui.set_status_notice("nothing selected to copy");
                }
            }
            UiAction::ToggleSelectedNamespace => {
                if self.ui.toggle_selected_namespace(snapshot) {
                    self.ui.set_status_notice("toggled namespace");
                }
            }
            UiAction::ToggleAllNamespaces => {
                let (collapsed, count) = self.ui.toggle_all_namespaces(snapshot);
                if count == 0 {
                    self.ui.set_status_notice("no namespaces to toggle");
                } else if collapsed {
                    self.ui
                        .set_status_notice(format!("collapsed {count} namespaces"));
                } else {
                    self.ui
                        .set_status_notice(format!("expanded {count} namespaces"));
                }
            }
        }
    }

    fn capture(&mut self, duration_ms: u64) -> Result<()> {
        let snapshot = self.snapshot();
        self.terminal
            .draw(|frame| prismo_tui::draw(frame, &snapshot, &mut self.ui))?;
        let lines = export_lines(self.terminal.backend().buffer(), &mut self.palette);
        self.frames.push(DemoFrame { duration_ms, lines });
        Ok(())
    }
}

fn export_lines(buffer: &Buffer, palette: &mut StylePalette) -> Vec<Vec<DemoSegment>> {
    (0..DEMO_HEIGHT)
        .map(|y| {
            let mut line = Vec::new();
            let mut current_style = None;
            let mut current_text = String::new();

            for x in 0..DEMO_WIDTH {
                let cell = &buffer[(x, y)];
                let style = palette.index_for(cell);
                match current_style {
                    Some(active_style) if active_style == style => {
                        current_text.push_str(cell.symbol());
                    }
                    Some(active_style) => {
                        line.push(DemoSegment {
                            text: std::mem::take(&mut current_text),
                            style: active_style,
                        });
                        current_text.push_str(cell.symbol());
                        current_style = Some(style);
                    }
                    None => {
                        current_text.push_str(cell.symbol());
                        current_style = Some(style);
                    }
                }
            }

            if let Some(style) = current_style {
                line.push(DemoSegment {
                    text: current_text,
                    style,
                });
            }

            line
        })
        .collect()
}

fn color_to_css(color: Color) -> Option<String> {
    match color {
        Color::Reset => None,
        Color::Black => Some("#0b0d10".to_string()),
        Color::Red => Some("#d35656".to_string()),
        Color::Green => Some("#9bc379".to_string()),
        Color::Yellow => Some("#e2c26d".to_string()),
        Color::Blue => Some("#6fa4ff".to_string()),
        Color::Magenta => Some("#c586c0".to_string()),
        Color::Cyan => Some("#61c4c8".to_string()),
        Color::Gray => Some("#a9b3bc".to_string()),
        Color::DarkGray => Some("#5f6973".to_string()),
        Color::LightRed => Some("#ff7a7a".to_string()),
        Color::LightGreen => Some("#b6e07f".to_string()),
        Color::LightYellow => Some("#f4d97a".to_string()),
        Color::LightBlue => Some("#8fb7ff".to_string()),
        Color::LightMagenta => Some("#e5a7e0".to_string()),
        Color::LightCyan => Some("#9fe7e9".to_string()),
        Color::White => Some("#f6fbff".to_string()),
        Color::Rgb(r, g, b) => Some(format!("#{r:02x}{g:02x}{b:02x}")),
        Color::Indexed(index) => Some(indexed_color(index)),
    }
}

fn indexed_color(index: u8) -> String {
    if index < 16 {
        return match index {
            0 => "#0b0d10".to_string(),
            1 => "#d35656".to_string(),
            2 => "#9bc379".to_string(),
            3 => "#e2c26d".to_string(),
            4 => "#6fa4ff".to_string(),
            5 => "#c586c0".to_string(),
            6 => "#61c4c8".to_string(),
            7 => "#a9b3bc".to_string(),
            8 => "#5f6973".to_string(),
            9 => "#ff7a7a".to_string(),
            10 => "#b6e07f".to_string(),
            11 => "#f4d97a".to_string(),
            12 => "#8fb7ff".to_string(),
            13 => "#e5a7e0".to_string(),
            14 => "#9fe7e9".to_string(),
            _ => "#f6fbff".to_string(),
        };
    }

    if index >= 232 {
        let level = 8 + (index - 232) * 10;
        return format!("#{level:02x}{level:02x}{level:02x}");
    }

    let index = index - 16;
    let red = index / 36;
    let green = (index % 36) / 6;
    let blue = index % 6;
    let channel = |value: u8| if value == 0 { 0 } else { value * 40 + 55 };
    format!(
        "#{:02x}{:02x}{:02x}",
        channel(red),
        channel(green),
        channel(blue)
    )
}

fn main() -> Result<()> {
    let demo = DemoRunner::new()?.build()?;
    let json = serde_json::to_string_pretty(&demo)?;
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    writeln!(
        stdout,
        "/* eslint-disable */\nconst homepageDemo = {json};\n\nexport default homepageDemo;"
    )?;
    stdout.flush()?;
    Ok(())
}
