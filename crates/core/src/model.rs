use std::fmt;
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct ChannelDescriptor {
    pub path: String,
    pub display_name: String,
    pub unit: Option<String>,
    pub description: String,
}

#[derive(Clone, Debug)]
pub enum ChannelValue {
    Bool(bool),
    Integer(i64),
    Float(f64),
    Text(String),
    Bytes(Vec<u8>),
}

impl ChannelValue {
    pub fn numeric_value(&self) -> Option<f64> {
        match self {
            Self::Integer(value) => Some(*value as f64),
            Self::Float(value) => Some(*value),
            _ => None,
        }
    }

    pub fn short_display(&self) -> String {
        match self {
            Self::Bool(value) => value.to_string(),
            Self::Integer(value) => value.to_string(),
            Self::Float(value) => format!("{value:.3}"),
            Self::Text(value) => value.clone(),
            Self::Bytes(bytes) => bytes
                .iter()
                .take(8)
                .map(|byte| format!("{byte:02X}"))
                .collect::<Vec<_>>()
                .join(" "),
        }
    }
}

impl fmt::Display for ChannelValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(value) => write!(f, "{value}"),
            Self::Integer(value) => write!(f, "{value}"),
            Self::Float(value) => write!(f, "{value:.3}"),
            Self::Text(value) => f.write_str(value),
            Self::Bytes(bytes) => {
                let rendered = bytes
                    .iter()
                    .map(|byte| format!("{byte:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                f.write_str(&rendered)
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct ChannelSample {
    pub path: String,
    pub value: ChannelValue,
    pub observed_at: Instant,
    pub received_timestamp_unix_ns: u64,
    pub source_timestamp_unix_ns: u64,
    pub sequence: u64,
}

impl ChannelSample {
    pub fn effective_timestamp_unix_ns(&self) -> u64 {
        if self.source_timestamp_unix_ns > 0 {
            self.source_timestamp_unix_ns
        } else {
            self.received_timestamp_unix_ns
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumericPoint {
    pub timestamp_unix_ns: u64,
    pub value: f64,
}

#[derive(Clone, Debug, Default)]
pub struct PluginHealth {
    pub emitted_updates: u64,
    pub dropped_updates: u64,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct PluginSnapshot {
    pub plugin_id: String,
    pub state: PluginRuntimeState,
    pub restart_count: u64,
    pub message: Option<String>,
    pub health: PluginHealth,
}

#[derive(Clone, Debug, Default)]
pub struct TelemetryUpdate {
    pub plugin_id: String,
    pub descriptors: Vec<ChannelDescriptor>,
    pub samples: Vec<ChannelSample>,
    pub health: Option<PluginHealth>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub enum PluginRuntimeState {
    #[default]
    Starting,
    Running,
    Restarting,
    Stopped,
    Crashed,
}

impl fmt::Display for PluginRuntimeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Starting => f.write_str("starting"),
            Self::Running => f.write_str("running"),
            Self::Restarting => f.write_str("restarting"),
            Self::Stopped => f.write_str("stopped"),
            Self::Crashed => f.write_str("crashed"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PluginStatusUpdate {
    pub plugin_id: String,
    pub state: PluginRuntimeState,
    pub restart_count: u64,
    pub message: Option<String>,
}

#[derive(Clone, Debug)]
pub enum RuntimeEvent {
    Telemetry(TelemetryUpdate),
    PluginStatus(PluginStatusUpdate),
}
