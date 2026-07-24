use std::io::{self, BufReader, BufWriter, Read, Write};

use anyhow::{Context, Result, bail};
use serde::de::DeserializeOwned;

pub use prismo_plugin_protocol::{
    ArrayElementType, ArrayValue, ChannelDescriptor, EnumValue, Health, Init, Sample, Value,
    ValueKind,
};
use prismo_plugin_protocol::{
    DeclareChannels, Envelope, Hello, Log, Message, SampleBatch, read_delimited, write_delimited,
};

const PROTOCOL_VERSION: u32 = 1;

pub struct PluginIo<W: Write> {
    init: Init,
    writer: BufWriter<W>,
}

impl<W: Write> PluginIo<W> {
    pub fn init(&self) -> &Init {
        &self.init
    }

    pub fn config<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_str(&self.init.config_json).context("failed to decode plugin config json")
    }

    pub fn send_hello(
        &mut self,
        plugin_id: &str,
        plugin_version: &str,
        language: &str,
    ) -> Result<()> {
        self.send(Envelope {
            message: Some(Message::Hello(Hello {
                protocol_version: PROTOCOL_VERSION,
                plugin_id: plugin_id.to_string(),
                plugin_version: plugin_version.to_string(),
                language: language.to_string(),
            })),
        })
    }

    pub fn declare_channels(
        &mut self,
        plugin_id: &str,
        channels: Vec<ChannelDescriptor>,
    ) -> Result<()> {
        self.send(Envelope {
            message: Some(Message::DeclareChannels(DeclareChannels {
                plugin_id: plugin_id.to_string(),
                channels,
            })),
        })
    }

    pub fn send_samples(&mut self, plugin_id: &str, samples: Vec<Sample>) -> Result<()> {
        self.send(Envelope {
            message: Some(Message::SampleBatch(SampleBatch {
                plugin_id: plugin_id.to_string(),
                samples,
            })),
        })
    }

    pub fn send_health(&mut self, plugin_id: &str, health: Health) -> Result<()> {
        if health.plugin_id != plugin_id {
            bail!("health.plugin_id must match plugin_id argument");
        }
        self.send(Envelope {
            message: Some(Message::Health(health)),
        })
    }

    pub fn log(&mut self, plugin_id: &str, level: &str, message: &str) -> Result<()> {
        self.send(Envelope {
            message: Some(Message::Log(Log {
                plugin_id: plugin_id.to_string(),
                level: level.to_string(),
                message: message.to_string(),
            })),
        })
    }

    fn send(&mut self, envelope: Envelope) -> Result<()> {
        write_delimited(&mut self.writer, &envelope)
    }
}

pub fn stdio() -> Result<PluginIo<io::StdoutLock<'static>>> {
    let stdin = Box::leak(Box::new(io::stdin()));
    let stdout = Box::leak(Box::new(io::stdout()));
    build_stdio(stdin.lock(), stdout.lock())
}

fn build_stdio<R: Read, W: Write>(stdin: R, stdout: W) -> Result<PluginIo<W>> {
    let mut reader = BufReader::new(stdin);
    let init = match read_delimited(&mut reader)? {
        Some(Envelope {
            message: Some(Message::Init(init)),
        }) if init.protocol_version == PROTOCOL_VERSION => init,
        Some(Envelope { message: Some(_) }) => {
            bail!("plugin expected init message as first protocol frame")
        }
        Some(Envelope { message: None }) => bail!("plugin received empty protocol envelope"),
        None => bail!("plugin stdin closed before receiving init message"),
    };

    Ok(PluginIo {
        init,
        writer: BufWriter::new(stdout),
    })
}

pub fn value_bool(value: bool) -> Value {
    Value {
        kind: Some(prismo_plugin_protocol::ValueKind::BoolValue(value)),
    }
}

pub fn value_integer(value: i64) -> Value {
    Value {
        kind: Some(prismo_plugin_protocol::ValueKind::IntegerValue(value)),
    }
}

pub fn value_float(value: f64) -> Value {
    Value {
        kind: Some(prismo_plugin_protocol::ValueKind::FloatValue(value)),
    }
}

pub fn value_text(value: impl Into<String>) -> Value {
    Value {
        kind: Some(prismo_plugin_protocol::ValueKind::TextValue(value.into())),
    }
}

pub fn value_bytes(value: Vec<u8>) -> Value {
    Value {
        kind: Some(prismo_plugin_protocol::ValueKind::BytesValue(value)),
    }
}

pub fn value_enum(value: i64, name: impl Into<String>) -> Value {
    Value {
        kind: Some(prismo_plugin_protocol::ValueKind::EnumValue(EnumValue {
            value,
            name: name.into(),
        })),
    }
}

pub fn value_array(
    leaf_type: ArrayElementType,
    dimensions: u32,
    values: impl IntoIterator<Item = Value>,
) -> Result<Value> {
    if leaf_type == ArrayElementType::Unspecified {
        bail!("array leaf type must be specified");
    }
    if dimensions == 0 {
        bail!("array dimensions must be at least 1");
    }

    let values = values.into_iter().collect::<Vec<_>>();
    validate_array_values(leaf_type, dimensions, &values)?;

    Ok(Value {
        kind: Some(ValueKind::ArrayValue(ArrayValue {
            leaf_type: leaf_type as i32,
            dimensions,
            values,
        })),
    })
}

fn validate_array_values(
    leaf_type: ArrayElementType,
    dimensions: u32,
    values: &[Value],
) -> Result<()> {
    for (index, value) in values.iter().enumerate() {
        validate_array_element(leaf_type, dimensions, value)
            .with_context(|| format!("array element [{index}]"))?;
    }
    Ok(())
}

fn validate_array_element(
    leaf_type: ArrayElementType,
    dimensions: u32,
    value: &Value,
) -> Result<()> {
    let kind = value
        .kind
        .as_ref()
        .context("array element is missing a value")?;

    if dimensions == 1 {
        let matches = matches!(
            (leaf_type, kind),
            (ArrayElementType::Bool, ValueKind::BoolValue(_))
                | (ArrayElementType::Integer, ValueKind::IntegerValue(_))
                | (ArrayElementType::Float, ValueKind::FloatValue(_))
                | (ArrayElementType::Text, ValueKind::TextValue(_))
                | (ArrayElementType::Bytes, ValueKind::BytesValue(_))
                | (ArrayElementType::Enum, ValueKind::EnumValue(_))
        );
        if matches {
            return Ok(());
        }
        bail!("expected {leaf_type:?}, received {}", value_kind_name(kind));
    }

    let ValueKind::ArrayValue(array) = kind else {
        bail!(
            "expected {leaf_type:?}[{}], received {}",
            dimensions - 1,
            value_kind_name(kind)
        );
    };
    let child_type =
        ArrayElementType::try_from(array.leaf_type).context("array has unknown leaf type")?;
    if child_type != leaf_type || array.dimensions != dimensions - 1 {
        bail!(
            "expected {leaf_type:?}[{}], received {child_type:?}[{}]",
            dimensions - 1,
            array.dimensions
        );
    }
    validate_array_values(child_type, array.dimensions, &array.values)
}

fn value_kind_name(kind: &ValueKind) -> &'static str {
    match kind {
        ValueKind::BoolValue(_) => "Bool",
        ValueKind::IntegerValue(_) => "Integer",
        ValueKind::FloatValue(_) => "Float",
        ValueKind::TextValue(_) => "Text",
        ValueKind::BytesValue(_) => "Bytes",
        ValueKind::EnumValue(_) => "Enum",
        ValueKind::ArrayValue(_) => "Array",
    }
}

pub fn channel_descriptor<U: Into<String>>(
    channel_path: impl Into<String>,
    display_name: impl Into<String>,
    unit: Option<U>,
    description: impl Into<String>,
) -> ChannelDescriptor {
    ChannelDescriptor {
        channel_path: channel_path.into(),
        display_name: display_name.into(),
        unit: unit.map(Into::into),
        description: description.into(),
    }
}

pub fn sample(
    channel_path: impl Into<String>,
    timestamp_unix_ns: u64,
    sequence: u64,
    value: Value,
) -> Sample {
    Sample {
        channel_path: channel_path.into(),
        timestamp_unix_ns,
        sequence,
        value: Some(value),
    }
}

pub fn health<E: Into<String>>(
    plugin_id: impl Into<String>,
    emitted_updates: u64,
    dropped_updates: u64,
    last_error: Option<E>,
) -> Health {
    Health {
        plugin_id: plugin_id.into(),
        emitted_updates,
        dropped_updates,
        last_error: last_error.map(Into::into),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use serde::Deserialize;

    use super::{build_stdio, value_array, value_enum, value_float, value_integer};
    use prismo_plugin_protocol::{
        ArrayElementType, Envelope, Init, Message, ValueKind, write_delimited,
    };

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    struct ExampleConfig {
        tick_ms: u64,
    }

    #[test]
    fn reads_init_config() {
        let init = Envelope {
            message: Some(Message::Init(Init {
                protocol_version: 1,
                instance_id: "example-rust".to_string(),
                plugin_id: "example-rust".to_string(),
                config_json: r#"{"tick_ms":150}"#.to_string(),
            })),
        };
        let mut input = Vec::new();
        write_delimited(&mut input, &init).expect("write init");
        let mut output = Vec::new();

        let io = build_stdio(Cursor::new(input), &mut output).expect("plugin io");
        assert_eq!(
            io.config::<ExampleConfig>().expect("config"),
            ExampleConfig { tick_ms: 150 }
        );
    }

    #[test]
    fn builds_enum_value() {
        let value = value_enum(2, "SAFE");

        match value.kind {
            Some(ValueKind::EnumValue(value)) => {
                assert_eq!(value.value, 2);
                assert_eq!(value.name, "SAFE");
            }
            _ => panic!("expected enum value"),
        }
    }

    #[test]
    fn builds_ragged_nested_array() {
        let first_row = value_array(
            ArrayElementType::Integer,
            1,
            [value_integer(1), value_integer(2)],
        )
        .expect("first row");
        let second_row =
            value_array(ArrayElementType::Integer, 1, [value_integer(3)]).expect("second row");
        let empty_row =
            value_array(ArrayElementType::Integer, 1, std::iter::empty()).expect("empty row");

        let value = value_array(
            ArrayElementType::Integer,
            2,
            [first_row, second_row, empty_row],
        )
        .expect("nested array");

        match value.kind {
            Some(ValueKind::ArrayValue(value)) => {
                assert_eq!(value.leaf_type, ArrayElementType::Integer as i32);
                assert_eq!(value.dimensions, 2);
                assert_eq!(value.values.len(), 3);
            }
            _ => panic!("expected array value"),
        }
    }

    #[test]
    fn rejects_mixed_array_elements() {
        let error = value_array(
            ArrayElementType::Integer,
            1,
            [value_integer(1), value_float(2.0)],
        )
        .expect_err("reject mixed array");

        assert!(format!("{error:#}").contains("array element [1]"));
    }

    #[test]
    fn rejects_inconsistent_array_depth() {
        let row =
            value_array(ArrayElementType::Integer, 1, [value_integer(1)]).expect("integer row");
        let error = value_array(ArrayElementType::Integer, 3, [row])
            .expect_err("reject wrong nested depth");

        let message = format!("{error:#}");
        assert!(message.contains("array element [0]"));
        assert!(message.contains("expected Integer[2], received Integer[1]"));
    }

    #[test]
    fn builds_nested_enum_array() {
        let row = value_array(
            ArrayElementType::Enum,
            1,
            [value_enum(1, "IDLE"), value_enum(2, "ACTIVE")],
        )
        .expect("enum row");
        let value = value_array(ArrayElementType::Enum, 2, [row]).expect("nested enum array");

        assert!(matches!(value.kind, Some(ValueKind::ArrayValue(_))));
    }
}
