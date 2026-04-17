use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use prismo_example_rust::ExampleTelemetrySource;
use prismo_plugin_sdk_rust::{
    ChannelDescriptor, Health, Sample, channel_descriptor, health, sample, stdio, value_bool,
    value_bytes, value_float, value_integer, value_text,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ExamplePluginConfig {
    #[serde(default = "default_tick_ms")]
    tick_ms: u64,
}

impl Default for ExamplePluginConfig {
    fn default() -> Self {
        Self {
            tick_ms: default_tick_ms(),
        }
    }
}

fn main() -> Result<()> {
    let mut io = stdio()?;
    let config = io.config::<ExamplePluginConfig>().unwrap_or_default();
    let mut source = ExampleTelemetrySource::new(Duration::from_millis(config.tick_ms));
    let descriptors = source
        .descriptors()
        .iter()
        .map(to_protocol_descriptor)
        .collect::<Vec<_>>();

    io.send_hello(source.plugin_id(), env!("CARGO_PKG_VERSION"), "rust")?;
    io.declare_channels(source.plugin_id(), descriptors)?;

    loop {
        thread::sleep(source.period());
        let update = source.next_update(Instant::now());
        let samples = update
            .samples
            .iter()
            .map(to_protocol_sample)
            .collect::<Vec<_>>();
        io.send_samples(source.plugin_id(), samples)?;
        if let Some(plugin_health) = update.health.as_ref() {
            io.send_health(
                source.plugin_id(),
                to_protocol_health(source.plugin_id(), plugin_health),
            )?;
        }
    }
}

fn to_protocol_descriptor(descriptor: &prismo_core::ChannelDescriptor) -> ChannelDescriptor {
    channel_descriptor(
        descriptor.path.clone(),
        descriptor.display_name.clone(),
        descriptor.unit.clone(),
        descriptor.description.clone(),
    )
}

fn to_protocol_sample(sample_value: &prismo_core::ChannelSample) -> Sample {
    let value = match &sample_value.value {
        prismo_core::ChannelValue::Bool(value) => value_bool(*value),
        prismo_core::ChannelValue::Integer(value) => value_integer(*value),
        prismo_core::ChannelValue::Float(value) => value_float(*value),
        prismo_core::ChannelValue::Text(value) => value_text(value.clone()),
        prismo_core::ChannelValue::Bytes(value) => value_bytes(value.clone()),
    };
    sample(
        sample_value.path.clone(),
        sample_value.source_timestamp_unix_ns,
        sample_value.sequence,
        value,
    )
}

fn to_protocol_health(plugin_id: &str, plugin_health: &prismo_core::PluginHealth) -> Health {
    health(
        plugin_id,
        plugin_health.emitted_updates,
        plugin_health.dropped_updates,
        plugin_health.last_error.clone(),
    )
}

fn default_tick_ms() -> u64 {
    200
}
