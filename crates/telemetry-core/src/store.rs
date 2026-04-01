use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};

use crate::{ChannelDescriptor, ChannelSample, PluginHealth, PluginSnapshot, TelemetryUpdate};

const HISTORY_LIMIT: usize = 64;
const STALE_AFTER: Duration = Duration::from_secs(3);

#[derive(Clone, Debug)]
pub struct ChannelSnapshot {
    pub descriptor: ChannelDescriptor,
    pub latest: Option<ChannelSample>,
    pub history: Vec<f64>,
    pub update_count: u64,
    pub is_stale: bool,
}

#[derive(Clone, Debug, Default)]
pub struct StoreSnapshot {
    pub channels: Vec<ChannelSnapshot>,
    pub plugins: Vec<PluginSnapshot>,
    pub total_updates: u64,
    pub dropped_updates: u64,
}

#[derive(Clone, Debug)]
struct ChannelState {
    descriptor: ChannelDescriptor,
    latest: Option<ChannelSample>,
    numeric_history: VecDeque<f64>,
    update_count: u64,
}

impl ChannelState {
    fn new(descriptor: ChannelDescriptor) -> Self {
        Self {
            descriptor,
            latest: None,
            numeric_history: VecDeque::with_capacity(HISTORY_LIMIT),
            update_count: 0,
        }
    }

    fn apply_sample(&mut self, sample: ChannelSample) {
        if let Some(value) = sample.value.numeric_value() {
            if self.numeric_history.len() == HISTORY_LIMIT {
                self.numeric_history.pop_front();
            }
            self.numeric_history.push_back(value);
        }

        self.latest = Some(sample);
        self.update_count += 1;
    }

    fn snapshot(&self, now: Instant) -> ChannelSnapshot {
        let is_stale = self
            .latest
            .as_ref()
            .map(|sample| now.saturating_duration_since(sample.timestamp) > STALE_AFTER)
            .unwrap_or(true);

        ChannelSnapshot {
            descriptor: self.descriptor.clone(),
            latest: self.latest.clone(),
            history: self.numeric_history.iter().copied().collect(),
            update_count: self.update_count,
            is_stale,
        }
    }
}

#[derive(Default)]
pub struct TelemetryStore {
    channels: BTreeMap<String, ChannelState>,
    plugin_health: BTreeMap<String, PluginHealth>,
    total_updates: u64,
    dropped_updates: u64,
}

impl TelemetryStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply_update(&mut self, update: TelemetryUpdate) {
        let TelemetryUpdate {
            plugin_id,
            descriptors,
            samples,
            health,
        } = update;

        for descriptor in descriptors {
            self.channels
                .entry(descriptor.path.clone())
                .or_insert_with(|| ChannelState::new(descriptor));
        }

        for sample in samples {
            if let Some(channel) = self.channels.get_mut(&sample.path) {
                channel.apply_sample(sample);
            } else {
                self.dropped_updates += 1;
            }
        }

        if let Some(health) = health {
            self.plugin_health.insert(plugin_id, health);
        }

        self.total_updates += 1;
    }

    pub fn snapshot(&self) -> StoreSnapshot {
        let now = Instant::now();
        StoreSnapshot {
            channels: self
                .channels
                .values()
                .map(|channel| channel.snapshot(now))
                .collect(),
            plugins: self
                .plugin_health
                .iter()
                .map(|(plugin_id, health)| PluginSnapshot {
                    plugin_id: plugin_id.clone(),
                    health: health.clone(),
                })
                .collect(),
            total_updates: self.total_updates,
            dropped_updates: self.dropped_updates,
        }
    }
}
