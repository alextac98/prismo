use std::io::{BufRead, BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use prismo_core::{
    ChannelDescriptor as CoreChannelDescriptor, ChannelSample, ChannelValue, PluginHealth,
    PluginRuntimeState, PluginStatusUpdate, RuntimeEvent, TelemetryUpdate,
};
use prismo_plugin_protocol::{
    DiscoveredPlugin, Envelope, Health, Init, Message, PluginManifest, ValueKind,
    default_plugin_dir, discover_plugins, read_delimited, write_delimited,
};

const PROTOCOL_VERSION: u32 = 1;
const SELF_ENTRYPOINT: &str = "__PRISMO_SELF__";
const RESTART_DELAY: Duration = Duration::from_secs(1);

pub struct PluginHost {
    shutdown_senders: Vec<Sender<SupervisorCommand>>,
    join_handles: Vec<thread::JoinHandle<()>>,
}

impl PluginHost {
    pub fn start(
        tx: Sender<RuntimeEvent>,
        current_exe: PathBuf,
        plugins_dir: Option<PathBuf>,
    ) -> Result<Self> {
        let resolved_plugins = resolve_plugins(&current_exe, plugins_dir)?;
        let mut shutdown_senders = Vec::new();
        let mut join_handles = Vec::new();

        for resolved in resolved_plugins {
            let (shutdown_tx, shutdown_rx) = mpsc::channel();
            let thread_tx = tx.clone();
            let current_exe = current_exe.clone();

            let handle = thread::spawn(move || {
                if let Err(error) = supervise_plugin(
                    resolved.runtime,
                    resolved.manifest,
                    resolved.manifest_dir,
                    current_exe,
                    thread_tx.clone(),
                    shutdown_rx,
                ) {
                    let _ = thread_tx.send(RuntimeEvent::PluginStatus(PluginStatusUpdate {
                        plugin_id: error
                            .downcast_ref::<PluginError>()
                            .map(|error| error.plugin_id.clone())
                            .unwrap_or_else(|| "unknown".to_string()),
                        state: PluginRuntimeState::Crashed,
                        restart_count: 0,
                        message: Some(error.to_string()),
                    }));
                }
            });

            shutdown_senders.push(shutdown_tx);
            join_handles.push(handle);
        }

        Ok(Self {
            shutdown_senders,
            join_handles,
        })
    }

    pub fn shutdown(self) {
        for sender in &self.shutdown_senders {
            let _ = sender.send(SupervisorCommand::Shutdown);
        }
        for handle in self.join_handles {
            let _ = handle.join();
        }
    }
}

#[derive(Clone)]
struct ResolvedPlugin {
    runtime: PluginRuntime,
    manifest: PluginManifest,
    manifest_dir: PathBuf,
}

#[derive(Clone)]
struct PluginRuntime {
    plugin_id: String,
}

enum SupervisorCommand {
    Shutdown,
}

enum ReaderEvent {
    Envelope(Envelope),
    StdErr(String),
    StdoutClosed,
    ReadError(String),
}

#[derive(Debug)]
struct PluginError {
    plugin_id: String,
    message: String,
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.plugin_id, self.message)
    }
}

impl std::error::Error for PluginError {}

fn supervise_plugin(
    plugin: PluginRuntime,
    manifest: PluginManifest,
    config_dir: PathBuf,
    current_exe: PathBuf,
    tx: Sender<RuntimeEvent>,
    shutdown_rx: Receiver<SupervisorCommand>,
) -> Result<()> {
    let mut restart_count = 0_u64;
    let mut stop_requested = false;

    while !stop_requested {
        let state = if restart_count == 0 {
            PluginRuntimeState::Starting
        } else {
            PluginRuntimeState::Restarting
        };
        send_status(&tx, &plugin.plugin_id, state, restart_count, None);

        let start_result = start_plugin_process(
            &plugin,
            &manifest,
            &config_dir,
            &current_exe,
            tx.clone(),
            &shutdown_rx,
        );

        match start_result {
            Ok(LoopControl::Stopped) => {
                stop_requested = true;
                send_status(
                    &tx,
                    &plugin.plugin_id,
                    PluginRuntimeState::Stopped,
                    restart_count,
                    None,
                );
            }
            Ok(LoopControl::Exited(status)) => {
                let crashed = !status.success();
                let message = Some(format_exit_status(status));
                let state = if crashed {
                    PluginRuntimeState::Crashed
                } else {
                    PluginRuntimeState::Stopped
                };
                send_status(
                    &tx,
                    &plugin.plugin_id,
                    state,
                    restart_count,
                    message.clone(),
                );

                if should_restart(crashed) {
                    restart_count += 1;
                    thread::sleep(RESTART_DELAY);
                } else {
                    break;
                }
            }
            Err(error) => {
                let message = error.to_string();
                send_status(
                    &tx,
                    &plugin.plugin_id,
                    PluginRuntimeState::Crashed,
                    restart_count,
                    Some(message.clone()),
                );
                if should_restart(true) {
                    restart_count += 1;
                    thread::sleep(RESTART_DELAY);
                } else {
                    return Err(Box::new(PluginError {
                        plugin_id: plugin.plugin_id.clone(),
                        message,
                    })
                    .into());
                }
            }
        }
    }

    Ok(())
}

enum LoopControl {
    Stopped,
    Exited(ExitStatus),
}

fn start_plugin_process(
    plugin: &PluginRuntime,
    manifest: &PluginManifest,
    manifest_dir: &Path,
    current_exe: &Path,
    tx: Sender<RuntimeEvent>,
    shutdown_rx: &Receiver<SupervisorCommand>,
) -> Result<LoopControl> {
    let command = resolve_command(&manifest.entrypoint.argv, manifest_dir, current_exe)?;
    let mut child = spawn_child(&command)?;

    let mut stdin = BufWriter::new(child.stdin.take().context("spawned plugin missing stdin")?);

    let init = Init {
        protocol_version: PROTOCOL_VERSION,
        instance_id: plugin.plugin_id.clone(),
        plugin_id: plugin.plugin_id.clone(),
        config_json: "{}".to_string(),
    };
    write_delimited(
        &mut stdin,
        &Envelope {
            message: Some(Message::Init(init)),
        },
    )?;

    let stdout = child
        .stdout
        .take()
        .context("spawned plugin missing stdout")?;
    let stderr = child
        .stderr
        .take()
        .context("spawned plugin missing stderr")?;

    let (reader_tx, reader_rx) = mpsc::channel();
    spawn_stdout_reader(stdout, reader_tx.clone());
    spawn_stderr_reader(stderr, reader_tx);

    let mut saw_hello = false;
    let mut saw_descriptor = false;
    let mut last_health = PluginHealth::default();

    loop {
        match shutdown_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(SupervisorCommand::Shutdown) => {
                let _ = write_delimited(
                    &mut stdin,
                    &Envelope {
                        message: Some(Message::Shutdown(prismo_plugin_protocol::Shutdown {
                            reason: "host shutdown".to_string(),
                        })),
                    },
                );
                let _ = child.kill();
                let status = child.wait().context("failed to wait for stopped plugin")?;
                return Ok(LoopControl::Stopped).map(|_| {
                    let _ = status;
                    LoopControl::Stopped
                });
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {}
        }

        match reader_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(ReaderEvent::Envelope(envelope)) => match envelope.message {
                Some(Message::Hello(hello)) => {
                    validate_hello(plugin, manifest, &hello)?;
                    saw_hello = true;
                    send_status(
                        &tx,
                        &plugin.plugin_id,
                        PluginRuntimeState::Running,
                        0,
                        Some(format!(
                            "{} {} ({})",
                            manifest.display_name, hello.plugin_version, hello.language
                        )),
                    );
                }
                Some(Message::DeclareChannels(declare)) => {
                    validate_plugin_id(&plugin.plugin_id, &declare.plugin_id)?;
                    saw_descriptor = true;
                    tx.send(RuntimeEvent::Telemetry(TelemetryUpdate {
                        plugin_id: plugin.plugin_id.clone(),
                        descriptors: declare
                            .channels
                            .into_iter()
                            .map(|channel| CoreChannelDescriptor {
                                path: channel.channel_path,
                                display_name: channel.display_name,
                                unit: channel.unit,
                                description: channel.description,
                            })
                            .collect::<Vec<_>>(),
                        samples: Vec::new(),
                        health: None,
                    }))
                    .ok();
                }
                Some(Message::SampleBatch(batch)) => {
                    validate_plugin_id(&plugin.plugin_id, &batch.plugin_id)?;
                    let observed_at = Instant::now();
                    tx.send(RuntimeEvent::Telemetry(TelemetryUpdate {
                        plugin_id: plugin.plugin_id.clone(),
                        descriptors: Vec::new(),
                        samples: batch
                            .samples
                            .into_iter()
                            .map(|sample| {
                                Ok(ChannelSample {
                                    path: sample.channel_path,
                                    value: decode_value(sample.value)?,
                                    observed_at,
                                    source_timestamp_unix_ns: sample.timestamp_unix_ns,
                                    sequence: sample.sequence,
                                })
                            })
                            .collect::<Result<Vec<_>>>()?,
                        health: None,
                    }))
                    .ok();
                }
                Some(Message::Health(Health {
                    plugin_id,
                    emitted_updates,
                    dropped_updates,
                    last_error,
                })) => {
                    validate_plugin_id(&plugin.plugin_id, &plugin_id)?;
                    last_health = PluginHealth {
                        emitted_updates,
                        dropped_updates,
                        last_error,
                    };
                    tx.send(RuntimeEvent::Telemetry(TelemetryUpdate {
                        plugin_id: plugin.plugin_id.clone(),
                        descriptors: Vec::new(),
                        samples: Vec::new(),
                        health: Some(last_health.clone()),
                    }))
                    .ok();
                }
                Some(Message::Log(log)) => {
                    validate_plugin_id(&plugin.plugin_id, &log.plugin_id)?;
                    send_status(
                        &tx,
                        &plugin.plugin_id,
                        PluginRuntimeState::Running,
                        0,
                        Some(format!("{}: {}", log.level, log.message)),
                    );
                }
                Some(Message::Shutdown(_)) | Some(Message::Init(_)) | None => {}
            },
            Ok(ReaderEvent::StdErr(line)) => {
                send_status(
                    &tx,
                    &plugin.plugin_id,
                    PluginRuntimeState::Running,
                    0,
                    Some(format!("stderr: {}", line)),
                );
            }
            Ok(ReaderEvent::StdoutClosed) => {
                let status = child.wait().context("failed to wait for plugin exit")?;
                if saw_hello && !saw_descriptor && last_health.emitted_updates > 0 {
                    send_status(
                        &tx,
                        &plugin.plugin_id,
                        PluginRuntimeState::Crashed,
                        0,
                        Some("plugin exited before declaring channels".to_string()),
                    );
                }
                return Ok(LoopControl::Exited(status));
            }
            Ok(ReaderEvent::ReadError(error)) => {
                let _ = child.kill();
                let _ = child.wait();
                bail!("protocol read error: {}", error);
            }
            Err(RecvTimeoutError::Timeout) => {
                if let Some(status) = child.try_wait().context("failed to poll plugin child")? {
                    return Ok(LoopControl::Exited(status));
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                let status = child
                    .wait()
                    .context("failed to wait for disconnected reader")?;
                return Ok(LoopControl::Exited(status));
            }
        }
    }
}

fn resolve_command(
    argv: &[String],
    manifest_dir: &Path,
    current_exe: &Path,
) -> Result<Vec<String>> {
    if argv.is_empty() {
        bail!("plugin manifest entrypoint argv cannot be empty");
    }

    Ok(argv
        .iter()
        .enumerate()
        .map(|(index, arg)| {
            if index == 0 && arg == SELF_ENTRYPOINT {
                current_exe.display().to_string()
            } else if index == 0 {
                manifest_dir.join(arg).display().to_string()
            } else {
                arg.clone()
            }
        })
        .collect::<Vec<_>>())
}

fn resolve_plugins(
    current_exe: &Path,
    plugins_dir: Option<PathBuf>,
) -> Result<Vec<ResolvedPlugin>> {
    let search_dir = match plugins_dir {
        Some(path) => path,
        None => default_plugin_dir(current_exe)?,
    };
    let mut resolved = discover_plugins(&search_dir)?
        .into_iter()
        .map(resolved_from_discovery)
        .collect::<Vec<_>>();
    resolved.sort_by(|left, right| left.runtime.plugin_id.cmp(&right.runtime.plugin_id));
    Ok(resolved)
}

fn resolved_from_discovery(plugin: DiscoveredPlugin) -> ResolvedPlugin {
    let plugin_id = plugin.manifest.plugin_id.clone();
    ResolvedPlugin {
        runtime: PluginRuntime { plugin_id },
        manifest: plugin.manifest,
        manifest_dir: plugin
            .manifest_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(".")),
    }
}

fn spawn_child(command: &[String]) -> Result<Child> {
    let (program, args) = command
        .split_first()
        .ok_or_else(|| anyhow!("missing plugin command"))?;
    Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to spawn plugin command: {}", program))
}

fn spawn_stdout_reader(stdout: impl std::io::Read + Send + 'static, tx: Sender<ReaderEvent>) {
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            match read_delimited(&mut reader) {
                Ok(Some(envelope)) => {
                    if tx.send(ReaderEvent::Envelope(envelope)).is_err() {
                        break;
                    }
                }
                Ok(None) => {
                    let _ = tx.send(ReaderEvent::StdoutClosed);
                    break;
                }
                Err(error) => {
                    let _ = tx.send(ReaderEvent::ReadError(error.to_string()));
                    break;
                }
            }
        }
    });
}

fn spawn_stderr_reader(stderr: impl std::io::Read + Send + 'static, tx: Sender<ReaderEvent>) {
    thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if tx.send(ReaderEvent::StdErr(line)).is_err() {
                break;
            }
        }
    });
}

fn validate_hello(
    plugin: &PluginRuntime,
    manifest: &PluginManifest,
    hello: &prismo_plugin_protocol::Hello,
) -> Result<()> {
    validate_plugin_id(&plugin.plugin_id, &hello.plugin_id)?;
    if hello.protocol_version != PROTOCOL_VERSION {
        bail!(
            "plugin {} speaks protocol version {}, host requires {}",
            plugin.plugin_id,
            hello.protocol_version,
            PROTOCOL_VERSION
        );
    }
    if manifest.protocol_version != hello.protocol_version {
        bail!(
            "manifest protocol version {} does not match plugin hello {}",
            manifest.protocol_version,
            hello.protocol_version
        );
    }
    Ok(())
}

fn validate_plugin_id(expected: &str, actual: &str) -> Result<()> {
    if expected == actual {
        Ok(())
    } else {
        bail!("expected plugin_id {}, got {}", expected, actual)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{resolve_command, resolve_plugins};

    #[test]
    fn resolves_entrypoint_relative_to_manifest_dir() {
        let command = resolve_command(
            &[String::from("./bin/example")],
            Path::new("/tmp/prismo/plugins/example"),
            Path::new("/tmp/prismo/bin/prismo"),
        )
        .expect("resolve command");

        assert_eq!(command, vec!["/tmp/prismo/plugins/example/./bin/example"]);
    }

    #[test]
    fn discovers_plugins_relative_to_current_executable() {
        let root = unique_temp_path("prismo-host-discovery");
        let plugin_dir = root.join("plugins").join("test-plugin");
        fs::create_dir_all(&plugin_dir).expect("create plugin dir");
        fs::write(
            plugin_dir.join("prismo-plugin.toml"),
            r#"
schema_version = 1
plugin_id = "test-plugin"
display_name = "Test Plugin"
plugin_version = "0.1.0"
protocol_version = 1
language = "rust"

[entrypoint]
argv = ["./bin/test-plugin"]
"#,
        )
        .expect("write manifest");

        let resolved = resolve_plugins(&root.join("prismo"), None).expect("resolve plugins");

        assert!(resolved.iter().any(|plugin| {
            plugin.runtime.plugin_id == "test-plugin"
                && plugin.manifest_dir == plugin_dir
                && plugin.manifest.plugin_id == "test-plugin"
        }));
    }

    fn unique_temp_path(prefix: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        path.push(format!("{}-{}-{}", prefix, std::process::id(), nanos));
        path
    }
}

fn decode_value(value: Option<prismo_plugin_protocol::Value>) -> Result<ChannelValue> {
    match value.and_then(|value| value.kind) {
        Some(ValueKind::BoolValue(value)) => Ok(ChannelValue::Bool(value)),
        Some(ValueKind::IntegerValue(value)) => Ok(ChannelValue::Integer(value)),
        Some(ValueKind::FloatValue(value)) => Ok(ChannelValue::Float(value)),
        Some(ValueKind::TextValue(value)) => Ok(ChannelValue::Text(value)),
        Some(ValueKind::BytesValue(value)) => Ok(ChannelValue::Bytes(value)),
        None => bail!("sample missing value"),
    }
}

fn should_restart(crashed: bool) -> bool {
    crashed
}

fn send_status(
    tx: &Sender<RuntimeEvent>,
    plugin_id: &str,
    state: PluginRuntimeState,
    restart_count: u64,
    message: Option<String>,
) {
    let _ = tx.send(RuntimeEvent::PluginStatus(PluginStatusUpdate {
        plugin_id: plugin_id.to_string(),
        state,
        restart_count,
        message,
    }));
}

fn format_exit_status(status: ExitStatus) -> String {
    if let Some(code) = status.code() {
        format!("exited with status {}", code)
    } else {
        "terminated by signal".to_string()
    }
}
