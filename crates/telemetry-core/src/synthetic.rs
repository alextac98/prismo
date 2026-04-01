use std::time::{Duration, Instant};

use rand::Rng;
use tokio::sync::mpsc;
use tokio::time;

use crate::model::{ChannelDescriptor, ChannelSample, ChannelValue, PluginHealth, TelemetryUpdate};
use crate::{PluginHandle, SourcePlugin};

#[derive(Clone, Copy)]
enum SyntheticValueKind {
    Float { base: f64, jitter: f64 },
    Integer { base: i64, jitter: i64 },
    Bool { nominal_probability: f64 },
    Text(&'static [&'static str]),
    Bytes { len: usize },
}

#[derive(Clone, Copy)]
struct DropoutPattern {
    cycle_ticks: u64,
    off_start_tick: u64,
    off_len_ticks: u64,
}

impl DropoutPattern {
    fn is_active(self, tick: u64) -> bool {
        let slot = tick % self.cycle_ticks;
        !(slot >= self.off_start_tick && slot < self.off_start_tick + self.off_len_ticks)
    }
}

#[derive(Clone, Copy)]
struct SyntheticChannelSpec {
    path: &'static str,
    unit: Option<&'static str>,
    description: &'static str,
    kind: SyntheticValueKind,
    every_ticks: u64,
    dropout: Option<DropoutPattern>,
}

impl SyntheticChannelSpec {
    fn descriptor(self) -> ChannelDescriptor {
        ChannelDescriptor {
            path: self.path.to_string(),
            display_name: self
                .path
                .rsplit('.')
                .next()
                .unwrap_or(self.path)
                .to_string(),
            unit: self.unit.map(str::to_string),
            description: self.description.to_string(),
        }
    }

    fn should_emit(self, tick: u64) -> bool {
        tick.is_multiple_of(self.every_ticks)
            && self
                .dropout
                .map(|pattern| pattern.is_active(tick))
                .unwrap_or(true)
    }

    fn is_dropped_out(self, tick: u64) -> bool {
        self.dropout
            .map(|pattern| !pattern.is_active(tick))
            .unwrap_or(false)
    }

    fn sample(self, tick: u64, sequence: u64, timestamp: Instant) -> ChannelSample {
        let mut rng = rand::rng();
        let phase = tick as f64 / self.every_ticks as f64;
        let value = match self.kind {
            SyntheticValueKind::Float { base, jitter } => {
                let wave = (phase * 0.37).sin() * jitter;
                let noise = rng.random_range((-jitter * 0.18)..(jitter * 0.18));
                ChannelValue::Float(base + wave + noise)
            }
            SyntheticValueKind::Integer { base, jitter } => {
                let wave = ((phase * 0.41).sin() * jitter as f64).round() as i64;
                let noise = rng.random_range(-jitter..=jitter);
                ChannelValue::Integer(base + wave + noise)
            }
            SyntheticValueKind::Bool {
                nominal_probability,
            } => ChannelValue::Bool(rng.random_bool(nominal_probability)),
            SyntheticValueKind::Text(options) => {
                let index = rng.random_range(0..options.len());
                ChannelValue::Text(options[index].to_string())
            }
            SyntheticValueKind::Bytes { len } => ChannelValue::Bytes(
                (0..len)
                    .map(|_| rng.random_range(0_u8..=255))
                    .collect::<Vec<_>>(),
            ),
        };

        ChannelSample {
            path: self.path.to_string(),
            value,
            timestamp,
            sequence,
        }
    }
}

pub struct SyntheticPlugin {
    period: Duration,
}

impl SyntheticPlugin {
    pub fn new(period: Duration) -> Self {
        Self { period }
    }

    fn specs(&self) -> Vec<SyntheticChannelSpec> {
        let secondary_link_dropout = Some(DropoutPattern {
            cycle_ticks: 90,
            off_start_tick: 38,
            off_len_ticks: 24,
        });
        let rear_camera_dropout = Some(DropoutPattern {
            cycle_ticks: 84,
            off_start_tick: 18,
            off_len_ticks: 20,
        });
        let gps_dropout = Some(DropoutPattern {
            cycle_ticks: 96,
            off_start_tick: 56,
            off_len_ticks: 22,
        });
        let science_dropout = Some(DropoutPattern {
            cycle_ticks: 72,
            off_start_tick: 30,
            off_len_ticks: 18,
        });

        vec![
            spec(
                "power.battery.voltage",
                Some("V"),
                "Battery bus voltage",
                SyntheticValueKind::Float {
                    base: 27.2,
                    jitter: 0.35,
                },
                1,
                None,
            ),
            spec(
                "power.battery.current",
                Some("A"),
                "Battery current draw",
                SyntheticValueKind::Float {
                    base: 8.1,
                    jitter: 1.0,
                },
                1,
                None,
            ),
            spec(
                "power.battery.soc",
                Some("%"),
                "Estimated battery state of charge",
                SyntheticValueKind::Float {
                    base: 82.0,
                    jitter: 0.4,
                },
                6,
                None,
            ),
            spec(
                "power.battery.temp",
                Some("C"),
                "Battery pack temperature",
                SyntheticValueKind::Float {
                    base: 34.0,
                    jitter: 1.5,
                },
                2,
                None,
            ),
            spec(
                "power.rails.logic_5v.voltage",
                Some("V"),
                "Logic rail voltage",
                SyntheticValueKind::Float {
                    base: 5.03,
                    jitter: 0.04,
                },
                2,
                None,
            ),
            spec(
                "power.rails.logic_5v.current",
                Some("A"),
                "Logic rail current",
                SyntheticValueKind::Float {
                    base: 1.8,
                    jitter: 0.15,
                },
                2,
                None,
            ),
            spec(
                "power.rails.motor_12v.voltage",
                Some("V"),
                "Motor rail voltage",
                SyntheticValueKind::Float {
                    base: 12.2,
                    jitter: 0.25,
                },
                2,
                None,
            ),
            spec(
                "power.rails.motor_12v.current",
                Some("A"),
                "Motor rail current",
                SyntheticValueKind::Float {
                    base: 5.7,
                    jitter: 0.7,
                },
                1,
                None,
            ),
            spec(
                "power.solar.panel_1.current",
                Some("A"),
                "Solar panel 1 current",
                SyntheticValueKind::Float {
                    base: 1.2,
                    jitter: 0.5,
                },
                4,
                None,
            ),
            spec(
                "power.solar.panel_2.current",
                Some("A"),
                "Solar panel 2 current",
                SyntheticValueKind::Float {
                    base: 1.1,
                    jitter: 0.5,
                },
                4,
                None,
            ),
            spec(
                "power.solar.panel_3.current",
                Some("A"),
                "Solar panel 3 current",
                SyntheticValueKind::Float {
                    base: 1.3,
                    jitter: 0.5,
                },
                4,
                None,
            ),
            spec(
                "power.solar.panel_4.current",
                Some("A"),
                "Solar panel 4 current",
                SyntheticValueKind::Float {
                    base: 1.0,
                    jitter: 0.5,
                },
                4,
                None,
            ),
            spec(
                "thermal.compute.cpu0.temp",
                Some("C"),
                "Primary CPU die temperature",
                SyntheticValueKind::Float {
                    base: 54.0,
                    jitter: 3.5,
                },
                1,
                None,
            ),
            spec(
                "thermal.compute.cpu1.temp",
                Some("C"),
                "Secondary CPU die temperature",
                SyntheticValueKind::Float {
                    base: 52.5,
                    jitter: 3.0,
                },
                1,
                None,
            ),
            spec(
                "thermal.compute.gpu.temp",
                Some("C"),
                "GPU temperature",
                SyntheticValueKind::Float {
                    base: 58.0,
                    jitter: 4.0,
                },
                1,
                None,
            ),
            spec(
                "thermal.avionics.temp",
                Some("C"),
                "Avionics board temperature",
                SyntheticValueKind::Float {
                    base: 41.5,
                    jitter: 1.6,
                },
                2,
                None,
            ),
            spec(
                "thermal.payload.camera_front.temp",
                Some("C"),
                "Front camera temperature",
                SyntheticValueKind::Float {
                    base: 39.0,
                    jitter: 2.0,
                },
                3,
                None,
            ),
            spec(
                "thermal.payload.camera_rear.temp",
                Some("C"),
                "Rear camera temperature",
                SyntheticValueKind::Float {
                    base: 37.0,
                    jitter: 2.0,
                },
                3,
                rear_camera_dropout,
            ),
            spec(
                "nav.position.altitude",
                Some("m"),
                "Altitude estimate",
                SyntheticValueKind::Float {
                    base: 1240.0,
                    jitter: 9.0,
                },
                1,
                None,
            ),
            spec(
                "nav.position.latitude",
                Some("deg"),
                "Latitude estimate",
                SyntheticValueKind::Float {
                    base: 37.422,
                    jitter: 0.0008,
                },
                4,
                gps_dropout,
            ),
            spec(
                "nav.position.longitude",
                Some("deg"),
                "Longitude estimate",
                SyntheticValueKind::Float {
                    base: -122.084,
                    jitter: 0.0008,
                },
                4,
                gps_dropout,
            ),
            spec(
                "nav.velocity.forward",
                Some("m/s"),
                "Forward velocity",
                SyntheticValueKind::Float {
                    base: 14.0,
                    jitter: 1.8,
                },
                1,
                None,
            ),
            spec(
                "nav.velocity.lateral",
                Some("m/s"),
                "Lateral velocity",
                SyntheticValueKind::Float {
                    base: 0.4,
                    jitter: 0.7,
                },
                2,
                None,
            ),
            spec(
                "nav.velocity.vertical",
                Some("m/s"),
                "Vertical velocity",
                SyntheticValueKind::Float {
                    base: -0.3,
                    jitter: 0.9,
                },
                1,
                None,
            ),
            spec(
                "nav.attitude.roll",
                Some("deg"),
                "Estimated roll angle",
                SyntheticValueKind::Float {
                    base: 0.0,
                    jitter: 4.5,
                },
                1,
                None,
            ),
            spec(
                "nav.attitude.pitch",
                Some("deg"),
                "Estimated pitch angle",
                SyntheticValueKind::Float {
                    base: 0.0,
                    jitter: 3.0,
                },
                1,
                None,
            ),
            spec(
                "nav.attitude.yaw",
                Some("deg"),
                "Estimated yaw angle",
                SyntheticValueKind::Float {
                    base: 182.0,
                    jitter: 15.0,
                },
                2,
                None,
            ),
            spec(
                "nav.gps.satellites",
                None,
                "Tracked GPS satellites",
                SyntheticValueKind::Integer {
                    base: 13,
                    jitter: 2,
                },
                5,
                gps_dropout,
            ),
            spec(
                "nav.gps.hdop",
                None,
                "Horizontal dilution of precision",
                SyntheticValueKind::Float {
                    base: 0.8,
                    jitter: 0.3,
                },
                5,
                gps_dropout,
            ),
            spec(
                "guidance.mode",
                None,
                "Current guidance mode",
                SyntheticValueKind::Text(&["IDLE", "SAFE", "TRACK", "DOCK", "HOLD"]),
                4,
                None,
            ),
            spec(
                "guidance.target.distance",
                Some("m"),
                "Distance to current target",
                SyntheticValueKind::Float {
                    base: 18.0,
                    jitter: 7.0,
                },
                2,
                None,
            ),
            spec(
                "guidance.target.bearing",
                Some("deg"),
                "Bearing to current target",
                SyntheticValueKind::Float {
                    base: 93.0,
                    jitter: 14.0,
                },
                2,
                None,
            ),
            spec(
                "guidance.plan.segment_index",
                None,
                "Current plan segment index",
                SyntheticValueKind::Integer { base: 4, jitter: 1 },
                8,
                None,
            ),
            spec(
                "guidance.hold.reason",
                None,
                "Current hold reason",
                SyntheticValueKind::Text(&["none", "terrain", "operator", "recovery"]),
                8,
                None,
            ),
            spec(
                "control.loops.attitude.error_roll",
                Some("deg"),
                "Roll control error",
                SyntheticValueKind::Float {
                    base: 0.0,
                    jitter: 1.2,
                },
                1,
                None,
            ),
            spec(
                "control.loops.attitude.error_pitch",
                Some("deg"),
                "Pitch control error",
                SyntheticValueKind::Float {
                    base: 0.0,
                    jitter: 1.0,
                },
                1,
                None,
            ),
            spec(
                "control.loops.attitude.error_yaw",
                Some("deg"),
                "Yaw control error",
                SyntheticValueKind::Float {
                    base: 0.0,
                    jitter: 2.0,
                },
                1,
                None,
            ),
            spec(
                "control.outputs.throttle",
                Some("%"),
                "Throttle output command",
                SyntheticValueKind::Float {
                    base: 42.0,
                    jitter: 11.0,
                },
                1,
                None,
            ),
            spec(
                "control.outputs.brake",
                Some("%"),
                "Brake output command",
                SyntheticValueKind::Float {
                    base: 2.0,
                    jitter: 4.0,
                },
                2,
                None,
            ),
            spec(
                "control.outputs.steer",
                Some("deg"),
                "Steering output command",
                SyntheticValueKind::Float {
                    base: 0.0,
                    jitter: 12.0,
                },
                1,
                None,
            ),
            spec(
                "actuators.drive.front_left.rpm",
                Some("rpm"),
                "Front left wheel RPM",
                SyntheticValueKind::Integer {
                    base: 1480,
                    jitter: 90,
                },
                1,
                None,
            ),
            spec(
                "actuators.drive.front_right.rpm",
                Some("rpm"),
                "Front right wheel RPM",
                SyntheticValueKind::Integer {
                    base: 1490,
                    jitter: 90,
                },
                1,
                None,
            ),
            spec(
                "actuators.drive.rear_left.rpm",
                Some("rpm"),
                "Rear left wheel RPM",
                SyntheticValueKind::Integer {
                    base: 1470,
                    jitter: 90,
                },
                1,
                None,
            ),
            spec(
                "actuators.drive.rear_right.rpm",
                Some("rpm"),
                "Rear right wheel RPM",
                SyntheticValueKind::Integer {
                    base: 1485,
                    jitter: 90,
                },
                1,
                None,
            ),
            spec(
                "actuators.gimbal.pan_deg",
                Some("deg"),
                "Gimbal pan angle",
                SyntheticValueKind::Float {
                    base: 15.0,
                    jitter: 22.0,
                },
                3,
                None,
            ),
            spec(
                "actuators.gimbal.tilt_deg",
                Some("deg"),
                "Gimbal tilt angle",
                SyntheticValueKind::Float {
                    base: -6.0,
                    jitter: 10.0,
                },
                3,
                None,
            ),
            spec(
                "actuators.valves.coolant.open",
                None,
                "Coolant valve state",
                SyntheticValueKind::Bool {
                    nominal_probability: 0.92,
                },
                4,
                None,
            ),
            spec(
                "actuators.valves.pressurant.open",
                None,
                "Pressurant valve state",
                SyntheticValueKind::Bool {
                    nominal_probability: 0.87,
                },
                4,
                None,
            ),
            spec(
                "comm.primary.health",
                None,
                "Primary link health",
                SyntheticValueKind::Text(&["nominal", "degraded", "recovering"]),
                3,
                None,
            ),
            spec(
                "comm.primary.rssi",
                Some("dBm"),
                "Primary link RSSI",
                SyntheticValueKind::Integer {
                    base: -61,
                    jitter: 5,
                },
                1,
                None,
            ),
            spec(
                "comm.primary.snr",
                Some("dB"),
                "Primary link SNR",
                SyntheticValueKind::Float {
                    base: 24.0,
                    jitter: 4.5,
                },
                2,
                None,
            ),
            spec(
                "comm.primary.tx_rate",
                Some("kbps"),
                "Primary transmit rate",
                SyntheticValueKind::Integer {
                    base: 540,
                    jitter: 120,
                },
                2,
                None,
            ),
            spec(
                "comm.primary.rx_rate",
                Some("kbps"),
                "Primary receive rate",
                SyntheticValueKind::Integer {
                    base: 610,
                    jitter: 130,
                },
                2,
                None,
            ),
            spec(
                "comm.secondary.health",
                None,
                "Secondary link health",
                SyntheticValueKind::Text(&["nominal", "degraded", "recovering", "timeout"]),
                4,
                secondary_link_dropout,
            ),
            spec(
                "comm.secondary.rssi",
                Some("dBm"),
                "Secondary link RSSI",
                SyntheticValueKind::Integer {
                    base: -74,
                    jitter: 6,
                },
                3,
                secondary_link_dropout,
            ),
            spec(
                "comm.secondary.snr",
                Some("dB"),
                "Secondary link SNR",
                SyntheticValueKind::Float {
                    base: 15.0,
                    jitter: 5.0,
                },
                4,
                secondary_link_dropout,
            ),
            spec(
                "comm.secondary.tx_rate",
                Some("kbps"),
                "Secondary transmit rate",
                SyntheticValueKind::Integer {
                    base: 220,
                    jitter: 70,
                },
                4,
                secondary_link_dropout,
            ),
            spec(
                "payload.cameras.front.frame",
                None,
                "Front camera frame bytes",
                SyntheticValueKind::Bytes { len: 16 },
                2,
                None,
            ),
            spec(
                "payload.cameras.rear.frame",
                None,
                "Rear camera frame bytes",
                SyntheticValueKind::Bytes { len: 16 },
                3,
                rear_camera_dropout,
            ),
            spec(
                "payload.cameras.front.exposure_ms",
                Some("ms"),
                "Front camera exposure time",
                SyntheticValueKind::Float {
                    base: 8.0,
                    jitter: 2.5,
                },
                6,
                None,
            ),
            spec(
                "payload.spectrometer.channel_1",
                None,
                "Spectrometer bin 1",
                SyntheticValueKind::Integer {
                    base: 512,
                    jitter: 90,
                },
                6,
                science_dropout,
            ),
            spec(
                "payload.spectrometer.channel_2",
                None,
                "Spectrometer bin 2",
                SyntheticValueKind::Integer {
                    base: 460,
                    jitter: 85,
                },
                6,
                science_dropout,
            ),
            spec(
                "payload.spectrometer.channel_3",
                None,
                "Spectrometer bin 3",
                SyntheticValueKind::Integer {
                    base: 420,
                    jitter: 80,
                },
                6,
                science_dropout,
            ),
            spec(
                "sensors.imu.accel.x",
                Some("m/s^2"),
                "IMU acceleration X",
                SyntheticValueKind::Float {
                    base: 0.2,
                    jitter: 0.9,
                },
                1,
                None,
            ),
            spec(
                "sensors.imu.accel.y",
                Some("m/s^2"),
                "IMU acceleration Y",
                SyntheticValueKind::Float {
                    base: -0.1,
                    jitter: 0.9,
                },
                1,
                None,
            ),
            spec(
                "sensors.imu.accel.z",
                Some("m/s^2"),
                "IMU acceleration Z",
                SyntheticValueKind::Float {
                    base: 9.81,
                    jitter: 0.4,
                },
                1,
                None,
            ),
            spec(
                "sensors.imu.gyro.x",
                Some("deg/s"),
                "IMU angular rate X",
                SyntheticValueKind::Float {
                    base: 0.0,
                    jitter: 4.0,
                },
                1,
                None,
            ),
            spec(
                "sensors.imu.gyro.y",
                Some("deg/s"),
                "IMU angular rate Y",
                SyntheticValueKind::Float {
                    base: 0.0,
                    jitter: 4.0,
                },
                1,
                None,
            ),
            spec(
                "sensors.imu.gyro.z",
                Some("deg/s"),
                "IMU angular rate Z",
                SyntheticValueKind::Float {
                    base: 0.0,
                    jitter: 6.0,
                },
                1,
                None,
            ),
            spec(
                "sensors.environment.pressure",
                Some("kPa"),
                "Ambient pressure",
                SyntheticValueKind::Float {
                    base: 99.8,
                    jitter: 0.5,
                },
                8,
                None,
            ),
            spec(
                "sensors.environment.humidity",
                Some("%"),
                "Ambient humidity",
                SyntheticValueKind::Float {
                    base: 44.0,
                    jitter: 3.0,
                },
                8,
                None,
            ),
            spec(
                "sensors.environment.wind_speed",
                Some("m/s"),
                "Wind speed estimate",
                SyntheticValueKind::Float {
                    base: 5.0,
                    jitter: 3.5,
                },
                8,
                science_dropout,
            ),
            spec(
                "estimator.covariance.position",
                None,
                "Position covariance",
                SyntheticValueKind::Float {
                    base: 0.7,
                    jitter: 0.4,
                },
                3,
                None,
            ),
            spec(
                "estimator.covariance.velocity",
                None,
                "Velocity covariance",
                SyntheticValueKind::Float {
                    base: 0.4,
                    jitter: 0.2,
                },
                3,
                None,
            ),
            spec(
                "estimator.covariance.attitude",
                None,
                "Attitude covariance",
                SyntheticValueKind::Float {
                    base: 0.3,
                    jitter: 0.15,
                },
                3,
                None,
            ),
            spec(
                "faults.power.low_battery",
                None,
                "Low battery fault",
                SyntheticValueKind::Bool {
                    nominal_probability: 0.03,
                },
                4,
                None,
            ),
            spec(
                "faults.thermal.overtemp",
                None,
                "Thermal over-temperature fault",
                SyntheticValueKind::Bool {
                    nominal_probability: 0.04,
                },
                4,
                None,
            ),
            spec(
                "faults.comm.primary_timeout",
                None,
                "Primary communication timeout fault",
                SyntheticValueKind::Bool {
                    nominal_probability: 0.02,
                },
                5,
                None,
            ),
            spec(
                "faults.comm.secondary_timeout",
                None,
                "Secondary communication timeout fault",
                SyntheticValueKind::Bool {
                    nominal_probability: 0.15,
                },
                5,
                secondary_link_dropout,
            ),
            spec(
                "faults.nav.gps_lost",
                None,
                "GPS signal lost fault",
                SyntheticValueKind::Bool {
                    nominal_probability: 0.18,
                },
                5,
                gps_dropout,
            ),
        ]
    }
}

impl Default for SyntheticPlugin {
    fn default() -> Self {
        Self::new(Duration::from_millis(200))
    }
}

impl SourcePlugin for SyntheticPlugin {
    fn id(&self) -> &'static str {
        "synthetic"
    }

    fn spawn(self: Box<Self>, tx: mpsc::Sender<TelemetryUpdate>) -> PluginHandle {
        tokio::spawn(async move {
            let specs = self.specs();
            let descriptors = specs
                .iter()
                .copied()
                .map(SyntheticChannelSpec::descriptor)
                .collect::<Vec<_>>();
            let mut ticker = time::interval(self.period);
            ticker.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

            let mut tick = 0_u64;
            let mut emitted_updates = 0_u64;
            let mut dropped_updates = 0_u64;

            loop {
                ticker.tick().await;
                tick += 1;
                emitted_updates += 1;

                let timestamp = Instant::now();
                let samples = specs
                    .iter()
                    .copied()
                    .filter(|spec| spec.should_emit(tick))
                    .map(|spec| spec.sample(tick, emitted_updates, timestamp))
                    .collect::<Vec<_>>();

                dropped_updates += specs
                    .iter()
                    .copied()
                    .filter(|spec| {
                        spec.is_dropped_out(tick) && tick.is_multiple_of(spec.every_ticks)
                    })
                    .count() as u64;

                let current_outages = specs
                    .iter()
                    .copied()
                    .filter(|spec| spec.is_dropped_out(tick))
                    .map(|spec| spec.path)
                    .take(3)
                    .collect::<Vec<_>>();

                let update = TelemetryUpdate {
                    plugin_id: self.id().to_string(),
                    descriptors: if tick == 1 {
                        descriptors.clone()
                    } else {
                        Vec::new()
                    },
                    samples,
                    health: Some(PluginHealth {
                        emitted_updates,
                        dropped_updates,
                        last_error: if current_outages.is_empty() {
                            None
                        } else {
                            Some(format!("dropouts: {}", current_outages.join(", ")))
                        },
                    }),
                };

                if tx.send(update).await.is_err() {
                    break;
                }
            }

            Ok(())
        })
    }
}

fn spec(
    path: &'static str,
    unit: Option<&'static str>,
    description: &'static str,
    kind: SyntheticValueKind,
    every_ticks: u64,
    dropout: Option<DropoutPattern>,
) -> SyntheticChannelSpec {
    SyntheticChannelSpec {
        path,
        unit,
        description,
        kind,
        every_ticks,
        dropout,
    }
}
