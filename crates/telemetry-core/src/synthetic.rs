use std::time::{Duration, Instant};

use rand::Rng;
use tokio::sync::mpsc;
use tokio::time;

use crate::model::{ChannelDescriptor, ChannelSample, ChannelValue, PluginHealth, TelemetryUpdate};
use crate::{PluginHandle, SourcePlugin};

const CHANNELS: &[(&str, Option<&str>, &str)] = &[
    ("power.battery.voltage", Some("V"), "Battery bus voltage"),
    ("power.battery.current", Some("A"), "Battery current draw"),
    (
        "power.battery.soc",
        Some("%"),
        "Estimated battery state of charge",
    ),
    (
        "thermal.cpu.temp",
        Some("C"),
        "Main compute die temperature",
    ),
    (
        "thermal.avionics.temp",
        Some("C"),
        "Avionics board temperature",
    ),
    ("nav.position.altitude", Some("m"), "Altitude estimate"),
    ("nav.velocity.forward", Some("m/s"), "Forward velocity"),
    ("nav.velocity.vertical", Some("m/s"), "Vertical velocity"),
    ("guidance.mode", None, "Current guidance mode"),
    ("comm.link.health", None, "Primary comms link health"),
    ("comm.link.rssi", Some("dBm"), "Received signal strength"),
    ("payload.camera.frame", None, "Recent payload bytes"),
];

pub struct SyntheticPlugin {
    period: Duration,
}

impl SyntheticPlugin {
    pub fn new(period: Duration) -> Self {
        Self { period }
    }

    fn descriptors(&self) -> Vec<ChannelDescriptor> {
        CHANNELS
            .iter()
            .map(|(path, unit, description)| ChannelDescriptor {
                path: (*path).to_string(),
                display_name: path.rsplit('.').next().unwrap_or(path).to_string(),
                unit: unit.map(str::to_string),
                description: (*description).to_string(),
            })
            .collect()
    }
}

impl Default for SyntheticPlugin {
    fn default() -> Self {
        Self::new(Duration::from_millis(250))
    }
}

impl SourcePlugin for SyntheticPlugin {
    fn id(&self) -> &'static str {
        "synthetic"
    }

    fn spawn(self: Box<Self>, tx: mpsc::Sender<TelemetryUpdate>) -> PluginHandle {
        tokio::spawn(async move {
            let descriptors = self.descriptors();
            let mut ticker = time::interval(self.period);
            ticker.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

            let mut sequence = 0_u64;
            let mut emitted_updates = 0_u64;
            loop {
                ticker.tick().await;
                sequence += 1;
                emitted_updates += 1;

                let update = {
                    let timestamp = Instant::now();
                    let mut rng = rand::rng();
                    let guidance_mode = ["IDLE", "SAFE", "TRACK", "DOCK"][rng.random_range(0..4)];
                    let link_health = ["nominal", "degraded", "recovering"][rng.random_range(0..3)];
                    let bytes = (0..8)
                        .map(|_| rng.random_range(0_u8..=255))
                        .collect::<Vec<_>>();

                    TelemetryUpdate {
                        plugin_id: self.id().to_string(),
                        descriptors: if sequence == 1 {
                            descriptors.clone()
                        } else {
                            Vec::new()
                        },
                        samples: vec![
                            ChannelSample {
                                path: "power.battery.voltage".to_string(),
                                value: ChannelValue::Float(27.0 + rng.random_range(-0.4..0.4)),
                                timestamp,
                                sequence,
                            },
                            ChannelSample {
                                path: "power.battery.current".to_string(),
                                value: ChannelValue::Float(8.0 + rng.random_range(-1.2..1.2)),
                                timestamp,
                                sequence,
                            },
                            ChannelSample {
                                path: "power.battery.soc".to_string(),
                                value: ChannelValue::Float(82.0 + rng.random_range(-0.5..0.5)),
                                timestamp,
                                sequence,
                            },
                            ChannelSample {
                                path: "thermal.cpu.temp".to_string(),
                                value: ChannelValue::Float(53.0 + rng.random_range(-2.0..4.0)),
                                timestamp,
                                sequence,
                            },
                            ChannelSample {
                                path: "thermal.avionics.temp".to_string(),
                                value: ChannelValue::Float(41.0 + rng.random_range(-1.5..1.5)),
                                timestamp,
                                sequence,
                            },
                            ChannelSample {
                                path: "nav.position.altitude".to_string(),
                                value: ChannelValue::Float(1240.0 + rng.random_range(-8.0..8.0)),
                                timestamp,
                                sequence,
                            },
                            ChannelSample {
                                path: "nav.velocity.forward".to_string(),
                                value: ChannelValue::Float(14.2 + rng.random_range(-1.5..1.5)),
                                timestamp,
                                sequence,
                            },
                            ChannelSample {
                                path: "nav.velocity.vertical".to_string(),
                                value: ChannelValue::Float(-0.4 + rng.random_range(-0.8..0.8)),
                                timestamp,
                                sequence,
                            },
                            ChannelSample {
                                path: "guidance.mode".to_string(),
                                value: ChannelValue::Text(guidance_mode.to_string()),
                                timestamp,
                                sequence,
                            },
                            ChannelSample {
                                path: "comm.link.health".to_string(),
                                value: ChannelValue::Text(link_health.to_string()),
                                timestamp,
                                sequence,
                            },
                            ChannelSample {
                                path: "comm.link.rssi".to_string(),
                                value: ChannelValue::Integer(-60 + rng.random_range(-6..6)),
                                timestamp,
                                sequence,
                            },
                            ChannelSample {
                                path: "payload.camera.frame".to_string(),
                                value: ChannelValue::Bytes(bytes),
                                timestamp,
                                sequence,
                            },
                        ],
                        health: Some(PluginHealth {
                            emitted_updates,
                            dropped_updates: 0,
                            last_error: None,
                        }),
                    }
                };

                if tx.send(update).await.is_err() {
                    break;
                }
            }

            Ok(())
        })
    }
}
