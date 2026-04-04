use std::io;

use prismo_plugin_sdk_rust::PluginIo;

struct PluginSessionState {
    inner: PluginIo<io::StdoutLock<'static>>,
}

#[diplomat::bridge]
#[diplomat::abi_rename = "prismo_{0}"]
pub mod ffi {
    use std::fmt::Write as _;

    use diplomat_runtime::DiplomatWrite;

    use super::PluginSessionState;
    use prismo_plugin_protocol::{ChannelDescriptor, Health, Sample, Value};
    use prismo_plugin_sdk_rust::{
        stdio, value_bool, value_bytes, value_float, value_integer, value_text,
    };

    #[diplomat::opaque]
    pub struct PluginSession(PluginSessionState);

    impl PluginSession {
        pub fn from_stdio() -> Option<Box<PluginSession>> {
            let inner = match stdio() {
                Ok(inner) => inner,
                Err(error) => {
                    eprintln!("failed to initialize plugin stdio session: {error}");
                    return None;
                }
            };
            Some(Box::new(PluginSession(PluginSessionState { inner })))
        }

        pub fn plugin_id(&self, write: &mut DiplomatWrite) {
            let _ = write.write_str(&self.0.inner.init().plugin_id);
            let _ = write.flush();
        }

        pub fn config_json(&self, write: &mut DiplomatWrite) {
            let _ = write.write_str(&self.0.inner.init().config_json);
            let _ = write.flush();
        }

        pub fn send_hello(&mut self, plugin_version: &str, language: &str) -> bool {
            let plugin_id = self.0.inner.init().plugin_id.clone();
            self.0
                .inner
                .send_hello(&plugin_id, plugin_version, language)
                .map_err(|error| eprintln!("failed to send hello: {error}"))
                .is_ok()
        }

        pub fn declare_channel(
            &mut self,
            channel_path: &str,
            display_name: &str,
            unit: &str,
            description: &str,
        ) -> bool {
            let plugin_id = self.0.inner.init().plugin_id.clone();
            self.0
                .inner
                .declare_channels(
                    &plugin_id,
                    vec![ChannelDescriptor {
                        channel_path: channel_path.to_string(),
                        display_name: display_name.to_string(),
                        unit: (!unit.is_empty()).then(|| unit.to_string()),
                        description: description.to_string(),
                    }],
                )
                .map_err(|error| eprintln!("failed to declare channel: {error}"))
                .is_ok()
        }

        pub fn send_bool_sample(
            &mut self,
            channel_path: &str,
            timestamp_unix_ns: u64,
            sequence: u64,
            value: bool,
        ) -> bool {
            self.send_sample(channel_path, timestamp_unix_ns, sequence, value_bool(value))
        }

        pub fn send_integer_sample(
            &mut self,
            channel_path: &str,
            timestamp_unix_ns: u64,
            sequence: u64,
            value: i64,
        ) -> bool {
            self.send_sample(
                channel_path,
                timestamp_unix_ns,
                sequence,
                value_integer(value),
            )
        }

        pub fn send_float_sample(
            &mut self,
            channel_path: &str,
            timestamp_unix_ns: u64,
            sequence: u64,
            value: f64,
        ) -> bool {
            self.send_sample(
                channel_path,
                timestamp_unix_ns,
                sequence,
                value_float(value),
            )
        }

        pub fn send_text_sample(
            &mut self,
            channel_path: &str,
            timestamp_unix_ns: u64,
            sequence: u64,
            value: &str,
        ) -> bool {
            self.send_sample(channel_path, timestamp_unix_ns, sequence, value_text(value))
        }

        pub fn send_bytes_sample(
            &mut self,
            channel_path: &str,
            timestamp_unix_ns: u64,
            sequence: u64,
            value: &[u8],
        ) -> bool {
            self.send_sample(
                channel_path,
                timestamp_unix_ns,
                sequence,
                value_bytes(value.to_vec()),
            )
        }

        pub fn send_health(
            &mut self,
            emitted_updates: u64,
            dropped_updates: u64,
            last_error: &str,
        ) -> bool {
            let plugin_id = self.0.inner.init().plugin_id.clone();
            self.0
                .inner
                .send_health(
                    &plugin_id,
                    Health {
                        plugin_id: plugin_id.clone(),
                        emitted_updates,
                        dropped_updates,
                        last_error: (!last_error.is_empty()).then(|| last_error.to_string()),
                    },
                )
                .map_err(|error| eprintln!("failed to send health: {error}"))
                .is_ok()
        }

        pub fn send_log(&mut self, level: &str, message: &str) -> bool {
            let plugin_id = self.0.inner.init().plugin_id.clone();
            self.0
                .inner
                .log(&plugin_id, level, message)
                .map_err(|error| eprintln!("failed to send log: {error}"))
                .is_ok()
        }
    }

    impl PluginSession {
        fn send_sample(
            &mut self,
            channel_path: &str,
            timestamp_unix_ns: u64,
            sequence: u64,
            value: Value,
        ) -> bool {
            let plugin_id = self.0.inner.init().plugin_id.clone();
            self.0
                .inner
                .send_samples(
                    &plugin_id,
                    vec![Sample {
                        channel_path: channel_path.to_string(),
                        timestamp_unix_ns,
                        sequence,
                        value: Some(value),
                    }],
                )
                .map_err(|error| eprintln!("failed to send sample: {error}"))
                .is_ok()
        }
    }
}
