use std::io::{self, Read, Write};

use anyhow::{Context, Result, bail};
use prost::Message as ProstMessage;

const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, PartialEq, prost::Message)]
pub struct Envelope {
    #[prost(oneof = "Message", tags = "1, 2, 3, 4, 5, 6, 7")]
    pub message: Option<Message>,
}

#[derive(Clone, PartialEq, prost::Oneof)]
pub enum Message {
    #[prost(message, tag = "1")]
    Init(Init),
    #[prost(message, tag = "2")]
    Hello(Hello),
    #[prost(message, tag = "3")]
    DeclareChannels(DeclareChannels),
    #[prost(message, tag = "4")]
    SampleBatch(SampleBatch),
    #[prost(message, tag = "5")]
    Health(Health),
    #[prost(message, tag = "6")]
    Shutdown(Shutdown),
    #[prost(message, tag = "7")]
    Log(Log),
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Init {
    #[prost(uint32, tag = "1")]
    pub protocol_version: u32,
    #[prost(string, tag = "2")]
    pub instance_id: String,
    #[prost(string, tag = "3")]
    pub plugin_id: String,
    #[prost(string, tag = "4")]
    pub config_json: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Hello {
    #[prost(uint32, tag = "1")]
    pub protocol_version: u32,
    #[prost(string, tag = "2")]
    pub plugin_id: String,
    #[prost(string, tag = "3")]
    pub plugin_version: String,
    #[prost(string, tag = "4")]
    pub language: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct DeclareChannels {
    #[prost(string, tag = "1")]
    pub plugin_id: String,
    #[prost(message, repeated, tag = "2")]
    pub channels: Vec<ChannelDescriptor>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ChannelDescriptor {
    #[prost(string, tag = "1")]
    pub channel_path: String,
    #[prost(string, tag = "2")]
    pub display_name: String,
    #[prost(string, optional, tag = "3")]
    pub unit: Option<String>,
    #[prost(string, tag = "4")]
    pub description: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SampleBatch {
    #[prost(string, tag = "1")]
    pub plugin_id: String,
    #[prost(message, repeated, tag = "2")]
    pub samples: Vec<Sample>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Sample {
    #[prost(string, tag = "1")]
    pub channel_path: String,
    #[prost(uint64, tag = "2")]
    pub timestamp_unix_ns: u64,
    #[prost(uint64, tag = "3")]
    pub sequence: u64,
    #[prost(message, optional, tag = "4")]
    pub value: Option<Value>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Value {
    #[prost(oneof = "ValueKind", tags = "1, 2, 3, 4, 5, 6, 7")]
    pub kind: Option<ValueKind>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct EnumValue {
    #[prost(int64, tag = "1")]
    pub value: i64,
    #[prost(string, tag = "2")]
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum ArrayElementType {
    Unspecified = 0,
    Bool = 1,
    Integer = 2,
    Float = 3,
    Text = 4,
    Bytes = 5,
    Enum = 6,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ArrayValue {
    #[prost(enumeration = "ArrayElementType", tag = "1")]
    pub leaf_type: i32,
    #[prost(uint32, tag = "2")]
    pub dimensions: u32,
    #[prost(message, repeated, tag = "3")]
    pub values: Vec<Value>,
}

#[derive(Clone, PartialEq, prost::Oneof)]
pub enum ValueKind {
    #[prost(bool, tag = "1")]
    BoolValue(bool),
    #[prost(int64, tag = "2")]
    IntegerValue(i64),
    #[prost(double, tag = "3")]
    FloatValue(f64),
    #[prost(string, tag = "4")]
    TextValue(String),
    #[prost(bytes, tag = "5")]
    BytesValue(Vec<u8>),
    #[prost(message, tag = "6")]
    EnumValue(EnumValue),
    #[prost(message, tag = "7")]
    ArrayValue(ArrayValue),
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Health {
    #[prost(string, tag = "1")]
    pub plugin_id: String,
    #[prost(uint64, tag = "2")]
    pub emitted_updates: u64,
    #[prost(uint64, tag = "3")]
    pub dropped_updates: u64,
    #[prost(string, optional, tag = "4")]
    pub last_error: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Shutdown {
    #[prost(string, tag = "1")]
    pub reason: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Log {
    #[prost(string, tag = "1")]
    pub plugin_id: String,
    #[prost(string, tag = "2")]
    pub level: String,
    #[prost(string, tag = "3")]
    pub message: String,
}

pub fn write_delimited<W: Write>(writer: &mut W, envelope: &Envelope) -> Result<()> {
    let mut buf = Vec::with_capacity(envelope.encoded_len());
    envelope
        .encode(&mut buf)
        .context("failed to encode protobuf envelope")?;
    let len = u32::try_from(buf.len()).context("protobuf envelope exceeds u32 length")?;

    writer
        .write_all(&len.to_le_bytes())
        .context("failed to write frame length")?;
    writer
        .write_all(&buf)
        .context("failed to write frame payload")?;
    writer.flush().context("failed to flush frame")?;
    Ok(())
}

pub fn read_delimited<R: Read>(reader: &mut R) -> Result<Option<Envelope>> {
    let mut len_buf = [0_u8; 4];
    match reader.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error).context("failed to read frame length"),
    }

    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_FRAME_BYTES {
        bail!("received oversized frame: {} bytes", len);
    }

    let mut buf = vec![0_u8; len];
    reader
        .read_exact(&mut buf)
        .context("failed to read frame payload")?;
    Envelope::decode(buf.as_slice())
        .context("failed to decode protobuf envelope")
        .map(Some)
}

#[cfg(test)]
mod tests {
    use super::{
        ArrayElementType, ArrayValue, EnumValue, Envelope, Hello, Message, Sample, SampleBatch,
        Value, ValueKind, read_delimited, write_delimited,
    };

    #[test]
    fn round_trips_delimited_envelope() {
        let envelope = Envelope {
            message: Some(Message::Hello(Hello {
                protocol_version: 1,
                plugin_id: "example-rust".to_string(),
                plugin_version: "0.1.0".to_string(),
                language: "rust".to_string(),
            })),
        };

        let mut bytes = Vec::new();
        write_delimited(&mut bytes, &envelope).expect("write envelope");

        let mut slice = bytes.as_slice();
        let decoded = read_delimited(&mut slice)
            .expect("read envelope")
            .expect("non-empty frame");

        assert_eq!(decoded, envelope);
    }

    #[test]
    fn round_trips_enum_value() {
        let envelope = Envelope {
            message: Some(Message::SampleBatch(SampleBatch {
                plugin_id: "example-rust".to_string(),
                samples: vec![Sample {
                    channel_path: "guidance.mode".to_string(),
                    timestamp_unix_ns: 42,
                    sequence: 1,
                    value: Some(Value {
                        kind: Some(ValueKind::EnumValue(EnumValue {
                            value: 2,
                            name: "SAFE".to_string(),
                        })),
                    }),
                }],
            })),
        };

        let mut bytes = Vec::new();
        write_delimited(&mut bytes, &envelope).expect("write enum envelope");

        let mut slice = bytes.as_slice();
        let decoded = read_delimited(&mut slice)
            .expect("read enum envelope")
            .expect("non-empty frame");

        assert_eq!(decoded, envelope);
    }

    #[test]
    fn round_trips_nested_array_value() {
        let row = |values: Vec<i64>| Value {
            kind: Some(ValueKind::ArrayValue(ArrayValue {
                leaf_type: ArrayElementType::Integer as i32,
                dimensions: 1,
                values: values
                    .into_iter()
                    .map(|value| Value {
                        kind: Some(ValueKind::IntegerValue(value)),
                    })
                    .collect(),
            })),
        };
        let array = Value {
            kind: Some(ValueKind::ArrayValue(ArrayValue {
                leaf_type: ArrayElementType::Integer as i32,
                dimensions: 2,
                values: vec![row(vec![1, 2]), row(vec![3]), row(Vec::new())],
            })),
        };
        let envelope = Envelope {
            message: Some(Message::SampleBatch(SampleBatch {
                plugin_id: "example-rust".to_string(),
                samples: vec![Sample {
                    channel_path: "matrix.values".to_string(),
                    timestamp_unix_ns: 42,
                    sequence: 1,
                    value: Some(array),
                }],
            })),
        };

        let mut bytes = Vec::new();
        write_delimited(&mut bytes, &envelope).expect("write array envelope");

        let mut slice = bytes.as_slice();
        let decoded = read_delimited(&mut slice)
            .expect("read array envelope")
            .expect("non-empty frame");

        assert_eq!(decoded, envelope);
    }
}
