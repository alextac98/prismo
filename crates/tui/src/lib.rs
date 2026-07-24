use std::cmp;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use prismo_core::{ChannelSnapshot, ChannelValue, NumericPoint, StoreSnapshot};
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::prelude::{Alignment, Color, Line, Modifier, Style};
use ratatui::symbols;
use ratatui::text::Span;
use ratatui::widgets::{
    Axis, Block, Borders, Chart, Clear, Dataset, GraphType, List, ListItem, ListState, Paragraph,
    Scrollbar, ScrollbarOrientation, ScrollbarState, Tabs, Wrap,
};

const MIN_TERMINAL_WIDTH: u16 = 80;
const MIN_TERMINAL_HEIGHT: u16 = 20;
const PRISMO_VERSION: &str = env!("PRISMO_VERSION");

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
    RunCommand(String),
    YankSelectedValue,
    ToggleSelectedNamespace,
    ToggleAllNamespaces,
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
    pub command_mode: bool,
    pub filter_mode: bool,
    pub help_mode: bool,
    pub command_input: String,
    pub filter_input: String,
    pub status_notice: Option<StatusNotice>,
    channel_area: Option<Rect>,
    plugin_tab_hitboxes: Vec<PluginTabHitbox>,
    details_area: Option<Rect>,
    latest_area: Option<Rect>,
    rendered_channels: usize,
    channel_scroll_offset: usize,
    channel_view_rows: usize,
    details_scroll_offset: usize,
    latest_scroll_offset: usize,
    selected_plugin: usize,
    details_cursor: TextCursor,
    latest_cursor: TextCursor,
    collapsed_namespaces: HashSet<String>,
    collapsed_arrays: HashSet<String>,
}

pub struct CopyPayload {
    pub text: String,
    pub label: String,
}

pub struct SelectedValue<'a> {
    pub channel: &'a ChannelSnapshot,
    pub value: &'a ChannelValue,
    pub path: String,
}

#[derive(Clone)]
struct SelectableLine {
    raw: String,
    rendered: Line<'static>,
}

struct PaneContent {
    detail_lines: Vec<SelectableLine>,
    latest_title: String,
    latest_content: LatestPaneContent,
}

#[derive(Clone, Debug)]
struct PluginTabHitbox {
    index: usize,
    area: Rect,
}

enum LatestPaneContent {
    Text(Vec<SelectableLine>),
    Numeric {
        summary: Vec<SelectableLine>,
        series: Vec<NumericSeries>,
        min_x: f64,
        max_x: f64,
        x_labels: [String; 2],
        min_y: f64,
        max_y: f64,
    },
}

struct NumericSeries {
    name: String,
    color: Color,
    points: Vec<(f64, f64)>,
}

#[derive(Clone, Copy)]
enum NumericGraphKind {
    Linear,
    StepAfter,
}

#[derive(Default)]
struct NamespaceNode<'a> {
    path: String,
    name: String,
    namespaces: BTreeMap<String, NamespaceNode<'a>>,
    channels: Vec<&'a ChannelSnapshot>,
}

#[derive(Clone)]
struct TreeRow<'a> {
    depth: usize,
    kind: TreeRowKind<'a>,
}

#[derive(Clone)]
enum TreeRowKind<'a> {
    Namespace {
        path: String,
        name: String,
        descendant_channels: Vec<&'a ChannelSnapshot>,
        child_namespace_count: usize,
        direct_channel_count: usize,
        collapsed: bool,
    },
    Channel {
        channel: &'a ChannelSnapshot,
    },
    Array {
        channel: &'a ChannelSnapshot,
        value: &'a ChannelValue,
        index_path: Vec<usize>,
        collapsed: bool,
    },
    ArrayElement {
        channel: &'a ChannelSnapshot,
        value: &'a ChannelValue,
        index_path: Vec<usize>,
    },
}

#[derive(Clone)]
enum RowKey {
    Namespace(String),
    Channel(String),
    Array(String),
    ArrayElement(String),
}

enum CollapsibleKey {
    Namespace(String),
    Array(String),
}

impl Default for FocusPane {
    fn default() -> Self {
        Self::Channels
    }
}

impl<'a> NamespaceNode<'a> {
    fn new(name: String, path: String) -> Self {
        Self {
            path,
            name,
            namespaces: BTreeMap::new(),
            channels: Vec::new(),
        }
    }
}

impl UiState {
    pub fn new() -> Self {
        Self {
            selected: 0,
            focus: FocusPane::Channels,
            command_mode: false,
            filter_mode: false,
            help_mode: false,
            command_input: String::new(),
            filter_input: String::new(),
            status_notice: None,
            channel_area: None,
            plugin_tab_hitboxes: Vec::new(),
            details_area: None,
            latest_area: None,
            rendered_channels: 0,
            channel_scroll_offset: 0,
            channel_view_rows: 0,
            details_scroll_offset: 0,
            latest_scroll_offset: 0,
            selected_plugin: 0,
            details_cursor: TextCursor::default(),
            latest_cursor: TextCursor::default(),
            collapsed_namespaces: HashSet::new(),
            collapsed_arrays: HashSet::new(),
        }
    }

    pub fn selected_channel<'a>(&self, snapshot: &'a StoreSnapshot) -> Option<&'a ChannelSnapshot> {
        match self.selected_row(snapshot)?.kind {
            TreeRowKind::Channel { channel } => Some(channel),
            TreeRowKind::Namespace { .. }
            | TreeRowKind::Array { .. }
            | TreeRowKind::ArrayElement { .. } => None,
        }
    }

    pub fn selected_namespace_path(&self, snapshot: &StoreSnapshot) -> Option<String> {
        match self.selected_row(snapshot)?.kind {
            TreeRowKind::Namespace { path, .. } => Some(path),
            TreeRowKind::Array {
                channel,
                index_path,
                ..
            } => Some(array_display_path(channel, &index_path)),
            TreeRowKind::Channel { .. } | TreeRowKind::ArrayElement { .. } => None,
        }
    }

    pub fn selected_value<'a>(&self, snapshot: &'a StoreSnapshot) -> Option<SelectedValue<'a>> {
        match self.selected_row(snapshot)?.kind {
            TreeRowKind::Channel { channel } => {
                channel.latest.as_ref().map(|sample| SelectedValue {
                    channel,
                    value: &sample.value,
                    path: channel.descriptor.path.clone(),
                })
            }
            TreeRowKind::ArrayElement {
                channel,
                value,
                index_path,
            } => Some(SelectedValue {
                channel,
                value,
                path: array_display_path(channel, &index_path),
            }),
            TreeRowKind::Namespace { .. } | TreeRowKind::Array { .. } => None,
        }
    }

    pub fn toggle_selected_namespace(&mut self, snapshot: &StoreSnapshot) -> bool {
        let selection = self.selected_row_key(snapshot);
        let Some(key) = self.selected_collapsible_key(snapshot) else {
            return false;
        };

        match key {
            CollapsibleKey::Namespace(path) => {
                if !self.collapsed_namespaces.insert(path.clone()) {
                    self.collapsed_namespaces.remove(&path);
                }
            }
            CollapsibleKey::Array(path) => {
                if !self.collapsed_arrays.insert(path.clone()) {
                    self.collapsed_arrays.remove(&path);
                }
            }
        }
        self.restore_selection(snapshot, selection);

        true
    }

    pub fn toggle_all_namespaces(&mut self, snapshot: &StoreSnapshot) -> (bool, usize) {
        let selection = self.selected_row_key(snapshot);
        let namespace_paths = visible_namespace_paths(self.filtered_channels(snapshot));
        let array_paths = visible_array_paths(self.filtered_channels(snapshot));
        let count = namespace_paths.len() + array_paths.len();
        if count == 0 {
            return (false, 0);
        }

        let all_namespaces_collapsed = namespace_paths
            .iter()
            .all(|path| self.collapsed_namespaces.contains(path));
        let all_arrays_collapsed = array_paths
            .iter()
            .all(|path| self.collapsed_arrays.contains(path));
        let all_collapsed = all_namespaces_collapsed && all_arrays_collapsed;
        if all_collapsed {
            self.collapsed_namespaces
                .retain(|path| !namespace_paths.contains(path));
            self.collapsed_arrays
                .retain(|path| !array_paths.contains(path));
        } else {
            self.collapsed_namespaces
                .extend(namespace_paths.iter().cloned());
            self.collapsed_arrays.extend(array_paths.iter().cloned());
        }
        self.restore_selection(snapshot, selection);

        (!all_collapsed, count)
    }

    pub fn clamp_selection(&mut self, total: usize) {
        self.rendered_channels = total;
        if total == 0 {
            self.selected = 0;
            self.channel_scroll_offset = 0;
        } else {
            self.selected = cmp::min(self.selected, total - 1);
            let max_offset = total.saturating_sub(self.channel_view_rows.max(1));
            self.channel_scroll_offset = cmp::min(self.channel_scroll_offset, max_offset);
            if self.selected < self.channel_scroll_offset {
                self.channel_scroll_offset = self.selected;
            }
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) -> UiAction {
        if self.help_mode {
            match key.code {
                KeyCode::Esc | KeyCode::Char('?') => {
                    self.help_mode = false;
                }
                _ => {}
            }
            return UiAction::None;
        }

        if self.command_mode {
            match key.code {
                KeyCode::Esc => {
                    self.command_mode = false;
                    self.command_input.clear();
                }
                KeyCode::Enter => {
                    self.command_mode = false;
                    let command = self.command_input.trim().to_string();
                    self.command_input.clear();
                    if !command.is_empty() {
                        return UiAction::RunCommand(command);
                    }
                }
                KeyCode::Backspace => {
                    self.command_input.pop();
                }
                KeyCode::Char(c) => {
                    self.command_input.push(c);
                }
                _ => {}
            }
            return UiAction::None;
        }

        if self.filter_mode {
            match key.code {
                KeyCode::Esc => {
                    self.filter_mode = false;
                    self.filter_input.clear();
                    self.selected = 0;
                }
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
            KeyCode::Char('y') => UiAction::YankSelectedValue,
            KeyCode::Enter if self.focus == FocusPane::Channels => {
                UiAction::ToggleSelectedNamespace
            }
            KeyCode::Char('z') if self.focus == FocusPane::Channels => {
                UiAction::ToggleAllNamespaces
            }
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
                    self.move_channel_selection_up(1);
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
                    self.move_channel_selection_up(1);
                } else if let Some(cursor) = self.focused_text_cursor_mut() {
                    cursor.move_up();
                }
                UiAction::None
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if self.focus == FocusPane::Channels {
                    self.select_previous_plugin();
                } else if let Some(cursor) = self.focused_text_cursor_mut() {
                    cursor.move_left();
                }
                UiAction::None
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if self.focus == FocusPane::Channels {
                    self.select_next_plugin();
                } else if let Some(cursor) = self.focused_text_cursor_mut() {
                    cursor.move_right();
                }
                UiAction::None
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.selected = 0;
                self.channel_scroll_offset = 0;
                UiAction::None
            }
            KeyCode::Char('G') | KeyCode::End => {
                if self.rendered_channels > 0 {
                    self.selected = self.rendered_channels - 1;
                    self.channel_scroll_offset = self
                        .rendered_channels
                        .saturating_sub(self.channel_view_rows.max(1));
                }
                UiAction::None
            }
            KeyCode::Char('/') => {
                self.filter_mode = true;
                UiAction::None
            }
            KeyCode::Char(':') => {
                self.command_mode = true;
                self.command_input.clear();
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

    pub fn on_mouse(&mut self, event: MouseEvent) {
        let column = event.column;
        let row = event.row;
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(hitbox) = self
                    .plugin_tab_hitboxes
                    .iter()
                    .find(|hitbox| rect_contains(hitbox.area, column, row))
                    .cloned()
                {
                    self.select_plugin(hitbox.index);
                    self.focus = FocusPane::Channels;
                } else if let Some(area) = self
                    .channel_area
                    .filter(|area| rect_contains(*area, column, row))
                {
                    let list_row = row.saturating_sub(area.y);
                    let index = self.channel_scroll_offset + usize::from(list_row);
                    if index < self.rendered_channels {
                        self.selected = index;
                        self.focus = FocusPane::Channels;
                    }
                } else if let Some(area) = self
                    .details_area
                    .filter(|area| rect_contains(*area, column, row))
                {
                    self.focus = FocusPane::Details;
                    self.details_cursor.point.line =
                        self.details_scroll_offset + usize::from(row.saturating_sub(area.y));
                    self.details_cursor.point.column = usize::from(column.saturating_sub(area.x));
                } else if let Some(area) = self
                    .latest_area
                    .filter(|area| rect_contains(*area, column, row))
                {
                    self.focus = FocusPane::LatestValue;
                    self.latest_cursor.point.line =
                        self.latest_scroll_offset + usize::from(row.saturating_sub(area.y));
                    self.latest_cursor.point.column = usize::from(column.saturating_sub(area.x));
                }
            }
            MouseEventKind::ScrollDown => {
                if self
                    .channel_area
                    .is_some_and(|area| rect_contains(area, column, row))
                {
                    self.focus = FocusPane::Channels;
                    self.move_channel_selection(1);
                } else if self
                    .latest_area
                    .is_some_and(|area| rect_contains(area, column, row))
                {
                    self.focus = FocusPane::LatestValue;
                    self.latest_cursor.move_down();
                }
            }
            MouseEventKind::ScrollUp => {
                if self
                    .channel_area
                    .is_some_and(|area| rect_contains(area, column, row))
                {
                    self.focus = FocusPane::Channels;
                    self.move_channel_selection_up(1);
                } else if self
                    .latest_area
                    .is_some_and(|area| rect_contains(area, column, row))
                {
                    self.focus = FocusPane::LatestValue;
                    self.latest_cursor.move_up();
                }
            }
            _ => {}
        }
    }

    pub fn set_channel_area(&mut self, area: Rect) {
        self.channel_area = Some(area);
        self.channel_view_rows = area.height as usize;
        self.clamp_selection(self.rendered_channels);
    }

    pub fn set_plugin_tabs(&mut self, area: Rect, plugin_ids: &[String]) {
        self.plugin_tab_hitboxes.clear();
        if area.height == 0 {
            return;
        }

        let mut x = area.x;
        for (index, plugin_id) in plugin_ids.iter().enumerate() {
            if x >= area.right() {
                break;
            }

            let width = (plugin_id.chars().count() as u16).min(area.right().saturating_sub(x));
            if width > 0 {
                self.plugin_tab_hitboxes.push(PluginTabHitbox {
                    index,
                    area: Rect::new(x, area.y, width, area.height),
                });
            }

            x = x.saturating_add(width).saturating_add(1);
        }
    }

    pub fn set_details_area(&mut self, area: Rect) {
        self.details_area = Some(area);
    }

    pub fn set_latest_area(&mut self, area: Rect) {
        self.latest_area = Some(area);
    }

    pub fn clear_scroll_areas(&mut self) {
        self.channel_area = None;
        self.plugin_tab_hitboxes.clear();
        self.details_area = None;
        self.latest_area = None;
        self.channel_view_rows = 0;
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
            let view_rows = self.channel_view_rows.max(1);
            if self.selected >= self.channel_scroll_offset + view_rows {
                self.channel_scroll_offset = self.selected + 1 - view_rows;
            }
        }
    }

    fn move_channel_selection_up(&mut self, amount: usize) {
        self.selected = self.selected.saturating_sub(amount);
        if self.selected < self.channel_scroll_offset {
            self.channel_scroll_offset = self.selected;
        }
    }

    fn select_previous_plugin(&mut self) {
        if self.selected_plugin > 0 {
            self.selected_plugin -= 1;
            self.selected = 0;
            self.channel_scroll_offset = 0;
        }
    }

    fn select_next_plugin(&mut self) {
        self.selected_plugin = self.selected_plugin.saturating_add(1);
        self.selected = 0;
        self.channel_scroll_offset = 0;
    }

    fn select_plugin(&mut self, index: usize) {
        if self.selected_plugin != index {
            self.selected_plugin = index;
            self.selected = 0;
            self.channel_scroll_offset = 0;
        }
    }

    fn clamp_selected_plugin(&mut self, plugin_count: usize) {
        if plugin_count == 0 {
            self.selected_plugin = 0;
        } else {
            self.selected_plugin = self.selected_plugin.min(plugin_count - 1);
        }
    }

    fn filtered_channels<'a>(&self, snapshot: &'a StoreSnapshot) -> Vec<&'a ChannelSnapshot> {
        let needle = self.filter_input.trim().to_ascii_lowercase();
        let plugin_ids = plugin_ids(snapshot);
        let selected_plugin = plugin_ids.get(self.selected_plugin).map(String::as_str);
        snapshot
            .channels
            .iter()
            .filter(|channel| {
                if selected_plugin.is_some_and(|plugin_id| channel.plugin_id != plugin_id) {
                    return false;
                }

                needle.is_empty()
                    || channel
                        .descriptor
                        .path
                        .to_ascii_lowercase()
                        .contains(&needle)
                    || channel
                        .descriptor
                        .description
                        .to_ascii_lowercase()
                        .contains(&needle)
            })
            .collect()
    }

    fn tree_rows<'a>(&self, snapshot: &'a StoreSnapshot) -> Vec<TreeRow<'a>> {
        build_tree_rows(
            self.filtered_channels(snapshot),
            &self.collapsed_namespaces,
            &self.collapsed_arrays,
        )
    }

    fn selected_row<'a>(&self, snapshot: &'a StoreSnapshot) -> Option<TreeRow<'a>> {
        let rows = self.tree_rows(snapshot);
        rows.get(self.selected).cloned()
    }

    fn selected_row_key(&self, snapshot: &StoreSnapshot) -> Option<RowKey> {
        self.selected_row(snapshot).map(|row| row_key(&row))
    }

    fn selected_collapsible_key(&self, snapshot: &StoreSnapshot) -> Option<CollapsibleKey> {
        match self.selected_row(snapshot)?.kind {
            TreeRowKind::Namespace { path, .. } => Some(CollapsibleKey::Namespace(path)),
            TreeRowKind::Array {
                channel,
                index_path,
                ..
            } => Some(CollapsibleKey::Array(array_tree_path(channel, &index_path))),
            TreeRowKind::Channel { .. } | TreeRowKind::ArrayElement { .. } => None,
        }
    }

    fn restore_selection(&mut self, snapshot: &StoreSnapshot, key: Option<RowKey>) {
        let Some(key) = key else {
            return;
        };

        let rows = self.tree_rows(snapshot);
        if let Some(index) = rows.iter().position(|row| row_matches_key(row, &key)) {
            self.selected = index;
            return;
        }

        for fallback in ancestor_row_keys(&key) {
            if let Some(index) = rows.iter().position(|row| row_matches_key(row, &fallback)) {
                self.selected = index;
                return;
            }
        }

        self.clamp_selection(rows.len());
    }
}

pub fn selected_text(snapshot: &StoreSnapshot, ui: &UiState) -> Option<CopyPayload> {
    let row = ui.selected_row(snapshot)?;
    let content = pane_content_for_row(&row, ui.filter_mode, &ui.filter_input);

    match ui.focus {
        FocusPane::Details => {
            extract_copy_from_selectable_lines(&content.detail_lines, &ui.details_cursor).map(
                |text| CopyPayload {
                    text,
                    label: "details line".to_string(),
                },
            )
        }
        FocusPane::LatestValue => {
            let lines = latest_selectable_lines(&content.latest_content);
            extract_copy_from_selectable_lines(&lines, &ui.latest_cursor).map(|text| CopyPayload {
                text,
                label: format!("{} line", content.latest_title.to_ascii_lowercase()),
            })
        }
        FocusPane::Channels => None,
    }
}

pub fn draw(frame: &mut ratatui::Frame<'_>, snapshot: &StoreSnapshot, ui: &mut UiState) {
    if frame.area().width < MIN_TERMINAL_WIDTH || frame.area().height < MIN_TERMINAL_HEIGHT {
        ui.clear_scroll_areas();
        let message = Paragraph::new(format!(
            "This window is too small.\nMinimum size: {}x{}",
            MIN_TERMINAL_WIDTH, MIN_TERMINAL_HEIGHT
        ))
        .block(Block::default().title("prismo").borders(Borders::ALL))
        .alignment(Alignment::Center);
        frame.render_widget(message, frame.area());
        return;
    }

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(frame.area());
    let main_area = root[0];
    let status_area = root[1];

    let vertical = frame.area().width < 110;
    let (detail_area, channel_area) = if vertical {
        let root = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(main_area);
        (root[0], root[1])
    } else {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(main_area);
        (columns[0], columns[1])
    };

    let plugin_ids = plugin_ids(snapshot);
    ui.clamp_selected_plugin(plugin_ids.len());
    let selected_plugin_id = plugin_ids.get(ui.selected_plugin).map(String::as_str);
    let filtered_channels = ui.filtered_channels(snapshot);
    let channel_count = filtered_channels.len();
    let rows = build_tree_rows(
        filtered_channels,
        &ui.collapsed_namespaces,
        &ui.collapsed_arrays,
    );
    ui.clamp_selection(rows.len());

    let mut list_state = ListState::default()
        .with_selected(Some(ui.selected))
        .with_offset(ui.channel_scroll_offset);
    let items = rows
        .iter()
        .map(|row| ListItem::new(render_tree_row(row)))
        .collect::<Vec<_>>();

    let list_block = Block::default()
        .title(if ui.filter_mode || !ui.filter_input.is_empty() {
            "Channels / filtered"
        } else {
            "Channels"
        })
        .title_top(Line::from("z toggle tree").right_aligned())
        .borders(Borders::ALL)
        .border_style(focus_style(ui.focus == FocusPane::Channels));
    let channel_inner = list_block.inner(channel_area);
    let stats_height = u16::from(channel_inner.height > 0);
    let tab_height = u16::from(plugin_ids.len() > 1 && channel_inner.height > stats_height);
    let channel_sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(tab_height),
            Constraint::Min(0),
            Constraint::Length(stats_height),
        ])
        .split(channel_inner);
    let tab_area = channel_sections[0];
    let list_area = channel_sections[1];
    let stats_area = channel_sections[2];
    let plugin_stats = if let Some(plugin_id) = selected_plugin_id {
        if let Some(plugin) = snapshot
            .plugins
            .iter()
            .find(|plugin| plugin.plugin_id == plugin_id)
        {
            format!(
                "updates:{} dropped:{} channels:{}",
                plugin.health.emitted_updates, plugin.health.dropped_updates, channel_count
            )
        } else {
            format!("starting channels:{channel_count}")
        }
    } else {
        "plugins: starting".to_string()
    };
    let (channel_list_area, channel_scrollbar_area) = split_scrollable_area(list_area, rows.len());
    ui.set_channel_area(channel_list_area);
    ui.set_plugin_tabs(tab_area, &plugin_ids);
    frame.render_widget(list_block, channel_area);
    if plugin_ids.len() > 1 && tab_area.height > 0 {
        let tabs = Tabs::new(
            plugin_ids
                .iter()
                .cloned()
                .map(Line::from)
                .collect::<Vec<_>>(),
        )
        .select(ui.selected_plugin)
        .style(Style::default().fg(Color::Gray))
        .highlight_style(
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        );
        frame.render_widget(tabs, tab_area);
    }
    if stats_area.height > 0 {
        frame.render_widget(
            Paragraph::new(format!("Stats: {plugin_stats}")).alignment(Alignment::Left),
            stats_area,
        );
    }
    let channel_list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    frame.render_stateful_widget(channel_list, channel_list_area, &mut list_state);
    render_vertical_scrollbar(
        frame,
        channel_scrollbar_area,
        rows.len(),
        channel_list_area.height as usize,
        ui.channel_scroll_offset,
        ui.focus == FocusPane::Channels,
    );

    let mut cursor_position = if let Some(selected) = rows.get(ui.selected) {
        render_selection_detail(frame, detail_area, selected, ui)
    } else {
        ui.set_details_area(Rect::default());
        ui.set_latest_area(Rect::default());
        let empty_message = if plugin_ids.is_empty() {
            "No channels are registered yet.".to_string()
        } else if ui.filter_input.is_empty() {
            format!(
                "No channels registered for {}.",
                plugin_ids
                    .get(ui.selected_plugin)
                    .map(String::as_str)
                    .unwrap_or("this plugin")
            )
        } else {
            "No channels match the current filter.".to_string()
        };
        let empty = Paragraph::new(empty_message)
            .block(Block::default().title("Selection").borders(Borders::ALL))
            .alignment(Alignment::Center);
        frame.render_widget(empty, detail_area);
        None
    };

    let status_left = if let Some(notice) = ui.status_notice() {
        format!(":q/:Q quit  : command  ? help  {notice}")
    } else {
        " :q quit | : command | ? help |".to_string()
    };

    let status_style = Style::default().fg(Color::Black).bg(Color::Gray);
    let version = format!("Prismo v{PRISMO_VERSION} ");
    let version_width = version.len().min(status_area.width as usize) as u16;
    let status_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(version_width)])
        .split(status_area);
    let left = Paragraph::new(status_left).style(status_style);
    let right = Paragraph::new(version)
        .style(status_style)
        .alignment(Alignment::Right);
    frame.render_widget(left, status_chunks[0]);
    frame.render_widget(right, status_chunks[1]);

    if ui.filter_mode || !ui.filter_input.is_empty() {
        let popup = filter_bar_rect(frame.area(), ui.filter_input.chars().count() as u16);
        frame.render_widget(Clear, popup);
        let filter = Paragraph::new(format!("/{}", ui.filter_input))
            .block(Block::default().title("Filter").borders(Borders::ALL))
            .wrap(Wrap { trim: false });
        frame.render_widget(filter, popup);
        if ui.filter_mode {
            cursor_position = Some(Position::new(
                popup.x + 2 + ui.filter_input.chars().count() as u16,
                popup.y + 1,
            ));
        }
    }

    if ui.command_mode {
        let popup = filter_bar_rect(frame.area(), ui.command_input.chars().count() as u16);
        frame.render_widget(Clear, popup);
        let command = Paragraph::new(format!(":{}", ui.command_input))
            .block(Block::default().title("Command").borders(Borders::ALL))
            .wrap(Wrap { trim: false });
        frame.render_widget(command, popup);
        cursor_position = Some(Position::new(
            popup.x + 2 + ui.command_input.chars().count() as u16,
            popup.y + 1,
        ));
    }

    if ui.help_mode {
        let popup = centered_rect(18, 78, frame.area());
        frame.render_widget(Clear, popup);
        let help_lines = vec![
            Line::from("Navigation"),
            Line::from("Tab cycle focus between Details, Latest Value, and Channels"),
            Line::from(
                "j/k move selection in Channels, or move cursor up/down in focused text panes",
            ),
            Line::from("h/l or Left/Right switch plugin tabs when Channels is focused"),
            Line::from("h/l or Left/Right move cursor horizontally in focused text panes"),
            Line::from("g/G jump to the first or last visible row"),
            Line::from("Enter collapse or expand the selected namespace or array in Channels"),
            Line::from("z toggle collapse or expand for the full channel tree"),
            Line::from(""),
            Line::from("Actions"),
            Line::from(
                "y copy the current line in Details or Latest Value, or copy the live value in Channels",
            ),
            Line::from("/ open channel filter"),
            Line::from(": open command mode"),
            Line::from(":q or :Q quit prismo"),
            Line::from("Mouse click select a row in the Channels pane"),
            Line::from("Esc close filter, command, or help"),
            Line::from("? toggle this help"),
        ];
        let help = Paragraph::new(help_lines)
            .block(Block::default().title("Help").borders(Borders::ALL))
            .wrap(Wrap { trim: false });
        frame.render_widget(help, popup);
    }

    if let Some(cursor) = cursor_position {
        frame.set_cursor_position(cursor);
    }
}

fn render_selection_detail(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    row: &TreeRow<'_>,
    ui: &mut UiState,
) -> Option<Position> {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(5)])
        .split(area);

    let content = pane_content_for_row(row, ui.filter_mode, &ui.filter_input);
    let detail_block = Block::default()
        .title("Details")
        .borders(Borders::ALL)
        .border_style(focus_style(
            !ui.filter_mode && ui.focus == FocusPane::Details,
        ));
    let detail_inner = detail_block.inner(sections[0]);
    frame.render_widget(detail_block, sections[0]);
    ui.set_details_area(detail_inner);
    let detail_cursor = render_fixed_selectable_text(
        frame,
        detail_inner,
        &content.detail_lines,
        &mut ui.details_cursor,
        !ui.filter_mode && ui.focus == FocusPane::Details,
    );

    let latest_block = Block::default()
        .title(content.latest_title.as_str())
        .borders(Borders::ALL)
        .border_style(focus_style(ui.focus == FocusPane::LatestValue));
    let latest_inner = latest_block.inner(sections[1]);
    frame.render_widget(latest_block, sections[1]);
    ui.set_latest_area(latest_scroll_area(latest_inner, &content.latest_content));
    let latest_cursor = render_latest_value(
        frame,
        latest_inner,
        &content.latest_content,
        &mut ui.latest_cursor,
        &mut ui.latest_scroll_offset,
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
    scroll_offset: &mut usize,
    focused: bool,
) -> Option<Position> {
    match content {
        LatestPaneContent::Text(lines) => {
            render_selectable_text(frame, area, lines, cursor, scroll_offset, focused)
        }
        LatestPaneContent::Numeric {
            summary,
            series,
            min_x,
            max_x,
            x_labels,
            min_y,
            max_y,
        } => {
            let summary_height = cmp::min(area.height, summary.len() as u16 + 1);
            let sections = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(summary_height), Constraint::Min(0)])
                .split(area);
            let cursor_position =
                render_selectable_text(frame, sections[0], summary, cursor, scroll_offset, focused);

            if sections[1].height > 0 {
                let datasets = series
                    .iter()
                    .map(|series| {
                        let dataset = Dataset::default()
                            .graph_type(GraphType::Line)
                            .marker(symbols::Marker::Braille)
                            .style(Style::default().fg(series.color))
                            .data(&series.points);
                        if series.name.is_empty() {
                            dataset
                        } else {
                            dataset.name(series.name.clone())
                        }
                    })
                    .collect::<Vec<_>>();
                let chart = Chart::new(datasets)
                    .x_axis(Axis::default().bounds([*min_x, *max_x]).labels([
                        Line::from(x_labels[0].clone()),
                        Line::from(x_labels[1].clone()),
                    ]))
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
    lines: &[SelectableLine],
    cursor: &mut TextCursor,
    scroll_offset: &mut usize,
    focused: bool,
) -> Option<Position> {
    if area.width == 0 || area.height == 0 {
        *scroll_offset = 0;
        return None;
    }

    if lines.is_empty() {
        *scroll_offset = 0;
        frame.render_widget(Paragraph::new(String::new()), area);
        return None;
    }

    let (text_area, scrollbar_area) = split_scrollable_area(area, lines.len());
    clamp_selectable_cursor(cursor, lines);
    let view_rows = text_area.height as usize;
    let max_offset = lines.len().saturating_sub(view_rows);
    *scroll_offset = (*scroll_offset).min(max_offset);
    if cursor.point.line < *scroll_offset {
        *scroll_offset = cursor.point.line;
    } else if cursor.point.line >= *scroll_offset + view_rows {
        *scroll_offset = cursor.point.line + 1 - view_rows;
    }

    let start = *scroll_offset;
    let end = (start + view_rows).min(lines.len());
    let visible_lines = &lines[start..end];
    let visible_cursor = TextCursor {
        point: TextPoint {
            line: cursor.point.line.saturating_sub(start),
            column: cursor.point.column,
        },
    };

    let rendered = build_rendered_lines(visible_lines, &visible_cursor, focused);
    frame.render_widget(
        Paragraph::new(rendered).wrap(Wrap { trim: false }),
        text_area,
    );
    render_vertical_scrollbar(
        frame,
        scrollbar_area,
        lines.len(),
        view_rows,
        *scroll_offset,
        focused,
    );

    if !focused {
        return None;
    }

    let line = cmp::min(
        visible_cursor.point.line as u16,
        text_area.height.saturating_sub(1),
    );
    let col = cmp::min(
        visible_cursor.point.column as u16,
        text_area.width.saturating_sub(1),
    );
    Some(Position::new(text_area.x + col, text_area.y + line))
}

fn render_fixed_selectable_text(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    lines: &[SelectableLine],
    cursor: &mut TextCursor,
    focused: bool,
) -> Option<Position> {
    if area.width == 0 || area.height == 0 {
        return None;
    }

    if lines.is_empty() {
        frame.render_widget(Paragraph::new(String::new()), area);
        return None;
    }

    let visible_lines = &lines[..lines.len().min(area.height as usize)];
    clamp_selectable_cursor(cursor, visible_lines);
    let rendered = build_rendered_lines(visible_lines, cursor, focused);
    frame.render_widget(Paragraph::new(rendered).wrap(Wrap { trim: false }), area);

    if !focused {
        return None;
    }

    let line = cmp::min(cursor.point.line as u16, area.height.saturating_sub(1));
    let col = cmp::min(cursor.point.column as u16, area.width.saturating_sub(1));
    Some(Position::new(area.x + col, area.y + line))
}

fn render_tree_row(row: &TreeRow<'_>) -> Line<'static> {
    match &row.kind {
        TreeRowKind::Namespace {
            name,
            descendant_channels,
            collapsed,
            ..
        } => {
            let indent = "  ".repeat(row.depth);
            let icon = if *collapsed { "▸" } else { "▾" };
            Line::from(vec![
                Span::raw(indent),
                Span::styled(format!("{icon} {name}"), Style::default().fg(Color::Yellow)),
                Span::raw(" "),
                Span::styled(
                    format!("[{}]", descendant_channels.len()),
                    Style::default().fg(Color::Gray),
                ),
            ])
        }
        TreeRowKind::Channel { channel } => {
            let indent = "  ".repeat(row.depth);
            let marker = if channel.is_stale { "stale" } else { "live" };
            let value = channel
                .latest
                .as_ref()
                .map(|sample| sample.value.short_display())
                .unwrap_or_else(|| "waiting".to_string());
            Line::from(vec![
                Span::raw(indent),
                Span::styled("• ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    channel.descriptor.display_name.clone(),
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
            ])
        }
        TreeRowKind::Array {
            channel,
            value,
            index_path,
            collapsed,
        } => {
            let indent = "  ".repeat(row.depth);
            let icon = if *collapsed { "▸" } else { "▾" };
            let name = index_path
                .last()
                .map(|index| format!("[{index}]"))
                .unwrap_or_else(|| channel.descriptor.display_name.clone());
            let item_count = match value {
                ChannelValue::Array { values, .. } => values.len(),
                _ => 0,
            };
            let mut spans = vec![
                Span::raw(indent),
                Span::styled(format!("{icon} {name}"), Style::default().fg(Color::Yellow)),
                Span::raw(" "),
                Span::styled(format!("[{item_count}]"), Style::default().fg(Color::Gray)),
            ];
            if index_path.is_empty() {
                let marker = if channel.is_stale { "stale" } else { "live" };
                spans.extend([
                    Span::raw(" "),
                    Span::styled(format!("[{marker}]"), status_style(channel.is_stale)),
                ]);
            }
            Line::from(spans)
        }
        TreeRowKind::ArrayElement {
            value, index_path, ..
        } => {
            let indent = "  ".repeat(row.depth);
            let name = index_path
                .last()
                .map(|index| format!("[{index}]"))
                .unwrap_or_else(|| "[]".to_string());
            Line::from(vec![
                Span::raw(indent),
                Span::styled("• ", Style::default().fg(Color::DarkGray)),
                Span::styled(name, Style::default().fg(Color::Cyan)),
                Span::raw(" "),
                Span::styled(value.short_display(), Style::default().fg(Color::Gray)),
            ])
        }
    }
}

fn latest_scroll_area(area: Rect, content: &LatestPaneContent) -> Rect {
    match content {
        LatestPaneContent::Text(_) => area,
        LatestPaneContent::Numeric { summary, .. } => {
            let summary_height = cmp::min(area.height, summary.len() as u16 + 1);
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(summary_height), Constraint::Min(0)])
                .split(area)[0]
        }
    }
}

fn split_scrollable_area(area: Rect, content_length: usize) -> (Rect, Option<Rect>) {
    if content_length > area.height as usize && area.width > 1 {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    }
}

fn render_vertical_scrollbar(
    frame: &mut ratatui::Frame<'_>,
    area: Option<Rect>,
    content_length: usize,
    viewport_length: usize,
    position: usize,
    focused: bool,
) {
    let Some(area) = area else {
        return;
    };
    if area.width == 0 || area.height == 0 || content_length <= viewport_length {
        return;
    }

    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .track_symbol(Some("│"))
        .thumb_style(if focused {
            Style::default().fg(Color::LightCyan)
        } else {
            Style::default().fg(Color::Gray)
        })
        .track_style(Style::default().fg(Color::DarkGray));
    let mut state = ScrollbarState::default()
        .content_length(content_length)
        .viewport_content_length(viewport_length)
        .position(position);
    frame.render_stateful_widget(scrollbar, area, &mut state);
}

fn pane_content_for_row(row: &TreeRow<'_>, filter_mode: bool, filter_input: &str) -> PaneContent {
    match &row.kind {
        TreeRowKind::Channel { channel } => PaneContent {
            detail_lines: build_channel_detail_lines(channel, filter_mode, filter_input),
            latest_title: "Latest Value".to_string(),
            latest_content: build_channel_latest_content(channel),
        },
        TreeRowKind::Namespace {
            path,
            descendant_channels,
            child_namespace_count,
            direct_channel_count,
            collapsed,
            ..
        } => PaneContent {
            detail_lines: build_namespace_detail_lines(
                path,
                descendant_channels.len(),
                *child_namespace_count,
                *direct_channel_count,
                *collapsed,
                filter_mode,
                filter_input,
            ),
            latest_title: "Channels".to_string(),
            latest_content: LatestPaneContent::Text(build_namespace_variable_lines(
                path,
                descendant_channels,
            )),
        },
        TreeRowKind::Array {
            channel,
            value,
            index_path,
            ..
        } => PaneContent {
            detail_lines: build_array_detail_lines(channel, value, index_path),
            latest_title: "Elements".to_string(),
            latest_content: build_array_latest_content(channel, value, index_path),
        },
        TreeRowKind::ArrayElement {
            channel,
            value,
            index_path,
        } => PaneContent {
            detail_lines: build_array_element_detail_lines(channel, value, index_path),
            latest_title: "Value".to_string(),
            latest_content: build_array_element_latest_content(channel, value, index_path),
        },
    }
}

fn build_tree_rows<'a>(
    channels: Vec<&'a ChannelSnapshot>,
    collapsed_namespaces: &HashSet<String>,
    collapsed_arrays: &HashSet<String>,
) -> Vec<TreeRow<'a>> {
    let mut root = NamespaceNode::default();

    for channel in channels {
        let tree_path = channel_display_path(channel);
        let parts = tree_path.split('.').collect::<Vec<_>>();

        if parts.len() <= 1 {
            root.channels.push(channel);
            continue;
        }

        let mut node = &mut root;
        let mut current_path = String::new();
        for part in &parts[..parts.len() - 1] {
            if !current_path.is_empty() {
                current_path.push('.');
            }
            current_path.push_str(part);
            node = node
                .namespaces
                .entry((*part).to_string())
                .or_insert_with(|| NamespaceNode::new((*part).to_string(), current_path.clone()));
        }
        node.channels.push(channel);
    }

    let mut rows = Vec::new();
    append_tree_rows(&root, 0, collapsed_namespaces, collapsed_arrays, &mut rows);
    rows
}

fn visible_namespace_paths(channels: Vec<&ChannelSnapshot>) -> HashSet<String> {
    let mut paths = HashSet::new();
    for channel in channels {
        let mut current = String::new();
        let tree_path = channel_display_path(channel);
        let parts = tree_path.split('.').collect::<Vec<_>>();
        for part in &parts[..parts.len().saturating_sub(1)] {
            if !current.is_empty() {
                current.push('.');
            }
            current.push_str(part);
            paths.insert(current.clone());
        }
    }
    paths
}

fn visible_array_paths(channels: Vec<&ChannelSnapshot>) -> HashSet<String> {
    let mut paths = HashSet::new();
    for channel in channels {
        if let Some(value @ ChannelValue::Array { .. }) =
            channel.latest.as_ref().map(|sample| &sample.value)
        {
            collect_array_paths(channel, value, &mut Vec::new(), &mut paths);
        }
    }
    paths
}

fn collect_array_paths(
    channel: &ChannelSnapshot,
    value: &ChannelValue,
    index_path: &mut Vec<usize>,
    paths: &mut HashSet<String>,
) {
    let ChannelValue::Array { values, .. } = value else {
        return;
    };
    paths.insert(array_tree_path(channel, index_path));
    for (index, child) in values.iter().enumerate() {
        if matches!(child, ChannelValue::Array { .. }) {
            index_path.push(index);
            collect_array_paths(channel, child, index_path, paths);
            index_path.pop();
        }
    }
}

fn append_tree_rows<'a>(
    node: &NamespaceNode<'a>,
    depth: usize,
    collapsed_namespaces: &HashSet<String>,
    collapsed_arrays: &HashSet<String>,
    rows: &mut Vec<TreeRow<'a>>,
) {
    for namespace in node.namespaces.values() {
        let mut descendant_channels = collect_descendant_channels(namespace);
        descendant_channels.sort_by_key(|channel| channel_display_path(channel));
        let collapsed = collapsed_namespaces.contains(&namespace.path);
        rows.push(TreeRow {
            depth,
            kind: TreeRowKind::Namespace {
                path: namespace.path.clone(),
                name: namespace.name.clone(),
                descendant_channels,
                child_namespace_count: namespace.namespaces.len(),
                direct_channel_count: namespace.channels.len(),
                collapsed,
            },
        });
        if !collapsed {
            append_tree_rows(
                namespace,
                depth + 1,
                collapsed_namespaces,
                collapsed_arrays,
                rows,
            );
        }
    }

    for channel in &node.channels {
        match channel.latest.as_ref().map(|sample| &sample.value) {
            Some(value @ ChannelValue::Array { .. }) => {
                append_array_tree_rows(channel, value, Vec::new(), depth, collapsed_arrays, rows)
            }
            _ => rows.push(TreeRow {
                depth,
                kind: TreeRowKind::Channel { channel },
            }),
        }
    }
}

fn append_array_tree_rows<'a>(
    channel: &'a ChannelSnapshot,
    value: &'a ChannelValue,
    index_path: Vec<usize>,
    depth: usize,
    collapsed_arrays: &HashSet<String>,
    rows: &mut Vec<TreeRow<'a>>,
) {
    let ChannelValue::Array { values, .. } = value else {
        return;
    };
    let key = array_tree_path(channel, &index_path);
    let collapsed = collapsed_arrays.contains(&key);
    rows.push(TreeRow {
        depth,
        kind: TreeRowKind::Array {
            channel,
            value,
            index_path: index_path.clone(),
            collapsed,
        },
    });

    if collapsed {
        return;
    }

    for (index, child) in values.iter().enumerate() {
        let mut child_path = index_path.clone();
        child_path.push(index);
        if matches!(child, ChannelValue::Array { .. }) {
            append_array_tree_rows(
                channel,
                child,
                child_path,
                depth + 1,
                collapsed_arrays,
                rows,
            );
        } else {
            rows.push(TreeRow {
                depth: depth + 1,
                kind: TreeRowKind::ArrayElement {
                    channel,
                    value: child,
                    index_path: child_path,
                },
            });
        }
    }
}

fn collect_descendant_channels<'a>(node: &NamespaceNode<'a>) -> Vec<&'a ChannelSnapshot> {
    let mut channels = node.channels.clone();
    for child in node.namespaces.values() {
        channels.extend(collect_descendant_channels(child));
    }
    channels
}

fn build_rendered_lines(
    lines: &[SelectableLine],
    cursor: &TextCursor,
    focused: bool,
) -> Vec<Line<'static>> {
    let cursor = clamped_selectable_cursor(cursor, lines);
    lines
        .iter()
        .enumerate()
        .map(|(line_idx, line)| {
            let mut chars = Vec::new();
            for span in &line.rendered.spans {
                for ch in span.content.chars() {
                    chars.push((ch, span.style));
                }
            }
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

            for (col_idx, (ch, base_style)) in chars.iter().enumerate() {
                let is_cursor =
                    focused && cursor.point.line == line_idx && cursor.point.column == col_idx;
                let style = if is_cursor {
                    cursor_style()
                } else {
                    *base_style
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

fn build_channel_detail_lines(
    channel: &ChannelSnapshot,
    _filter_mode: bool,
    _filter_input: &str,
) -> Vec<SelectableLine> {
    let latest = channel.latest.as_ref();
    let latest_value = latest
        .map(|sample| sample.value.to_string())
        .unwrap_or_else(|| "waiting for data".to_string());
    let last_received = latest
        .map(|sample| format_duration(sample.observed_at.elapsed()))
        .unwrap_or_else(|| "n/a".to_string());
    let rate = channel
        .rate_hz
        .map(|rate| format!("{rate:.2} Hz"))
        .unwrap_or_else(|| "n/a".to_string());

    vec![
        labeled_value_line("Path", &channel.descriptor.path, primary_style()),
        detail_row("Type", "channel", "Value", &latest_value),
        SelectableLine {
            raw: format!(
                "{:<32}  Units: {}",
                format!(
                    "Updates: [{}] {}",
                    if channel.is_stale { "stale" } else { "live" },
                    channel.update_count
                ),
                channel.descriptor.unit.as_deref().unwrap_or("-")
            ),
            rendered: Line::from(vec![
                Span::styled("Updates: ", label_style()),
                Span::styled(
                    format!("[{}]", if channel.is_stale { "stale" } else { "live" }),
                    status_style(channel.is_stale),
                ),
                Span::raw(" "),
                Span::styled(channel.update_count.to_string(), value_style()),
                Span::raw(
                    " ".repeat(
                        34usize.saturating_sub(
                            format!(
                                "Updates: [{}] {}",
                                if channel.is_stale { "stale" } else { "live" },
                                channel.update_count
                            )
                            .chars()
                            .count(),
                        ),
                    ),
                ),
                Span::styled("Units: ", label_style()),
                Span::styled(
                    channel
                        .descriptor
                        .unit
                        .as_deref()
                        .unwrap_or("-")
                        .to_string(),
                    value_style(),
                ),
            ]),
        },
        detail_row("Rate", &rate, "Last Received", &last_received),
        detail_row(
            "Plugin",
            &channel.plugin_id,
            "Description",
            &channel.descriptor.description,
        ),
    ]
}

fn build_namespace_detail_lines(
    path: &str,
    variable_count: usize,
    child_namespace_count: usize,
    _direct_channel_count: usize,
    _collapsed: bool,
    _filter_mode: bool,
    _filter_input: &str,
) -> Vec<SelectableLine> {
    vec![
        labeled_value_line("Path", path, primary_style()),
        labeled_value_line("Type", "namespace", primary_style()),
        labeled_value_line(
            "Child Namespaces",
            &child_namespace_count.to_string(),
            value_style(),
        ),
        labeled_value_line("Total Channels", &variable_count.to_string(), value_style()),
        labeled_value_line("Description", "", value_style()),
    ]
}

fn build_array_detail_lines(
    channel: &ChannelSnapshot,
    value: &ChannelValue,
    index_path: &[usize],
) -> Vec<SelectableLine> {
    let ChannelValue::Array { leaf_type, .. } = value else {
        return Vec::new();
    };
    let state = if channel.is_stale { "stale" } else { "live" };
    let array_type = format!("array ({})", leaf_type.to_string().to_ascii_lowercase());
    let description = if index_path.is_empty() {
        channel.descriptor.description.as_str()
    } else {
        "Nested array element"
    };

    vec![
        labeled_value_line(
            "Path",
            &array_display_path(channel, index_path),
            primary_style(),
        ),
        labeled_value_line("Type", &array_type, primary_style()),
        labeled_value_line("Shape", &array_shape(value), value_style()),
        detail_row("Plugin", &channel.plugin_id, "State", state),
        labeled_value_line("Description", description, value_style()),
    ]
}

fn array_shape(value: &ChannelValue) -> String {
    let ChannelValue::Array {
        dimensions, values, ..
    } = value
    else {
        return String::new();
    };

    let outer_size = values.len().to_string();
    if *dimensions <= 1 {
        return outer_size;
    }
    if values.is_empty() {
        return std::iter::once(outer_size)
            .chain(std::iter::repeat_n(
                "?".to_string(),
                dimensions.saturating_sub(1) as usize,
            ))
            .collect::<Vec<_>>()
            .join("x");
    }

    let child_shapes = values
        .iter()
        .map(array_shape)
        .filter(|shape| !shape.is_empty())
        .collect::<Vec<_>>();
    if child_shapes.len() != values.len() {
        return format!("{outer_size}x?");
    }

    let first = &child_shapes[0];
    if child_shapes.iter().all(|shape| shape == first) {
        format!("{outer_size}x{first}")
    } else {
        format!("{outer_size}xragged")
    }
}

fn build_array_element_detail_lines(
    channel: &ChannelSnapshot,
    value: &ChannelValue,
    index_path: &[usize],
) -> Vec<SelectableLine> {
    let state = if channel.is_stale { "stale" } else { "live" };
    vec![
        labeled_value_line(
            "Path",
            &array_display_path(channel, index_path),
            primary_style(),
        ),
        detail_row(
            "Type",
            channel_value_type_name(value),
            "Value",
            &value.to_string(),
        ),
        detail_row(
            "Index",
            &index_path
                .last()
                .map(usize::to_string)
                .unwrap_or_else(|| "n/a".to_string()),
            "Units",
            channel.descriptor.unit.as_deref().unwrap_or("-"),
        ),
        detail_row("Plugin", &channel.plugin_id, "State", state),
        labeled_value_line("Description", "Array element", value_style()),
    ]
}

fn build_array_descendant_lines(value: &ChannelValue) -> Vec<SelectableLine> {
    let ChannelValue::Array { values, .. } = value else {
        return Vec::new();
    };
    if values.is_empty() {
        return vec![plain_line("No elements in this array.")];
    }

    let mut lines = Vec::new();
    append_array_descendant_lines(values, &mut Vec::new(), &mut lines);
    lines
}

fn append_array_descendant_lines(
    values: &[ChannelValue],
    index_path: &mut Vec<usize>,
    lines: &mut Vec<SelectableLine>,
) {
    for (index, value) in values.iter().enumerate() {
        index_path.push(index);
        match value {
            ChannelValue::Array { values, .. } if values.is_empty() => {
                lines.push(plain_line(&format!(
                    "{} = empty array",
                    format_index_path(index_path)
                )));
            }
            ChannelValue::Array { values, .. } => {
                append_array_descendant_lines(values, index_path, lines);
            }
            ChannelValue::Text(text) => lines.push(plain_line(&format!(
                "{} = {text:?}",
                format_index_path(index_path)
            ))),
            _ => lines.push(plain_line(&format!(
                "{} = {}",
                format_index_path(index_path),
                value.short_display()
            ))),
        }
        index_path.pop();
    }
}

fn format_index_path(index_path: &[usize]) -> String {
    index_path
        .iter()
        .map(|index| format!("[{index}]"))
        .collect()
}

fn build_array_latest_content(
    channel: &ChannelSnapshot,
    value: &ChannelValue,
    index_path: &[usize],
) -> LatestPaneContent {
    if matches!(value, ChannelValue::Array { dimensions: 1, .. }) {
        if let Some(content) = build_numeric_array_latest_content(channel, index_path, true) {
            return content;
        }
    }

    LatestPaneContent::Text(build_array_descendant_lines(value))
}

fn build_array_element_latest_content(
    channel: &ChannelSnapshot,
    value: &ChannelValue,
    index_path: &[usize],
) -> LatestPaneContent {
    if value.numeric_value().is_some() {
        if let Some(content) = build_numeric_array_latest_content(channel, index_path, false) {
            return content;
        }
    }

    LatestPaneContent::Text(build_array_element_latest_lines(value))
}

fn build_array_element_latest_lines(value: &ChannelValue) -> Vec<SelectableLine> {
    match value {
        ChannelValue::Bytes(bytes) => {
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
            vec![
                labeled_value_line(
                    "HEX",
                    &bytes
                        .iter()
                        .map(|byte| format!("{byte:02X}"))
                        .collect::<Vec<_>>()
                        .join(" "),
                    value_style(),
                ),
                labeled_value_line("ASCII", &ascii, value_style()),
            ]
        }
        ChannelValue::Enum { value, name } => vec![
            labeled_value_line("Name", name, value_style()),
            labeled_value_line("Value", &value.to_string(), value_style()),
        ],
        ChannelValue::Text(text) => {
            vec![labeled_value_line(
                "Value",
                &format!("{text:?}"),
                value_style(),
            )]
        }
        ChannelValue::Array { .. } => build_array_descendant_lines(value),
        _ => vec![labeled_value_line(
            "Value",
            &value.to_string(),
            value_style(),
        )],
    }
}

fn channel_value_type_name(value: &ChannelValue) -> &'static str {
    match value {
        ChannelValue::Bool(_) => "bool",
        ChannelValue::Integer(_) => "integer",
        ChannelValue::Float(_) => "float",
        ChannelValue::Text(_) => "text",
        ChannelValue::Bytes(_) => "bytes",
        ChannelValue::Enum { .. } => "enum",
        ChannelValue::Array { .. } => "array",
    }
}

fn build_channel_latest_content(channel: &ChannelSnapshot) -> LatestPaneContent {
    let latest = channel.latest.as_ref();
    let last_received = latest
        .map(|sample| format_duration(sample.observed_at.elapsed()))
        .unwrap_or_else(|| "n/a".to_string());
    let rate = channel
        .rate_hz
        .map(|rate| format!("{rate:.2} Hz"))
        .unwrap_or_else(|| "n/a".to_string());

    match latest.map(|sample| &sample.value) {
        Some(value @ ChannelValue::Array { .. }) => build_array_latest_content(channel, value, &[]),
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
                labeled_value_line(
                    "HEX",
                    &bytes
                        .iter()
                        .map(|byte| format!("{byte:02X}"))
                        .collect::<Vec<_>>()
                        .join(" "),
                    value_style(),
                ),
                labeled_value_line("ASCII", &ascii, value_style()),
            ])
        }
        Some(ChannelValue::Enum { value, name }) if !channel.history.is_empty() => {
            build_numeric_latest_content(
                channel,
                vec![
                    labeled_value_line("Name", name, value_style()),
                    labeled_value_line("Value", &value.to_string(), value_style()),
                    labeled_value_line("Rate", &rate, value_style()),
                    labeled_value_line("Last Received", &last_received, value_style()),
                    labeled_value_line(
                        "Samples",
                        &channel.history.len().to_string(),
                        value_style(),
                    ),
                ],
                NumericGraphKind::StepAfter,
            )
        }
        Some(ChannelValue::Enum { value, name }) => LatestPaneContent::Text(vec![
            labeled_value_line("Name", name, value_style()),
            labeled_value_line("Value", &value.to_string(), value_style()),
            labeled_value_line("Rate", &rate, value_style()),
            labeled_value_line("Last Received", &last_received, value_style()),
        ]),
        Some(ChannelValue::Text(_)) | Some(ChannelValue::Bool(_)) => LatestPaneContent::Text(vec![
            labeled_value_line(
                "Value",
                &latest
                    .map(|sample| sample.value.to_string())
                    .unwrap_or_default(),
                value_style(),
            ),
            labeled_value_line("Rate", &rate, value_style()),
            labeled_value_line("Last Received", &last_received, value_style()),
        ]),
        Some(ChannelValue::Integer(_) | ChannelValue::Float(_)) if !channel.history.is_empty() => {
            build_numeric_latest_content(
                channel,
                vec![
                    labeled_value_line(
                        "Value",
                        &latest
                            .map(|sample| sample.value.to_string())
                            .unwrap_or_default(),
                        value_style(),
                    ),
                    labeled_value_line("Rate", &rate, value_style()),
                    labeled_value_line("Last Received", &last_received, value_style()),
                    labeled_value_line(
                        "Samples",
                        &channel.history.len().to_string(),
                        value_style(),
                    ),
                ],
                NumericGraphKind::Linear,
            )
        }
        Some(ChannelValue::Integer(_) | ChannelValue::Float(_)) => LatestPaneContent::Text(vec![
            labeled_value_line(
                "Value",
                &latest
                    .map(|sample| sample.value.to_string())
                    .unwrap_or_default(),
                value_style(),
            ),
            labeled_value_line("Rate", &rate, value_style()),
            labeled_value_line("Last Received", &last_received, value_style()),
        ]),
        _ => LatestPaneContent::Text(vec![plain_line("No detailed renderer for this value yet.")]),
    }
}

fn build_numeric_latest_content(
    channel: &ChannelSnapshot,
    summary: Vec<SelectableLine>,
    graph_kind: NumericGraphKind,
) -> LatestPaneContent {
    let now_timestamp_unix_ns = current_unix_timestamp_ns();
    let oldest_timestamp_unix_ns = channel
        .history
        .first()
        .map(|point| point.timestamp_unix_ns)
        .unwrap_or(now_timestamp_unix_ns);
    let newest_timestamp_unix_ns = channel
        .history
        .last()
        .map(|point| point.timestamp_unix_ns)
        .unwrap_or(now_timestamp_unix_ns);
    let chart_end_timestamp_unix_ns = newest_timestamp_unix_ns.max(now_timestamp_unix_ns);
    let points = numeric_history_points(&channel.history, graph_kind);
    let (min_y, max_y) = history_bounds(&channel.history);

    LatestPaneContent::Numeric {
        summary,
        series: vec![NumericSeries {
            name: String::new(),
            color: Color::LightCyan,
            points,
        }],
        min_x: oldest_timestamp_unix_ns as f64,
        max_x: chart_end_timestamp_unix_ns as f64,
        x_labels: [
            format_chart_edge_label("old", oldest_timestamp_unix_ns, chart_end_timestamp_unix_ns),
            format_chart_edge_label(
                "now",
                chart_end_timestamp_unix_ns,
                chart_end_timestamp_unix_ns,
            ),
        ],
        min_y,
        max_y,
    }
}

fn build_numeric_array_latest_content(
    channel: &ChannelSnapshot,
    selected_path: &[usize],
    include_children: bool,
) -> Option<LatestPaneContent> {
    let sample = channel.latest.as_ref()?;
    let ChannelValue::Array { .. } = &sample.value else {
        return None;
    };
    let current_values = sample.value.numeric_array_values()?;
    if channel.array_history.is_empty() {
        return None;
    }

    let index_paths = if include_children {
        current_values
            .iter()
            .filter(|value| {
                value.index_path.starts_with(selected_path)
                    && value.index_path.len() == selected_path.len() + 1
            })
            .map(|value| value.index_path.clone())
            .collect()
    } else if current_values
        .iter()
        .any(|value| value.index_path == selected_path)
    {
        vec![selected_path.to_vec()]
    } else {
        return None;
    };
    if index_paths.is_empty() {
        return None;
    }

    let series = index_paths
        .iter()
        .map(|index_path| NumericSeries {
            name: if include_children {
                format_index_path(&index_path[selected_path.len()..])
            } else {
                String::new()
            },
            color: if include_children {
                numeric_series_color(numeric_series_color_index(
                    &index_path[selected_path.len()..],
                ))
            } else {
                Color::LightCyan
            },
            points: channel
                .array_history
                .iter()
                .filter_map(|point| {
                    point
                        .values
                        .iter()
                        .find(|value| value.index_path == *index_path)
                        .map(|value| (point.timestamp_unix_ns as f64, value.value))
                })
                .collect(),
        })
        .filter(|series| !series.points.is_empty())
        .collect::<Vec<_>>();
    if series.is_empty() {
        return None;
    }

    let oldest_timestamp_unix_ns = channel
        .array_history
        .iter()
        .find(|point| {
            index_paths.iter().any(|index_path| {
                point
                    .values
                    .iter()
                    .any(|value| value.index_path == *index_path)
            })
        })
        .map(|point| point.timestamp_unix_ns)?;
    let newest_timestamp_unix_ns = channel
        .array_history
        .iter()
        .rev()
        .find(|point| {
            index_paths.iter().any(|index_path| {
                point
                    .values
                    .iter()
                    .any(|value| value.index_path == *index_path)
            })
        })
        .map(|point| point.timestamp_unix_ns)?;
    let now_timestamp_unix_ns = current_unix_timestamp_ns();
    let chart_end_timestamp_unix_ns = newest_timestamp_unix_ns.max(now_timestamp_unix_ns);
    let (min_y, max_y) = numeric_series_bounds(&series);
    let rate = channel
        .rate_hz
        .map(|rate| format!("{rate:.2} Hz"))
        .unwrap_or_else(|| "n/a".to_string());
    let last_received = format_duration(sample.observed_at.elapsed());
    let mut summary = Vec::new();
    if include_children {
        summary.push(labeled_value_line(
            "Series",
            &index_paths.len().to_string(),
            value_style(),
        ));
    } else {
        let value = array_value_at_path(&sample.value, selected_path)?;
        summary.push(labeled_value_line(
            "Value",
            &value.to_string(),
            value_style(),
        ));
    }
    summary.extend([
        labeled_value_line("Rate", &rate, value_style()),
        labeled_value_line("Last Received", &last_received, value_style()),
        labeled_value_line(
            "Samples",
            &channel.array_history.len().to_string(),
            value_style(),
        ),
    ]);

    Some(LatestPaneContent::Numeric {
        summary,
        series,
        min_x: oldest_timestamp_unix_ns as f64,
        max_x: chart_end_timestamp_unix_ns as f64,
        x_labels: [
            format_chart_edge_label("old", oldest_timestamp_unix_ns, chart_end_timestamp_unix_ns),
            format_chart_edge_label(
                "now",
                chart_end_timestamp_unix_ns,
                chart_end_timestamp_unix_ns,
            ),
        ],
        min_y,
        max_y,
    })
}

fn array_value_at_path<'a>(
    value: &'a ChannelValue,
    index_path: &[usize],
) -> Option<&'a ChannelValue> {
    let mut value = value;
    for index in index_path {
        let ChannelValue::Array { values, .. } = value else {
            return None;
        };
        value = values.get(*index)?;
    }
    Some(value)
}

fn numeric_series_color_index(index_path: &[usize]) -> usize {
    index_path.iter().fold(0usize, |hash, index| {
        hash.wrapping_mul(31).wrapping_add(*index)
    })
}

fn numeric_series_color(index: usize) -> Color {
    const COLORS: [Color; 12] = [
        Color::LightCyan,
        Color::LightMagenta,
        Color::LightYellow,
        Color::LightGreen,
        Color::LightBlue,
        Color::LightRed,
        Color::Cyan,
        Color::Magenta,
        Color::Yellow,
        Color::Green,
        Color::Blue,
        Color::Red,
    ];
    COLORS[index % COLORS.len()]
}

fn numeric_series_bounds(series: &[NumericSeries]) -> (f64, f64) {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for (_, value) in series.iter().flat_map(|series| series.points.iter()) {
        min = min.min(*value);
        max = max.max(*value);
    }

    padded_bounds(min, max)
}

fn numeric_history_points(
    history: &[NumericPoint],
    graph_kind: NumericGraphKind,
) -> Vec<(f64, f64)> {
    match graph_kind {
        NumericGraphKind::Linear => history
            .iter()
            .map(|point| (point.timestamp_unix_ns as f64, point.value))
            .collect(),
        NumericGraphKind::StepAfter => {
            let mut points = Vec::with_capacity(history.len().saturating_mul(2).saturating_sub(1));
            let Some(first) = history.first() else {
                return points;
            };
            points.push((first.timestamp_unix_ns as f64, first.value));

            for pair in history.windows(2) {
                let previous = pair[0];
                let next = pair[1];
                let next_timestamp = next.timestamp_unix_ns as f64;
                points.push((next_timestamp, previous.value));
                points.push((next_timestamp, next.value));
            }

            points
        }
    }
}

fn build_namespace_variable_lines(
    path: &str,
    channels: &[&ChannelSnapshot],
) -> Vec<SelectableLine> {
    if channels.is_empty() {
        return vec![plain_line("No variables in this namespace.")];
    }

    let mut sorted = channels.to_vec();
    sorted.sort_by_key(|channel| channel_display_path(channel));
    sorted
        .into_iter()
        .map(|channel| {
            let relative = relative_channel_path(path, &channel_display_path(channel));
            let marker = if channel.is_stale { "stale" } else { "live" };
            let value = channel
                .latest
                .as_ref()
                .map(|sample| sample.value.short_display())
                .unwrap_or_else(|| "waiting".to_string());
            let unit = channel
                .descriptor
                .unit
                .as_ref()
                .map(|unit| format!(" {unit}"))
                .unwrap_or_default();
            SelectableLine {
                raw: format!("{relative} [{marker}] = {value}{unit}"),
                rendered: Line::from(vec![
                    Span::styled(relative, primary_style()),
                    Span::raw(" "),
                    Span::styled(format!("[{marker}]"), status_style(channel.is_stale)),
                    Span::raw(" = "),
                    Span::styled(value, value_style()),
                    Span::styled(unit, muted_style()),
                ]),
            }
        })
        .collect()
}

fn latest_selectable_lines(content: &LatestPaneContent) -> Vec<SelectableLine> {
    match content {
        LatestPaneContent::Text(lines) => lines.clone(),
        LatestPaneContent::Numeric { summary, .. } => summary.clone(),
    }
}

fn relative_channel_path(namespace_path: &str, full_path: &str) -> String {
    let prefix = format!("{namespace_path}.");
    full_path
        .strip_prefix(&prefix)
        .unwrap_or(full_path)
        .to_string()
}

fn row_key(row: &TreeRow<'_>) -> RowKey {
    match &row.kind {
        TreeRowKind::Namespace { path, .. } => RowKey::Namespace(path.clone()),
        TreeRowKind::Channel { channel } => RowKey::Channel(channel_tree_path(channel)),
        TreeRowKind::Array {
            channel,
            index_path,
            ..
        } => RowKey::Array(array_tree_path(channel, index_path)),
        TreeRowKind::ArrayElement {
            channel,
            index_path,
            ..
        } => RowKey::ArrayElement(array_tree_path(channel, index_path)),
    }
}

fn row_matches_key(row: &TreeRow<'_>, key: &RowKey) -> bool {
    match (&row.kind, key) {
        (TreeRowKind::Namespace { path, .. }, RowKey::Namespace(target)) => path == target,
        (TreeRowKind::Channel { channel }, RowKey::Channel(target)) => {
            channel_tree_path(channel) == *target
        }
        (
            TreeRowKind::Array {
                channel,
                index_path,
                ..
            },
            RowKey::Array(target),
        ) => array_tree_path(channel, index_path) == *target,
        (
            TreeRowKind::ArrayElement {
                channel,
                index_path,
                ..
            },
            RowKey::ArrayElement(target),
        ) => array_tree_path(channel, index_path) == *target,
        _ => false,
    }
}

fn channel_tree_path(channel: &ChannelSnapshot) -> String {
    format!("{}.{}", channel.plugin_id, channel.descriptor.path)
}

fn channel_display_path(channel: &ChannelSnapshot) -> String {
    channel.descriptor.path.clone()
}

fn array_tree_path(channel: &ChannelSnapshot, index_path: &[usize]) -> String {
    let mut path = channel_tree_path(channel);
    for index in index_path {
        path.push_str(&format!("[{index}]"));
    }
    path
}

fn array_display_path(channel: &ChannelSnapshot, index_path: &[usize]) -> String {
    let mut path = channel.descriptor.path.clone();
    for index in index_path {
        path.push_str(&format!("[{index}]"));
    }
    path
}

fn plugin_ids(snapshot: &StoreSnapshot) -> Vec<String> {
    let mut plugin_ids = BTreeSet::new();
    plugin_ids.extend(
        snapshot
            .plugins
            .iter()
            .map(|plugin| plugin.plugin_id.clone()),
    );
    plugin_ids.extend(
        snapshot
            .channels
            .iter()
            .map(|channel| channel.plugin_id.clone()),
    );
    plugin_ids.into_iter().collect()
}

fn ancestor_row_keys(key: &RowKey) -> Vec<RowKey> {
    match key {
        RowKey::Array(path) | RowKey::ArrayElement(path) => {
            let mut path = path.clone();
            let mut fallbacks = Vec::new();
            while let Some(index_start) = path.rfind('[') {
                path.truncate(index_start);
                fallbacks.push(RowKey::Array(path.clone()));
            }
            fallbacks
        }
        RowKey::Namespace(path) | RowKey::Channel(path) => {
            let parts = path.split('.').collect::<Vec<_>>();
            let namespace_end = match key {
                RowKey::Namespace(_) => parts.len(),
                RowKey::Channel(_) => parts.len().saturating_sub(1),
                RowKey::Array(_) | RowKey::ArrayElement(_) => unreachable!(),
            };

            let mut fallbacks = Vec::new();
            for depth in (1..namespace_end).rev() {
                fallbacks.push(RowKey::Namespace(parts[..depth].join(".")));
            }
            fallbacks
        }
    }
}

fn extract_copy_from_selectable_lines(
    lines: &[SelectableLine],
    cursor: &TextCursor,
) -> Option<String> {
    if lines.is_empty() {
        return None;
    }

    let cursor = clamped_selectable_cursor(cursor, lines);
    Some(lines[cursor.point.line].raw.clone())
}

fn clamp_selectable_cursor(cursor: &mut TextCursor, lines: &[SelectableLine]) {
    *cursor = clamped_selectable_cursor(cursor, lines);
}

fn clamped_selectable_cursor(cursor: &TextCursor, lines: &[SelectableLine]) -> TextCursor {
    if lines.is_empty() {
        return TextCursor::default();
    }

    let mut clamped = cursor.clone();
    clamp_selectable_point(&mut clamped.point, lines);
    clamped
}

fn clamp_selectable_point(point: &mut TextPoint, lines: &[SelectableLine]) {
    let max_line = lines.len().saturating_sub(1);
    point.line = point.line.min(max_line);
    let max_col = lines[point.line].raw.chars().count();
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

fn rect_contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x && column < area.x + area.width && row >= area.y && row < area.y + area.height
}

fn detail_row(
    left_label: &str,
    left_value: &str,
    right_label: &str,
    right_value: &str,
) -> SelectableLine {
    let left_cell = format!("{left_label}: {left_value}");
    if right_label.is_empty() {
        labeled_value_line(left_label, left_value, value_style())
    } else {
        let right_cell = format!("{right_label}: {right_value}");
        let padding = 34usize.saturating_sub(left_cell.chars().count());
        SelectableLine {
            raw: format!("{left_cell:<32}  {right_cell}"),
            rendered: Line::from(vec![
                Span::styled(format!("{left_label}: "), label_style()),
                styled_value_span(left_label, left_value),
                Span::raw(" ".repeat(padding)),
                Span::styled(format!("{right_label}: "), label_style()),
                styled_value_span(right_label, right_value),
            ]),
        }
    }
}

fn plain_line(text: &str) -> SelectableLine {
    SelectableLine {
        raw: text.to_string(),
        rendered: Line::from(Span::raw(text.to_string())),
    }
}

fn labeled_value_line(label: &str, value: &str, value_style: Style) -> SelectableLine {
    SelectableLine {
        raw: format!("{label}: {value}"),
        rendered: Line::from(vec![
            Span::styled(format!("{label}: "), label_style()),
            Span::styled(value.to_string(), value_style),
        ]),
    }
}

fn label_style() -> Style {
    Style::default().fg(Color::White)
}

fn primary_style() -> Style {
    Style::default().fg(Color::Cyan)
}

fn value_style() -> Style {
    Style::default().fg(Color::Gray)
}

fn muted_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

fn status_style(is_stale: bool) -> Style {
    Style::default().fg(if is_stale {
        Color::Yellow
    } else {
        Color::Green
    })
}

fn styled_value_span(label: &str, value: &str) -> Span<'static> {
    match label {
        "Type" => Span::styled(
            value.to_string(),
            if value == "namespace" {
                Style::default().fg(Color::Yellow)
            } else {
                primary_style()
            },
        ),
        "Path" => Span::styled(value.to_string(), primary_style()),
        "Value" => Span::styled(value.to_string(), value_style()),
        "Unit" | "Units" | "Rate" | "Last Received" | "Updates" | "Channels" | "Total Channels"
        | "Direct" | "Children" | "Child Namespaces" | "Samples" | "Notes" | "Description" => {
            Span::styled(value.to_string(), value_style())
        }
        "State" => Span::styled(value.to_string(), value_style()),
        _ => Span::styled(value.to_string(), value_style()),
    }
}

fn filter_bar_rect(area: Rect, input_width: u16) -> Rect {
    let width = (input_width + 4).clamp(24, area.width.saturating_sub(2).max(24));
    let height = 3.min(area.height);
    let y = area.bottom().saturating_sub(height + 1);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y,
        width: width.min(area.width),
        height,
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

fn current_unix_timestamp_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or_default()
}

fn format_chart_edge_label(
    label: &str,
    timestamp_unix_ns: u64,
    end_timestamp_unix_ns: u64,
) -> String {
    if label == "now" {
        return "now".to_string();
    }

    format_elapsed_ns(end_timestamp_unix_ns.saturating_sub(timestamp_unix_ns))
}

fn format_elapsed_ns(duration_ns: u64) -> String {
    let duration = Duration::from_nanos(duration_ns);
    if duration.as_secs() > 0 {
        format!("{:.1}s ago", duration.as_secs_f64())
    } else {
        format!("{}ms ago", duration.as_millis())
    }
}

fn history_bounds(history: &[NumericPoint]) -> (f64, f64) {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for point in history {
        min = min.min(point.value);
        max = max.max(point.value);
    }

    padded_bounds(min, max)
}

fn padded_bounds(min: f64, max: f64) -> (f64, f64) {
    if !min.is_finite() || !max.is_finite() {
        return (0.0, 1.0);
    }

    if (max - min).abs() < f64::EPSILON {
        return (min - 1.0, max + 1.0);
    }

    let padding = (max - min) * 0.1;
    (min - padding, max + padding)
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend, style::Color};

    use super::{
        FocusPane, LatestPaneContent, UiState, build_channel_latest_content,
        format_chart_edge_label, plugin_ids,
    };
    use prismo_core::{
        ArrayElementType, ChannelDescriptor, ChannelSample, ChannelSnapshot, ChannelValue,
        NumericArrayPoint, NumericArrayValue, NumericPoint, PluginHealth, PluginRuntimeState,
        PluginSnapshot, StoreSnapshot,
    };

    #[test]
    fn integer_channels_render_numeric_latest_content() {
        let channel = ChannelSnapshot {
            plugin_id: "example".to_string(),
            descriptor: ChannelDescriptor {
                path: "counter.value".to_string(),
                display_name: "Counter".to_string(),
                unit: None,
                description: "Integer counter".to_string(),
            },
            latest: Some(ChannelSample {
                path: "counter.value".to_string(),
                value: ChannelValue::Integer(42),
                observed_at: Instant::now(),
                received_timestamp_unix_ns: 3_000_000_000,
                source_timestamp_unix_ns: 42,
                sequence: 1,
            }),
            history: vec![
                NumericPoint {
                    timestamp_unix_ns: 1_000_000_000,
                    value: 40.0,
                },
                NumericPoint {
                    timestamp_unix_ns: 2_000_000_000,
                    value: 41.0,
                },
                NumericPoint {
                    timestamp_unix_ns: 3_000_000_000,
                    value: 42.0,
                },
            ],
            array_history: Vec::new(),
            update_count: 3,
            rate_hz: Some(2.0),
            is_stale: false,
        };

        match build_channel_latest_content(&channel) {
            LatestPaneContent::Numeric {
                summary,
                series,
                min_x,
                max_x,
                x_labels,
                min_y,
                max_y,
            } => {
                assert_eq!(summary.len(), 4);
                assert_eq!(series.len(), 1);
                assert_eq!(
                    series[0].points,
                    vec![
                        (1_000_000_000_f64, 40.0),
                        (2_000_000_000_f64, 41.0),
                        (3_000_000_000_f64, 42.0),
                    ]
                );
                assert_eq!(min_x, 1_000_000_000_f64);
                assert!(max_x >= 3_000_000_000_f64);
                assert!(x_labels[0].ends_with(" ago"));
                assert_eq!(x_labels[1], "now");
                assert!(min_y < 40.0);
                assert!(max_y > 42.0);
            }
            LatestPaneContent::Text(_) => panic!("expected numeric content for integer channel"),
        }
    }

    #[test]
    fn enum_channels_graph_discriminants() {
        let channel = ChannelSnapshot {
            plugin_id: "example".to_string(),
            descriptor: ChannelDescriptor {
                path: "guidance.mode".to_string(),
                display_name: "Mode".to_string(),
                unit: None,
                description: "Guidance mode".to_string(),
            },
            latest: Some(ChannelSample {
                path: "guidance.mode".to_string(),
                value: ChannelValue::Enum {
                    value: 2,
                    name: "SAFE".to_string(),
                },
                observed_at: Instant::now(),
                received_timestamp_unix_ns: 3_000_000_000,
                source_timestamp_unix_ns: 42,
                sequence: 1,
            }),
            history: vec![
                NumericPoint {
                    timestamp_unix_ns: 1_000_000_000,
                    value: 0.0,
                },
                NumericPoint {
                    timestamp_unix_ns: 2_000_000_000,
                    value: 1.0,
                },
                NumericPoint {
                    timestamp_unix_ns: 3_000_000_000,
                    value: 2.0,
                },
            ],
            array_history: Vec::new(),
            update_count: 3,
            rate_hz: Some(2.0),
            is_stale: false,
        };

        match build_channel_latest_content(&channel) {
            LatestPaneContent::Numeric {
                summary,
                series,
                min_y,
                max_y,
                ..
            } => {
                assert_eq!(summary.len(), 5);
                assert_eq!(summary[0].raw, "Name: SAFE");
                assert_eq!(summary[1].raw, "Value: 2");
                assert_eq!(summary[2].raw, "Rate: 2.00 Hz");
                assert!(summary[3].raw.starts_with("Last Received: "));
                assert_eq!(summary[4].raw, "Samples: 3");
                assert_eq!(series.len(), 1);
                assert_eq!(
                    series[0].points,
                    vec![
                        (1_000_000_000_f64, 0.0),
                        (2_000_000_000_f64, 0.0),
                        (2_000_000_000_f64, 1.0),
                        (3_000_000_000_f64, 1.0),
                        (3_000_000_000_f64, 2.0),
                    ]
                );
                assert!(min_y < 0.0);
                assert!(max_y > 2.0);
            }
            LatestPaneContent::Text(_) => {
                panic!("expected numeric content for enum channel")
            }
        }
    }

    #[test]
    fn arrays_expand_and_collapse_like_namespaces() {
        let snapshot = StoreSnapshot {
            plugins: vec![plugin("example")],
            channels: vec![array_channel("example", "matrix")],
            ..StoreSnapshot::default()
        };
        let mut ui = UiState::new();

        let rows = ui.tree_rows(&snapshot);
        assert_eq!(rows.len(), 7);
        assert!(matches!(
            rows[0].kind,
            super::TreeRowKind::Array {
                ref index_path,
                collapsed: false,
                ..
            } if index_path.is_empty()
        ));
        assert!(matches!(
            rows[1].kind,
            super::TreeRowKind::Array {
                ref index_path,
                collapsed: false,
                ..
            } if index_path == &[0]
        ));
        assert!(matches!(
            rows[2].kind,
            super::TreeRowKind::ArrayElement {
                ref index_path,
                ..
            } if index_path == &[0, 0]
        ));

        assert!(ui.toggle_selected_namespace(&snapshot));
        let rows = ui.tree_rows(&snapshot);
        assert_eq!(rows.len(), 1);
        assert!(matches!(
            rows[0].kind,
            super::TreeRowKind::Array {
                collapsed: true,
                ..
            }
        ));
    }

    #[test]
    fn nested_array_elements_are_selectable_values() {
        let snapshot = StoreSnapshot {
            plugins: vec![plugin("example")],
            channels: vec![array_channel("example", "matrix")],
            ..StoreSnapshot::default()
        };
        let mut ui = UiState::new();
        ui.selected = 3;

        let selected = ui
            .selected_value(&snapshot)
            .expect("select nested array element");
        assert_eq!(selected.path, "matrix[0][1]");
        assert_eq!(selected.value.to_string(), "2");
    }

    #[test]
    fn nested_numeric_array_element_plots_its_path_history() {
        let snapshot = StoreSnapshot {
            plugins: vec![plugin("example")],
            channels: vec![array_channel("example", "matrix")],
            ..StoreSnapshot::default()
        };
        let ui = UiState::new();
        let rows = ui.tree_rows(&snapshot);
        let content = super::pane_content_for_row(&rows[3], false, "");

        match content.latest_content {
            LatestPaneContent::Numeric {
                summary, series, ..
            } => {
                assert_eq!(summary[0].raw, "Value: 2");
                assert_eq!(series.len(), 1);
                assert!(series[0].name.is_empty());
                assert_eq!(series[0].color, Color::LightCyan);
                assert_eq!(
                    series[0].points,
                    vec![(1_000_000_000_f64, 1.5), (2_000_000_000_f64, 2.0),]
                );
            }
            LatestPaneContent::Text(_) => panic!("expected nested numeric element chart"),
        }
    }

    #[test]
    fn nested_one_dimensional_array_plots_its_children_as_colored_series() {
        let snapshot = StoreSnapshot {
            plugins: vec![plugin("example")],
            channels: vec![array_channel("example", "matrix")],
            ..StoreSnapshot::default()
        };
        let ui = UiState::new();
        let rows = ui.tree_rows(&snapshot);
        let content = super::pane_content_for_row(&rows[1], false, "");

        match content.latest_content {
            LatestPaneContent::Numeric {
                summary, series, ..
            } => {
                assert_eq!(summary[0].raw, "Series: 2");
                assert_eq!(series.len(), 2);
                assert_eq!(series[0].name, "[0]");
                assert_eq!(series[1].name, "[1]");
                assert_ne!(series[0].color, series[1].color);
                assert_eq!(
                    series[0].points,
                    vec![(1_000_000_000_f64, 0.5), (2_000_000_000_f64, 1.0)]
                );
                assert_eq!(
                    series[1].points,
                    vec![(1_000_000_000_f64, 1.5), (2_000_000_000_f64, 2.0)]
                );
            }
            LatestPaneContent::Text(_) => panic!("expected nested row numeric chart"),
        }
    }

    #[test]
    fn nested_array_panes_show_all_descendant_values() {
        let snapshot = StoreSnapshot {
            plugins: vec![plugin("example")],
            channels: vec![array_channel("example", "matrix")],
            ..StoreSnapshot::default()
        };
        let ui = UiState::new();
        let rows = ui.tree_rows(&snapshot);
        let content = super::pane_content_for_row(&rows[0], false, "");

        assert_eq!(content.latest_title, "Elements");
        assert_eq!(content.detail_lines[0].raw, "Path: matrix");
        assert_eq!(content.detail_lines[1].raw, "Type: array (integer)");
        assert_eq!(content.detail_lines[2].raw, "Shape: 2x2");
        match content.latest_content {
            LatestPaneContent::Text(lines) => {
                assert_eq!(
                    lines
                        .iter()
                        .map(|line| line.raw.as_str())
                        .collect::<Vec<_>>(),
                    vec!["[0][0] = 1", "[0][1] = 2", "[1][0] = 3", "[1][1] = 4"]
                );
            }
            LatestPaneContent::Numeric { .. } => panic!("nested arrays should not be plotted"),
        }
    }

    #[test]
    fn flat_numeric_array_plots_each_element_as_a_colored_series() {
        let snapshot = StoreSnapshot {
            plugins: vec![plugin("example")],
            channels: vec![flat_numeric_array_channel("example", "sensors")],
            ..StoreSnapshot::default()
        };
        let ui = UiState::new();
        let rows = ui.tree_rows(&snapshot);
        let content = super::pane_content_for_row(&rows[0], false, "");

        match content.latest_content {
            LatestPaneContent::Numeric {
                summary, series, ..
            } => {
                assert_eq!(summary[0].raw, "Series: 2");
                assert_eq!(series.len(), 2);
                assert_eq!(series[0].name, "[0]");
                assert_eq!(series[1].name, "[1]");
                assert_ne!(series[0].color, series[1].color);
                assert_eq!(
                    series[0].points,
                    vec![(1_000_000_000_f64, 1.0), (2_000_000_000_f64, 1.5),]
                );
                assert_eq!(
                    series[1].points,
                    vec![(1_000_000_000_f64, 2.0), (2_000_000_000_f64, 2.5),]
                );
            }
            LatestPaneContent::Text(_) => panic!("expected numeric array chart"),
        }
    }

    #[test]
    fn numeric_array_element_plots_its_series() {
        let snapshot = StoreSnapshot {
            plugins: vec![plugin("example")],
            channels: vec![flat_numeric_array_channel("example", "sensors")],
            ..StoreSnapshot::default()
        };
        let ui = UiState::new();
        let rows = ui.tree_rows(&snapshot);
        let content = super::pane_content_for_row(&rows[2], false, "");

        match content.latest_content {
            LatestPaneContent::Numeric {
                summary, series, ..
            } => {
                assert_eq!(summary[0].raw, "Value: 2.500");
                assert_eq!(series.len(), 1);
                assert!(series[0].name.is_empty());
                assert_eq!(series[0].color, Color::LightCyan);
                assert_eq!(
                    series[0].points,
                    vec![(1_000_000_000_f64, 2.0), (2_000_000_000_f64, 2.5),]
                );
            }
            LatestPaneContent::Text(_) => panic!("expected numeric element chart"),
        }
    }

    #[test]
    fn ragged_array_shape_is_explicit() {
        let mut channel = array_channel("example", "matrix");
        let ChannelValue::Array { values, .. } =
            &mut channel.latest.as_mut().expect("array sample").value
        else {
            panic!("expected root array");
        };
        let ChannelValue::Array { values, .. } = &mut values[1] else {
            panic!("expected nested array");
        };
        values.pop();

        let value = &channel.latest.as_ref().expect("array sample").value;
        assert_eq!(super::array_shape(value), "2xragged");
    }

    #[test]
    fn chart_edge_labels_show_elapsed_time() {
        assert_eq!(
            format_chart_edge_label("old", 1_000_000_000, 3_500_000_000),
            "2.5s ago"
        );
        assert_eq!(
            format_chart_edge_label("now", 3_500_000_000, 3_500_000_000),
            "now"
        );
    }

    #[test]
    fn plugin_ids_include_running_plugins_and_channel_only_plugins() {
        let snapshot = StoreSnapshot {
            plugins: vec![plugin("beta"), plugin("alpha")],
            channels: vec![
                channel("gamma", "orphan.value"),
                channel("alpha", "cpu.load"),
            ],
            ..StoreSnapshot::default()
        };

        assert_eq!(plugin_ids(&snapshot), vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn filtered_channels_only_include_selected_plugin() {
        let snapshot = StoreSnapshot {
            plugins: vec![plugin("alpha"), plugin("beta")],
            channels: vec![
                channel("alpha", "cpu.load"),
                channel("beta", "cpu.load"),
                channel("beta", "memory.used"),
            ],
            ..StoreSnapshot::default()
        };
        let mut ui = UiState::new();

        assert_eq!(
            filtered_channel_keys(&ui, &snapshot),
            vec!["alpha:cpu.load"]
        );

        ui.select_next_plugin();

        assert_eq!(
            filtered_channel_keys(&ui, &snapshot),
            vec!["beta:cpu.load", "beta:memory.used"]
        );
    }

    #[test]
    fn channel_filter_is_applied_within_selected_plugin() {
        let snapshot = StoreSnapshot {
            plugins: vec![plugin("alpha"), plugin("beta")],
            channels: vec![
                channel("alpha", "cpu.load"),
                channel("alpha", "memory.used"),
                channel("beta", "memory.used"),
            ],
            ..StoreSnapshot::default()
        };
        let mut ui = UiState::new();
        ui.filter_input = "memory".to_string();

        assert_eq!(
            filtered_channel_keys(&ui, &snapshot),
            vec!["alpha:memory.used"]
        );
    }

    #[test]
    fn channel_stats_exclude_namespace_rows_and_ignore_collapsed_state() {
        let snapshot = StoreSnapshot {
            plugins: vec![plugin("alpha")],
            channels: vec![
                channel("alpha", "power.voltage"),
                channel("alpha", "power.current"),
            ],
            ..StoreSnapshot::default()
        };
        let mut ui = UiState::new();
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("create test terminal");

        terminal
            .draw(|frame| super::draw(frame, &snapshot, &mut ui))
            .expect("draw expanded channel tree");
        assert!(rendered_terminal_text(&terminal).contains("channels:2"));

        ui.collapsed_namespaces.insert("power".to_string());
        terminal
            .draw(|frame| super::draw(frame, &snapshot, &mut ui))
            .expect("draw collapsed channel tree");
        assert!(rendered_terminal_text(&terminal).contains("channels:2"));
    }

    #[test]
    fn channel_tab_switching_resets_selection_and_changes_visible_channels() {
        let snapshot = StoreSnapshot {
            plugins: vec![plugin("alpha"), plugin("beta")],
            channels: vec![channel("alpha", "cpu.load"), channel("beta", "memory.used")],
            ..StoreSnapshot::default()
        };
        let mut ui = UiState::new();
        ui.selected = 3;
        ui.channel_scroll_offset = 2;

        ui.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));

        assert_eq!(ui.selected_plugin, 1);
        assert_eq!(ui.selected, 0);
        assert_eq!(ui.channel_scroll_offset, 0);
        assert_eq!(
            filtered_channel_keys(&ui, &snapshot),
            vec!["beta:memory.used"]
        );
    }

    #[test]
    fn non_channel_focus_does_not_switch_plugin_tabs() {
        let snapshot = StoreSnapshot {
            plugins: vec![plugin("alpha"), plugin("beta")],
            channels: vec![channel("alpha", "cpu.load"), channel("beta", "memory.used")],
            ..StoreSnapshot::default()
        };
        let mut ui = UiState::new();
        ui.focus = FocusPane::Details;

        ui.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));

        assert_eq!(ui.selected_plugin, 0);
        assert_eq!(
            filtered_channel_keys(&ui, &snapshot),
            vec!["alpha:cpu.load"]
        );
    }

    fn filtered_channel_keys(ui: &UiState, snapshot: &StoreSnapshot) -> Vec<String> {
        ui.filtered_channels(snapshot)
            .into_iter()
            .map(|channel| format!("{}:{}", channel.plugin_id, channel.descriptor.path))
            .collect()
    }

    fn rendered_terminal_text(terminal: &Terminal<TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    fn channel(plugin_id: &str, path: &str) -> ChannelSnapshot {
        ChannelSnapshot {
            plugin_id: plugin_id.to_string(),
            descriptor: ChannelDescriptor {
                path: path.to_string(),
                display_name: path.to_string(),
                unit: None,
                description: format!("{path} description"),
            },
            latest: None,
            history: Vec::new(),
            array_history: Vec::new(),
            update_count: 0,
            rate_hz: None,
            is_stale: false,
        }
    }

    fn array_channel(plugin_id: &str, path: &str) -> ChannelSnapshot {
        ChannelSnapshot {
            plugin_id: plugin_id.to_string(),
            descriptor: ChannelDescriptor {
                path: path.to_string(),
                display_name: "Matrix".to_string(),
                unit: None,
                description: "Ragged integer matrix".to_string(),
            },
            latest: Some(ChannelSample {
                path: path.to_string(),
                value: ChannelValue::Array {
                    leaf_type: ArrayElementType::Integer,
                    dimensions: 2,
                    values: vec![
                        ChannelValue::Array {
                            leaf_type: ArrayElementType::Integer,
                            dimensions: 1,
                            values: vec![ChannelValue::Integer(1), ChannelValue::Integer(2)],
                        },
                        ChannelValue::Array {
                            leaf_type: ArrayElementType::Integer,
                            dimensions: 1,
                            values: vec![ChannelValue::Integer(3), ChannelValue::Integer(4)],
                        },
                    ],
                },
                observed_at: Instant::now(),
                received_timestamp_unix_ns: 3_000_000_000,
                source_timestamp_unix_ns: 42,
                sequence: 1,
            }),
            history: Vec::new(),
            array_history: vec![
                NumericArrayPoint {
                    timestamp_unix_ns: 1_000_000_000,
                    values: vec![
                        NumericArrayValue {
                            index_path: vec![0, 0],
                            value: 0.5,
                        },
                        NumericArrayValue {
                            index_path: vec![0, 1],
                            value: 1.5,
                        },
                        NumericArrayValue {
                            index_path: vec![1, 0],
                            value: 2.5,
                        },
                        NumericArrayValue {
                            index_path: vec![1, 1],
                            value: 3.5,
                        },
                    ],
                },
                NumericArrayPoint {
                    timestamp_unix_ns: 2_000_000_000,
                    values: vec![
                        NumericArrayValue {
                            index_path: vec![0, 0],
                            value: 1.0,
                        },
                        NumericArrayValue {
                            index_path: vec![0, 1],
                            value: 2.0,
                        },
                        NumericArrayValue {
                            index_path: vec![1, 0],
                            value: 3.0,
                        },
                        NumericArrayValue {
                            index_path: vec![1, 1],
                            value: 4.0,
                        },
                    ],
                },
            ],
            update_count: 1,
            rate_hz: Some(2.0),
            is_stale: false,
        }
    }

    fn flat_numeric_array_channel(plugin_id: &str, path: &str) -> ChannelSnapshot {
        ChannelSnapshot {
            plugin_id: plugin_id.to_string(),
            descriptor: ChannelDescriptor {
                path: path.to_string(),
                display_name: "Sensors".to_string(),
                unit: None,
                description: "Sensor values".to_string(),
            },
            latest: Some(ChannelSample {
                path: path.to_string(),
                value: ChannelValue::Array {
                    leaf_type: ArrayElementType::Float,
                    dimensions: 1,
                    values: vec![ChannelValue::Float(1.5), ChannelValue::Float(2.5)],
                },
                observed_at: Instant::now(),
                received_timestamp_unix_ns: 2_000_000_000,
                source_timestamp_unix_ns: 2_000_000_000,
                sequence: 2,
            }),
            history: Vec::new(),
            array_history: vec![
                NumericArrayPoint {
                    timestamp_unix_ns: 1_000_000_000,
                    values: vec![
                        NumericArrayValue {
                            index_path: vec![0],
                            value: 1.0,
                        },
                        NumericArrayValue {
                            index_path: vec![1],
                            value: 2.0,
                        },
                    ],
                },
                NumericArrayPoint {
                    timestamp_unix_ns: 2_000_000_000,
                    values: vec![
                        NumericArrayValue {
                            index_path: vec![0],
                            value: 1.5,
                        },
                        NumericArrayValue {
                            index_path: vec![1],
                            value: 2.5,
                        },
                    ],
                },
            ],
            update_count: 2,
            rate_hz: Some(2.0),
            is_stale: false,
        }
    }

    fn plugin(plugin_id: &str) -> PluginSnapshot {
        PluginSnapshot {
            plugin_id: plugin_id.to_string(),
            state: PluginRuntimeState::Running,
            restart_count: 0,
            message: None,
            health: PluginHealth::default(),
        }
    }
}
