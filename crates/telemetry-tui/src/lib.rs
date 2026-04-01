use std::cmp;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::prelude::{Alignment, Buffer, Color, Line, Modifier, Style, Widget};
use ratatui::symbols;
use ratatui::text::Span;
use ratatui::widgets::{
    Axis, Block, Borders, Chart, Clear, Dataset, GraphType, List, ListItem, ListState, Paragraph,
    Wrap,
};
use telemetry_core::{ChannelSnapshot, ChannelValue, StoreSnapshot};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusPane {
    Details,
    LatestValue,
    Channels,
}

#[derive(Debug)]
pub enum UiAction {
    None,
    Quit,
    YankSelectedValue,
}

#[derive(Clone, Debug)]
pub struct StatusNotice {
    text: String,
    expires_at: Instant,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
struct TextPoint {
    line: usize,
    column: usize,
}

#[derive(Clone, Debug, Default)]
struct TextCursor {
    point: TextPoint,
}

impl TextCursor {
    fn move_left(&mut self) {
        self.point.column = self.point.column.saturating_sub(1);
    }

    fn move_right(&mut self) {
        self.point.column = self.point.column.saturating_add(1);
    }

    fn move_up(&mut self) {
        self.point.line = self.point.line.saturating_sub(1);
    }

    fn move_down(&mut self) {
        self.point.line = self.point.line.saturating_add(1);
    }
}

pub struct UiState {
    pub selected: usize,
    pub focus: FocusPane,
    pub filter_mode: bool,
    pub help_mode: bool,
    pub filter_input: String,
    pub status_notice: Option<StatusNotice>,
    channel_area: Option<Rect>,
    rendered_channels: usize,
    details_cursor: TextCursor,
    latest_cursor: TextCursor,
}

pub struct CopyPayload {
    pub text: String,
    pub label: String,
}

enum LatestPaneContent {
    Text(Vec<String>),
    Numeric {
        summary: Vec<String>,
        points: Vec<(f64, f64)>,
        min_y: f64,
        max_y: f64,
    },
}

impl Default for FocusPane {
    fn default() -> Self {
        Self::Channels
    }
}

impl UiState {
    pub fn new() -> Self {
        Self {
            selected: 0,
            focus: FocusPane::Channels,
            filter_mode: false,
            help_mode: false,
            filter_input: String::new(),
            status_notice: None,
            channel_area: None,
            rendered_channels: 0,
            details_cursor: TextCursor::default(),
            latest_cursor: TextCursor::default(),
        }
    }

    pub fn visible_channels<'a>(&self, snapshot: &'a StoreSnapshot) -> Vec<&'a ChannelSnapshot> {
        let needle = self.filter_input.trim().to_ascii_lowercase();
        snapshot
            .channels
            .iter()
            .filter(|channel| {
                if needle.is_empty() {
                    true
                } else {
                    channel
                        .descriptor
                        .path
                        .to_ascii_lowercase()
                        .contains(&needle)
                        || channel
                            .descriptor
                            .description
                            .to_ascii_lowercase()
                            .contains(&needle)
                }
            })
            .collect()
    }

    pub fn selected_channel<'a>(&self, snapshot: &'a StoreSnapshot) -> Option<&'a ChannelSnapshot> {
        let channels = self.visible_channels(snapshot);
        channels.get(self.selected).copied()
    }

    pub fn clamp_selection(&mut self, total: usize) {
        self.rendered_channels = total;
        if total == 0 {
            self.selected = 0;
        } else {
            self.selected = cmp::min(self.selected, total - 1);
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) -> UiAction {
        if self.help_mode {
            match key.code {
                KeyCode::Char('q') => return UiAction::Quit,
                KeyCode::Esc | KeyCode::Char('?') => {
                    self.help_mode = false;
                }
                _ => {}
            }
            return UiAction::None;
        }

        if self.filter_mode {
            match key.code {
                KeyCode::Esc => self.filter_mode = false,
                KeyCode::Enter => self.filter_mode = false,
                KeyCode::Backspace => {
                    self.filter_input.pop();
                    self.selected = 0;
                }
                KeyCode::Char(c) => {
                    self.filter_input.push(c);
                    self.selected = 0;
                }
                _ => {}
            }
            return UiAction::None;
        }

        match key.code {
            KeyCode::Char('q') => UiAction::Quit,
            KeyCode::Char('y') => UiAction::YankSelectedValue,
            KeyCode::Char('j') => {
                if self.focus == FocusPane::Channels {
                    self.move_channel_selection(1);
                } else if let Some(cursor) = self.focused_text_cursor_mut() {
                    cursor.move_down();
                }
                UiAction::None
            }
            KeyCode::Char('k') => {
                if self.focus == FocusPane::Channels {
                    self.selected = self.selected.saturating_sub(1);
                } else if let Some(cursor) = self.focused_text_cursor_mut() {
                    cursor.move_up();
                }
                UiAction::None
            }
            KeyCode::Down => {
                if self.focus == FocusPane::Channels {
                    self.move_channel_selection(1);
                } else if let Some(cursor) = self.focused_text_cursor_mut() {
                    cursor.move_down();
                }
                UiAction::None
            }
            KeyCode::Up => {
                if self.focus == FocusPane::Channels {
                    self.selected = self.selected.saturating_sub(1);
                } else if let Some(cursor) = self.focused_text_cursor_mut() {
                    cursor.move_up();
                }
                UiAction::None
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if let Some(cursor) = self.focused_text_cursor_mut() {
                    cursor.move_left();
                }
                UiAction::None
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if let Some(cursor) = self.focused_text_cursor_mut() {
                    cursor.move_right();
                }
                UiAction::None
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.selected = 0;
                UiAction::None
            }
            KeyCode::Char('G') | KeyCode::End => {
                if self.rendered_channels > 0 {
                    self.selected = self.rendered_channels - 1;
                }
                UiAction::None
            }
            KeyCode::Char('/') => {
                self.filter_mode = true;
                UiAction::None
            }
            KeyCode::Char('?') => {
                self.help_mode = true;
                UiAction::None
            }
            KeyCode::Tab => {
                self.focus = match self.focus {
                    FocusPane::Details => FocusPane::LatestValue,
                    FocusPane::LatestValue => FocusPane::Channels,
                    FocusPane::Channels => FocusPane::Details,
                };
                UiAction::None
            }
            _ => UiAction::None,
        }
    }

    pub fn set_status_notice(&mut self, text: impl Into<String>) {
        self.status_notice = Some(StatusNotice {
            text: text.into(),
            expires_at: Instant::now() + Duration::from_secs(3),
        });
    }

    pub fn status_notice(&self) -> Option<&str> {
        self.status_notice.as_ref().and_then(|notice| {
            if Instant::now() <= notice.expires_at {
                Some(notice.text.as_str())
            } else {
                None
            }
        })
    }

    pub fn focus_label(&self) -> &'static str {
        match self.focus {
            FocusPane::Details => "details",
            FocusPane::LatestValue => "latest",
            FocusPane::Channels => "channels",
        }
    }

    pub fn on_mouse(&mut self, event: MouseEvent) {
        if !matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
            return;
        }

        let Some(area) = self.channel_area else {
            return;
        };

        let column = event.column;
        let row = event.row;
        if column < area.x
            || column >= area.x + area.width
            || row < area.y
            || row >= area.y + area.height
        {
            return;
        }

        let list_row = row.saturating_sub(area.y + 1);
        let index = usize::from(list_row);
        if index < self.rendered_channels {
            self.selected = index;
            self.focus = FocusPane::Channels;
        }
    }

    fn focused_text_cursor_mut(&mut self) -> Option<&mut TextCursor> {
        match self.focus {
            FocusPane::Details => Some(&mut self.details_cursor),
            FocusPane::LatestValue => Some(&mut self.latest_cursor),
            FocusPane::Channels => None,
        }
    }

    fn move_channel_selection(&mut self, amount: usize) {
        if self.rendered_channels > 0 {
            self.selected = cmp::min(self.selected + amount, self.rendered_channels - 1);
        }
    }
}

pub fn selected_text(snapshot: &StoreSnapshot, ui: &UiState) -> Option<CopyPayload> {
    let channel = ui.selected_channel(snapshot)?;
    match ui.focus {
        FocusPane::Details => {
            let lines = build_detail_lines(channel, ui.filter_mode, &ui.filter_input);
            extract_copy_from_lines(&lines, &ui.details_cursor).map(|text| CopyPayload {
                text,
                label: "details selection".to_string(),
            })
        }
        FocusPane::LatestValue => {
            let content = build_latest_pane_content(channel);
            let lines = latest_selectable_lines(&content);
            extract_copy_from_lines(&lines, &ui.latest_cursor).map(|text| CopyPayload {
                text,
                label: "latest selection".to_string(),
            })
        }
        FocusPane::Channels => None,
    }
}

pub fn draw(frame: &mut ratatui::Frame<'_>, snapshot: &StoreSnapshot, ui: &mut UiState) {
    let vertical = frame.area().width < 110;
    let (detail_area, channel_area, status_area) = if vertical {
        let root = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(60),
                Constraint::Percentage(40),
                Constraint::Length(1),
            ])
            .split(frame.area());
        (root[0], root[1], root[2])
    } else {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(frame.area());
        (
            columns[0],
            columns[1],
            Rect::new(
                frame.area().x,
                frame.area().bottom().saturating_sub(1),
                frame.area().width,
                1,
            ),
        )
    };

    let channels = ui.visible_channels(snapshot);
    ui.clamp_selection(channels.len());
    ui.channel_area = Some(channel_area);

    let mut list_state = ListState::default().with_selected(Some(ui.selected));
    let items = channels
        .iter()
        .map(|channel| {
            let marker = if channel.is_stale { "stale" } else { "live" };
            let value = channel
                .latest
                .as_ref()
                .map(|sample| sample.value.short_display())
                .unwrap_or_else(|| "waiting".to_string());
            ListItem::new(Line::from(vec![
                Span::styled(
                    channel.descriptor.path.clone(),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("[{marker}]"),
                    Style::default().fg(if channel.is_stale {
                        Color::Yellow
                    } else {
                        Color::Green
                    }),
                ),
                Span::raw(" "),
                Span::styled(value, Style::default().fg(Color::Gray)),
            ]))
        })
        .collect::<Vec<_>>();

    let list_block = Block::default()
        .title(if ui.filter_mode {
            "Channels / filter"
        } else {
            "Channels"
        })
        .borders(Borders::ALL)
        .border_style(focus_style(ui.focus == FocusPane::Channels));
    let channel_list = List::new(items)
        .block(list_block)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    frame.render_stateful_widget(channel_list, channel_area, &mut list_state);

    let text_cursor = if let Some(selected) = channels.get(ui.selected).copied() {
        render_detail(frame, detail_area, selected, ui)
    } else {
        let empty = Paragraph::new("No channels match the current filter.")
            .block(
                Block::default()
                    .title("Channel Detail")
                    .borders(Borders::ALL),
            )
            .alignment(Alignment::Center);
        frame.render_widget(empty, detail_area);
        None
    };

    if let Some(cursor) = text_cursor {
        frame.set_cursor_position(cursor);
    }

    let plugin_summary = snapshot
        .plugins
        .first()
        .map(|plugin| {
            format!(
                "{} updates:{} dropped:{}",
                plugin.plugin_id, plugin.health.emitted_updates, plugin.health.dropped_updates
            )
        })
        .unwrap_or_else(|| "plugin: connecting".to_string());

    let status_left = if let Some(notice) = ui.status_notice() {
        format!("q quit  ? help  focus:{}  {}", ui.focus_label(), notice)
    } else {
        format!("q quit  ? help  focus:{}", ui.focus_label())
    };
    let status_right = format!(
        "total:{} dropped:{}  {}",
        snapshot.total_updates, snapshot.dropped_updates, plugin_summary
    );
    let right_width = status_right.len().min(status_area.width as usize) as u16;
    let status_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(right_width)])
        .split(status_area);

    let status_style = Style::default().fg(Color::Black).bg(Color::Gray);
    let left = Paragraph::new(status_left).style(status_style);
    let right = Paragraph::new(status_right)
        .style(status_style)
        .alignment(Alignment::Right);
    frame.render_widget(left, status_chunks[0]);
    frame.render_widget(right, status_chunks[1]);

    if ui.filter_mode {
        let popup = centered_rect(60, 3, frame.area());
        frame.render_widget(Clear, popup);
        let filter = Paragraph::new(ui.filter_input.as_str())
            .block(Block::default().title("Filter").borders(Borders::ALL))
            .wrap(Wrap { trim: false });
        frame.render_widget(filter, popup);
    }

    if ui.help_mode {
        let popup = centered_rect(16, 74, frame.area());
        frame.render_widget(Clear, popup);
        let help_lines = vec![
            Line::from("Navigation"),
            Line::from("Tab cycle focus between Details, Latest Value, and Channels"),
            Line::from(
                "j/k move channel selection in Channels, or move cursor up/down in focused text panes",
            ),
            Line::from("h/l or Left/Right move cursor horizontally in focused text panes"),
            Line::from("g/G jump to the first or last channel"),
            Line::from(""),
            Line::from("Actions"),
            Line::from(
                "y copy the current line in Details/Latest Value, or copy the live value in Channels",
            ),
            Line::from("/ open channel filter"),
            Line::from("Mouse click select a channel in the Channels pane"),
            Line::from("Esc close filter or help"),
            Line::from("? toggle this help"),
            Line::from("q quit prismo"),
        ];
        let help = Paragraph::new(help_lines)
            .block(Block::default().title("Help").borders(Borders::ALL))
            .wrap(Wrap { trim: false });
        frame.render_widget(help, popup);
    }
}

fn render_detail(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    channel: &ChannelSnapshot,
    ui: &mut UiState,
) -> Option<Position> {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(5)])
        .split(area);

    let detail_lines = build_detail_lines(channel, ui.filter_mode, &ui.filter_input);
    let detail_block = Block::default()
        .title("Details")
        .borders(Borders::ALL)
        .border_style(focus_style(
            !ui.filter_mode && ui.focus == FocusPane::Details,
        ));
    let detail_inner = detail_block.inner(sections[0]);
    frame.render_widget(detail_block, sections[0]);
    let detail_cursor = render_selectable_text(
        frame,
        detail_inner,
        &detail_lines,
        &mut ui.details_cursor,
        !ui.filter_mode && ui.focus == FocusPane::Details,
    );

    let latest_content = build_latest_pane_content(channel);
    let latest_block = Block::default()
        .title("Latest Value")
        .borders(Borders::ALL)
        .border_style(focus_style(ui.focus == FocusPane::LatestValue));
    let latest_inner = latest_block.inner(sections[1]);
    frame.render_widget(latest_block, sections[1]);
    let latest_cursor = render_latest_value(
        frame,
        latest_inner,
        &latest_content,
        &mut ui.latest_cursor,
        ui.focus == FocusPane::LatestValue,
    );

    match ui.focus {
        FocusPane::Details if !ui.filter_mode => detail_cursor,
        FocusPane::LatestValue => latest_cursor,
        _ => None,
    }
}

fn render_latest_value(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    content: &LatestPaneContent,
    cursor: &mut TextCursor,
    focused: bool,
) -> Option<Position> {
    match content {
        LatestPaneContent::Text(lines) => {
            render_selectable_text(frame, area, lines, cursor, focused)
        }
        LatestPaneContent::Numeric {
            summary,
            points,
            min_y,
            max_y,
        } => {
            let summary_height = cmp::min(area.height, summary.len() as u16 + 1);
            let sections = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(summary_height), Constraint::Min(0)])
                .split(area);
            let cursor_position =
                render_selectable_text(frame, sections[0], summary, cursor, focused);

            if sections[1].height > 0 {
                let max_x = (points.len().saturating_sub(1) as f64).max(1.0);
                let dataset = Dataset::default()
                    .graph_type(GraphType::Line)
                    .marker(symbols::Marker::Braille)
                    .style(Style::default().fg(Color::LightCyan))
                    .data(points);
                let chart = Chart::new(vec![dataset])
                    .x_axis(
                        Axis::default()
                            .bounds([0.0, max_x])
                            .labels([Line::from("old"), Line::from("now")]),
                    )
                    .y_axis(Axis::default().bounds([*min_y, *max_y]).labels([
                        Line::from(format!("{min_y:.1}")),
                        Line::from(format!("{max_y:.1}")),
                    ]))
                    .style(Style::default().fg(Color::Gray));
                frame.render_widget(chart, sections[1]);
            }

            cursor_position
        }
    }
}

fn render_selectable_text(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    lines: &[String],
    cursor: &mut TextCursor,
    focused: bool,
) -> Option<Position> {
    if area.width == 0 || area.height == 0 {
        return None;
    }

    clamp_cursor(cursor, lines);
    let rendered = build_rendered_lines(lines, cursor, focused);
    frame.render_widget(Paragraph::new(rendered).wrap(Wrap { trim: false }), area);

    if !focused {
        return None;
    }

    let line = cmp::min(cursor.point.line as u16, area.height.saturating_sub(1));
    let col = cmp::min(cursor.point.column as u16, area.width.saturating_sub(1));
    Some(Position::new(area.x + col, area.y + line))
}

fn build_rendered_lines(
    lines: &[String],
    cursor: &TextCursor,
    focused: bool,
) -> Vec<Line<'static>> {
    let cursor = clamped_cursor(cursor, lines);
    lines
        .iter()
        .enumerate()
        .map(|(line_idx, line)| {
            let chars = line.chars().collect::<Vec<_>>();
            let mut spans = Vec::new();

            if chars.is_empty() {
                let style = if focused && cursor.point.line == line_idx && cursor.point.column == 0
                {
                    Some(cursor_style())
                } else {
                    None
                };

                if let Some(style) = style {
                    spans.push(Span::styled(" ".to_string(), style));
                }
                return Line::from(spans);
            }

            for (col_idx, ch) in chars.iter().enumerate() {
                let is_cursor =
                    focused && cursor.point.line == line_idx && cursor.point.column == col_idx;
                let style = if is_cursor {
                    cursor_style()
                } else {
                    Style::default()
                };
                spans.push(Span::styled(ch.to_string(), style));
            }

            if focused && cursor.point.line == line_idx && cursor.point.column == chars.len() {
                spans.push(Span::styled(" ".to_string(), cursor_style()));
            }

            Line::from(spans)
        })
        .collect()
}

fn build_detail_lines(
    channel: &ChannelSnapshot,
    filter_mode: bool,
    filter_input: &str,
) -> Vec<String> {
    let latest = channel.latest.as_ref();
    let latest_value = latest
        .map(|sample| sample.value.to_string())
        .unwrap_or_else(|| "waiting for data".to_string());
    let age = latest
        .map(|sample| format_duration(sample.timestamp.elapsed()))
        .unwrap_or_else(|| "n/a".to_string());

    vec![
        format!("Path: {}", channel.descriptor.path),
        format!("Value: {latest_value}"),
        format!(
            "Unit: {}    Freshness: {}",
            channel
                .descriptor
                .unit
                .clone()
                .unwrap_or_else(|| "-".to_string()),
            if channel.is_stale { "stale" } else { "live" }
        ),
        format!("Age: {age}    Updates: {}", channel.update_count),
        format!("Notes: {}", channel.descriptor.description),
        format!(
            "Filter: {}",
            if filter_mode && !filter_input.is_empty() {
                filter_input.to_string()
            } else {
                "off".to_string()
            }
        ),
    ]
}

fn build_latest_pane_content(channel: &ChannelSnapshot) -> LatestPaneContent {
    let latest = channel.latest.as_ref();
    let age = latest
        .map(|sample| format_duration(sample.timestamp.elapsed()))
        .unwrap_or_else(|| "n/a".to_string());

    match latest.map(|sample| &sample.value) {
        Some(ChannelValue::Bytes(bytes)) => {
            let ascii = bytes
                .iter()
                .map(|byte| {
                    if byte.is_ascii_graphic() {
                        char::from(*byte)
                    } else {
                        '.'
                    }
                })
                .collect::<String>();
            LatestPaneContent::Text(vec![
                format!(
                    "HEX: {}",
                    bytes
                        .iter()
                        .map(|byte| format!("{byte:02X}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                ),
                format!("ASCII: {ascii}"),
            ])
        }
        Some(ChannelValue::Text(_))
        | Some(ChannelValue::Bool(_))
        | Some(ChannelValue::Integer(_)) => LatestPaneContent::Text(vec![
            format!(
                "Value: {}",
                latest
                    .map(|sample| sample.value.to_string())
                    .unwrap_or_default()
            ),
            format!("Age: {age}"),
        ]),
        Some(ChannelValue::Float(_)) if !channel.history.is_empty() => {
            let points = channel
                .history
                .iter()
                .enumerate()
                .map(|(index, value)| (index as f64, *value))
                .collect::<Vec<_>>();
            let (min_y, max_y) = history_bounds(&channel.history);
            LatestPaneContent::Numeric {
                summary: vec![
                    format!(
                        "Value: {}",
                        latest
                            .map(|sample| sample.value.to_string())
                            .unwrap_or_default()
                    ),
                    format!("Age: {age}"),
                    format!("Samples: {}", channel.history.len()),
                ],
                points,
                min_y,
                max_y,
            }
        }
        _ => LatestPaneContent::Text(vec!["No detailed renderer for this value yet.".to_string()]),
    }
}

fn latest_selectable_lines(content: &LatestPaneContent) -> Vec<String> {
    match content {
        LatestPaneContent::Text(lines) => lines.clone(),
        LatestPaneContent::Numeric { summary, .. } => summary.clone(),
    }
}

fn extract_copy_from_lines(lines: &[String], cursor: &TextCursor) -> Option<String> {
    if lines.is_empty() {
        return None;
    }

    let cursor = clamped_cursor(cursor, lines);
    Some(lines[cursor.point.line].clone())
}

fn clamp_cursor(cursor: &mut TextCursor, lines: &[String]) {
    *cursor = clamped_cursor(cursor, lines);
}

fn clamped_cursor(cursor: &TextCursor, lines: &[String]) -> TextCursor {
    if lines.is_empty() {
        return TextCursor::default();
    }

    let mut clamped = cursor.clone();
    clamp_point(&mut clamped.point, lines);
    clamped
}

fn clamp_point(point: &mut TextPoint, lines: &[String]) {
    let max_line = lines.len().saturating_sub(1);
    point.line = point.line.min(max_line);
    let max_col = lines[point.line].chars().count();
    point.column = point.column.min(max_col);
}

fn centered_rect(height: u16, width: u16, area: Rect) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

fn format_duration(duration: Duration) -> String {
    if duration.as_secs() > 0 {
        format!("{}.{:03}s", duration.as_secs(), duration.subsec_millis())
    } else {
        format!("{}ms", duration.as_millis())
    }
}

fn focus_style(active: bool) -> Style {
    if active {
        Style::default().fg(Color::LightCyan)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn cursor_style() -> Style {
    Style::default().fg(Color::Black).bg(Color::White)
}

fn history_bounds(history: &[f64]) -> (f64, f64) {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for value in history {
        min = min.min(*value);
        max = max.max(*value);
    }

    if !min.is_finite() || !max.is_finite() {
        return (0.0, 1.0);
    }

    if (max - min).abs() < f64::EPSILON {
        return (min - 1.0, max + 1.0);
    }

    let padding = (max - min) * 0.1;
    (min - padding, max + padding)
}

pub struct EmptyWidget;

impl Widget for EmptyWidget {
    fn render(self, _area: Rect, _buf: &mut Buffer) {}
}
