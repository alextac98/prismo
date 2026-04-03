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
    AppConfig, Envelope, Health, Init, Message, PluginConfig, PluginManifest, RestartPolicy,
    ValueKind, load_plugin_manifest, read_delimited, write_delimited,
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
        config_path: &Path,
        tx: Sender<RuntimeEvent>,
        current_exe: PathBuf,
    ) -> Result<Self> {
        let config = prismo_plugin_protocol::load_app_config(config_path)?;
        Self::from_config(config, config_path, tx, current_exe)
    }

    fn from_config(
        config: AppConfig,
        config_path: &Path,
        tx: Sender<RuntimeEvent>,
        current_exe: PathBuf,
    ) -> Result<Self> {
        let config_dir = config_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let mut shutdown_senders = Vec::new();
        let mut join_handles = Vec::new();

        for plugin in config.plugins.into_iter().filter(|plugin| plugin.enabled) {
            let manifest_path = config_dir.join(&plugin.manifest);
            let manifest = load_plugin_manifest(&manifest_path)?;
            if manifest.plugin_id != plugin.plugin_id {
                bail!(
                    "plugin_id mismatch between config ({}) and manifest ({})",
                    plugin.plugin_id,
                    manifest.plugin_id
                );
            }

            let (shutdown_tx, shutdown_rx) = mpsc::channel();
            let thread_tx = tx.clone();
            let current_exe = current_exe.clone();
            let config_dir = config_dir.clone();

            let handle = thread::spawn(move || {
                if let Err(error) = supervise_plugin(
                    plugin,
                    manifest,
                    config_dir,
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
    plugin: PluginConfig,
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

                if should_restart(&plugin.restart, crashed) {
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
                if should_restart(&plugin.restart, true) {
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
    plugin: &PluginConfig,
    manifest: &PluginManifest,
    config_dir: &Path,
    current_exe: &Path,
    tx: Sender<RuntimeEvent>,
    shutdown_rx: &Receiver<SupervisorCommand>,
) -> Result<LoopControl> {
    let command = resolve_command(&manifest.entrypoint.argv, config_dir, current_exe)?;
    let mut child = spawn_child(&command)?;

    let mut stdin = BufWriter::new(child.stdin.take().context("spawned plugin missing stdin")?);

    let init = Init {
        protocol_version: PROTOCOL_VERSION,
        instance_id: plugin.plugin_id.clone(),
        plugin_id: plugin.plugin_id.clone(),
        config_json: serde_json::to_string(&plugin.config_json())
            .context("failed to encode plugin config json")?,
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

fn resolve_command(argv: &[String], config_dir: &Path, current_exe: &Path) -> Result<Vec<String>> {
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
                config_dir.join(arg).display().to_string()
            } else {
                arg.clone()
            }
        })
        .collect::<Vec<_>>())
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
    plugin: &PluginConfig,
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

fn should_restart(policy: &RestartPolicy, crashed: bool) -> bool {
    match policy {
        RestartPolicy::Never => false,
        RestartPolicy::Always => true,
        RestartPolicy::OnFailure => crashed,
    }
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
