use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};

use crate::{
    ChannelDescriptor, ChannelSample, PluginHealth, PluginRuntimeState, PluginSnapshot,
    PluginStatusUpdate, RuntimeEvent, TelemetryUpdate,
};

const HISTORY_LIMIT: usize = 64;
const INITIAL_STALE_AFTER: Duration = Duration::from_secs(3);

#[derive(Clone, Debug)]
pub struct ChannelSnapshot {
    pub plugin_id: String,
    pub descriptor: ChannelDescriptor,
    pub latest: Option<ChannelSample>,
    pub history: Vec<f64>,
    pub update_count: u64,
    pub rate_hz: Option<f64>,
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
    plugin_id: String,
    descriptor: ChannelDescriptor,
    latest: Option<ChannelSample>,
    numeric_history: VecDeque<f64>,
    update_count: u64,
    last_interval: Option<Duration>,
    rate_hz: Option<f64>,
}

impl ChannelState {
    fn new(plugin_id: String, descriptor: ChannelDescriptor) -> Self {
        Self {
            plugin_id,
            descriptor,
            latest: None,
            numeric_history: VecDeque::with_capacity(HISTORY_LIMIT),
            update_count: 0,
            last_interval: None,
            rate_hz: None,
        }
    }

    fn apply_sample(&mut self, sample: ChannelSample) {
        self.last_interval = self.latest.as_ref().and_then(|previous| {
            sample
                .source_timestamp_unix_ns
                .checked_sub(previous.source_timestamp_unix_ns)
                .map(Duration::from_nanos)
                .or_else(|| {
                    sample
                        .observed_at
                        .checked_duration_since(previous.observed_at)
                })
        });
        self.rate_hz = self.last_interval.and_then(|interval| {
            let seconds = interval.as_secs_f64();
            (seconds > 0.0).then_some(1.0 / seconds)
        });

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
        let stale_after = self
            .last_interval
            .map(|interval| interval.saturating_mul(3))
            .unwrap_or(INITIAL_STALE_AFTER);
        let is_stale = self
            .latest
            .as_ref()
            .map(|sample| now.saturating_duration_since(sample.observed_at) > stale_after)
            .unwrap_or(true);

        ChannelSnapshot {
            plugin_id: self.plugin_id.clone(),
            descriptor: self.descriptor.clone(),
            latest: self.latest.clone(),
            history: self.numeric_history.iter().copied().collect(),
            update_count: self.update_count,
            rate_hz: self.rate_hz,
            is_stale,
        }
    }
}

#[derive(Default)]
pub struct TelemetryStore {
    channels: BTreeMap<(String, String), ChannelState>,
    plugin_health: BTreeMap<String, PluginHealth>,
    plugin_status: BTreeMap<String, PluginStatusUpdate>,
    total_updates: u64,
    dropped_updates: u64,
}

impl TelemetryStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply_event(&mut self, event: RuntimeEvent) {
        match event {
            RuntimeEvent::Telemetry(update) => self.apply_update(update),
            RuntimeEvent::PluginStatus(status) => self.apply_plugin_status(status),
        }
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
                .entry((plugin_id.clone(), descriptor.path.clone()))
                .or_insert_with(|| ChannelState::new(plugin_id.clone(), descriptor));
        }

        for sample in samples {
            if let Some(channel) = self
                .channels
                .get_mut(&(plugin_id.clone(), sample.path.clone()))
            {
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

    pub fn apply_plugin_status(&mut self, status: PluginStatusUpdate) {
        self.plugin_status.insert(status.plugin_id.clone(), status);
    }

    pub fn snapshot(&self) -> StoreSnapshot {
        let now = Instant::now();
        let plugin_ids = self
            .plugin_health
            .keys()
            .chain(self.plugin_status.keys())
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        StoreSnapshot {
            channels: self
                .channels
                .values()
                .map(|channel| channel.snapshot(now))
                .collect(),
            plugins: plugin_ids
                .iter()
                .map(|plugin_id| PluginSnapshot {
                    plugin_id: plugin_id.clone(),
                    state: self
                        .plugin_status
                        .get(plugin_id)
                        .map(|status| status.state)
                        .unwrap_or(PluginRuntimeState::Starting),
                    restart_count: self
                        .plugin_status
                        .get(plugin_id)
                        .map(|status| status.restart_count)
                        .unwrap_or(0),
                    message: self
                        .plugin_status
                        .get(plugin_id)
                        .and_then(|status| status.message.clone()),
                    health: self
                        .plugin_health
                        .get(plugin_id)
                        .cloned()
                        .unwrap_or_default(),
                })
                .collect(),
            total_updates: self.total_updates,
            dropped_updates: self.dropped_updates,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crate::{
        ChannelDescriptor, ChannelSample, ChannelValue, PluginHealth, PluginRuntimeState,
        PluginStatusUpdate, RuntimeEvent, TelemetryStore, TelemetryUpdate,
    };

    #[test]
    fn drops_samples_that_arrive_before_descriptors() {
        let mut store = TelemetryStore::new();

        store.apply_event(RuntimeEvent::Telemetry(TelemetryUpdate {
            plugin_id: "example-rust".to_string(),
            descriptors: Vec::new(),
            samples: vec![sample("power.battery.voltage", 27.2, Instant::now(), 1)],
            health: None,
        }));

        let snapshot = store.snapshot();

        assert_eq!(snapshot.total_updates, 1);
        assert_eq!(snapshot.dropped_updates, 1);
        assert!(snapshot.channels.is_empty());
    }

    #[test]
    fn tracks_channel_history_and_plugin_health() {
        let mut store = TelemetryStore::new();
        let start = Instant::now();

        store.apply_event(RuntimeEvent::PluginStatus(PluginStatusUpdate {
            plugin_id: "example-rust".to_string(),
            state: PluginRuntimeState::Running,
            restart_count: 0,
            message: Some("ready".to_string()),
        }));
        store.apply_event(RuntimeEvent::Telemetry(TelemetryUpdate {
            plugin_id: "example-rust".to_string(),
            descriptors: vec![descriptor("power.battery.voltage", Some("V"))],
            samples: vec![sample("power.battery.voltage", 27.2, start, 1)],
            health: Some(PluginHealth {
                emitted_updates: 1,
                dropped_updates: 0,
                last_error: None,
            }),
        }));
        store.apply_event(RuntimeEvent::Telemetry(TelemetryUpdate {
            plugin_id: "example-rust".to_string(),
            descriptors: Vec::new(),
            samples: vec![sample(
                "power.battery.voltage",
                27.4,
                start + Duration::from_millis(100),
                2,
            )],
            health: Some(PluginHealth {
                emitted_updates: 2,
                dropped_updates: 0,
                last_error: Some("none".to_string()),
            }),
        }));

        let snapshot = store.snapshot();
        let channel = snapshot
            .channels
            .iter()
            .find(|channel| channel.descriptor.path == "power.battery.voltage")
            .expect("channel snapshot");
        let plugin = snapshot
            .plugins
            .iter()
            .find(|plugin| plugin.plugin_id == "example-rust")
            .expect("plugin snapshot");

        assert_eq!(snapshot.total_updates, 2);
        assert_eq!(snapshot.dropped_updates, 0);
        assert_eq!(channel.history, vec![27.2, 27.4]);
        assert_eq!(channel.update_count, 2);
        assert_eq!(
            channel.latest.as_ref().map(|sample| sample.sequence),
            Some(2)
        );
        assert_eq!(channel.plugin_id, "example-rust");
        assert_eq!(plugin.state, PluginRuntimeState::Running);
        assert_eq!(plugin.health.emitted_updates, 2);
        assert_eq!(plugin.health.last_error.as_deref(), Some("none"));
        assert!(!channel.is_stale);
        assert!(channel.rate_hz.expect("sample rate") > 0.0);
    }

    fn descriptor(path: &str, unit: Option<&str>) -> ChannelDescriptor {
        ChannelDescriptor {
            path: path.to_string(),
            display_name: path.rsplit('.').next().unwrap_or(path).to_string(),
            unit: unit.map(str::to_string),
            description: "test channel".to_string(),
        }
    }

    fn sample(path: &str, value: f64, timestamp: Instant, sequence: u64) -> ChannelSample {
        ChannelSample {
            path: path.to_string(),
            value: ChannelValue::Float(value),
            observed_at: timestamp,
            source_timestamp_unix_ns: sequence * 100_000_000,
            sequence,
        }
    }
}
