use std::io::{self, BufReader, BufWriter, Read, Write};

use anyhow::{Context, Result, bail};
use serde::de::DeserializeOwned;

use prismo_plugin_protocol::{
    ChannelDescriptor, DeclareChannels, Envelope, Health, Hello, Init, Log, Message, Sample,
    SampleBatch, Value, read_delimited, write_delimited,
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use serde::Deserialize;

    use super::build_stdio;
    use prismo_plugin_protocol::{Envelope, Init, Message, write_delimited};

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
}
