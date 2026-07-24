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
    Enum {
        value: i64,
        name: String,
    },
    Array {
        leaf_type: ArrayElementType,
        dimensions: u32,
        values: Vec<ChannelValue>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArrayElementType {
    Bool,
    Integer,
    Float,
    Text,
    Bytes,
    Enum,
}

impl fmt::Display for ArrayElementType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool => f.write_str("Bool"),
            Self::Integer => f.write_str("Integer"),
            Self::Float => f.write_str("Float"),
            Self::Text => f.write_str("Text"),
            Self::Bytes => f.write_str("Bytes"),
            Self::Enum => f.write_str("Enum"),
        }
    }
}

impl ChannelValue {
    pub fn numeric_value(&self) -> Option<f64> {
        match self {
            Self::Integer(value) => Some(*value as f64),
            Self::Float(value) => Some(*value),
            Self::Enum { value, .. } => Some(*value as f64),
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
            Self::Enum { value, name } => format_enum_value(*value, name),
            Self::Array {
                leaf_type,
                dimensions,
                values,
            } => format_array_summary(*leaf_type, *dimensions, values.len()),
        }
    }

    pub fn numeric_array_values(&self) -> Option<Vec<NumericArrayValue>> {
        let Self::Array { leaf_type, .. } = self else {
            return None;
        };

        if !matches!(
            leaf_type,
            ArrayElementType::Integer | ArrayElementType::Float
        ) {
            return None;
        }

        let mut values = Vec::new();
        if append_numeric_array_values(self, *leaf_type, &mut Vec::new(), &mut values) {
            Some(values)
        } else {
            None
        }
    }
}

fn append_numeric_array_values(
    value: &ChannelValue,
    leaf_type: ArrayElementType,
    index_path: &mut Vec<usize>,
    values: &mut Vec<NumericArrayValue>,
) -> bool {
    match value {
        ChannelValue::Array {
            leaf_type: nested_leaf_type,
            values: nested_values,
            ..
        } if *nested_leaf_type == leaf_type => {
            for (index, nested_value) in nested_values.iter().enumerate() {
                index_path.push(index);
                if !append_numeric_array_values(nested_value, leaf_type, index_path, values) {
                    return false;
                }
                index_path.pop();
            }
            true
        }
        ChannelValue::Integer(value) if leaf_type == ArrayElementType::Integer => {
            values.push(NumericArrayValue {
                index_path: index_path.clone(),
                value: *value as f64,
            });
            true
        }
        ChannelValue::Float(value) if leaf_type == ArrayElementType::Float => {
            values.push(NumericArrayValue {
                index_path: index_path.clone(),
                value: *value,
            });
            true
        }
        _ => false,
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
            Self::Enum { value, name } => f.write_str(&format_enum_value(*value, name)),
            Self::Array {
                leaf_type,
                dimensions,
                values,
            } => f.write_str(&format_array_summary(*leaf_type, *dimensions, values.len())),
        }
    }
}

fn format_enum_value(value: i64, name: &str) -> String {
    if name.is_empty() {
        value.to_string()
    } else {
        format!("{name} ({value})")
    }
}

fn format_array_summary(
    leaf_type: ArrayElementType,
    dimensions: u32,
    value_count: usize,
) -> String {
    let item_label = if value_count == 1 { "item" } else { "items" };
    format!("{leaf_type}[{dimensions}] ({value_count} {item_label})")
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

#[derive(Clone, Debug, PartialEq)]
pub struct NumericArrayPoint {
    pub timestamp_unix_ns: u64,
    pub values: Vec<NumericArrayValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NumericArrayValue {
    pub index_path: Vec<usize>,
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

#[cfg(test)]
mod tests {
    use super::{ArrayElementType, ChannelValue, NumericArrayValue};

    #[test]
    fn enum_values_display_name_and_discriminant() {
        let value = ChannelValue::Enum {
            value: 2,
            name: "SAFE".to_string(),
        };

        assert_eq!(value.short_display(), "SAFE (2)");
        assert_eq!(value.to_string(), "SAFE (2)");
        assert_eq!(value.numeric_value(), Some(2.0));
    }

    #[test]
    fn unnamed_enum_values_display_discriminant() {
        let value = ChannelValue::Enum {
            value: -1,
            name: String::new(),
        };

        assert_eq!(value.short_display(), "-1");
        assert_eq!(value.to_string(), "-1");
    }

    #[test]
    fn arrays_display_type_dimensions_and_top_level_size() {
        let value = ChannelValue::Array {
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
                    values: Vec::new(),
                },
            ],
        };

        assert_eq!(value.short_display(), "Integer[2] (2 items)");
        assert_eq!(value.to_string(), "Integer[2] (2 items)");
        assert_eq!(value.numeric_value(), None);
        assert_eq!(
            value.numeric_array_values(),
            Some(vec![
                NumericArrayValue {
                    index_path: vec![0, 0],
                    value: 1.0,
                },
                NumericArrayValue {
                    index_path: vec![0, 1],
                    value: 2.0,
                },
            ])
        );
    }

    #[test]
    fn flat_numeric_arrays_expose_plot_values() {
        let integers = ChannelValue::Array {
            leaf_type: ArrayElementType::Integer,
            dimensions: 1,
            values: vec![ChannelValue::Integer(1), ChannelValue::Integer(2)],
        };
        let floats = ChannelValue::Array {
            leaf_type: ArrayElementType::Float,
            dimensions: 1,
            values: vec![ChannelValue::Float(1.5), ChannelValue::Float(2.5)],
        };

        assert_eq!(
            integers.numeric_array_values(),
            Some(vec![
                NumericArrayValue {
                    index_path: vec![0],
                    value: 1.0,
                },
                NumericArrayValue {
                    index_path: vec![1],
                    value: 2.0,
                },
            ])
        );
        assert_eq!(
            floats.numeric_array_values(),
            Some(vec![
                NumericArrayValue {
                    index_path: vec![0],
                    value: 1.5,
                },
                NumericArrayValue {
                    index_path: vec![1],
                    value: 2.5,
                },
            ])
        );
    }
}
