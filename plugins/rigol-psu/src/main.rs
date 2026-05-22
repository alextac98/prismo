use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use prismo_plugin_sdk_rust::{channel_descriptor, health, sample, stdio, value_float, value_text};
use serde::Deserialize;

const PLUGIN_ID: &str = "rigol-psu";

#[derive(Debug, Deserialize)]
struct RigolConfig {
    #[serde(default)]
    ip_address: String,
    #[serde(default = "default_port")]
    port: u16,
    #[serde(default = "default_sample_rate_hz")]
    sample_rate_hz: f64,
    #[serde(default = "default_timeout_ms")]
    timeout_ms: u64,
}

impl Default for RigolConfig {
    fn default() -> Self {
        Self {
            ip_address: String::new(),
            port: default_port(),
            sample_rate_hz: default_sample_rate_hz(),
            timeout_ms: default_timeout_ms(),
        }
    }
}

struct RigolConnection {
    reader: BufReader<TcpStream>,
}

impl RigolConnection {
    fn connect(config: &RigolConfig) -> Result<Self> {
        if config.ip_address.trim().is_empty() {
            bail!("missing config.ip_address");
        }

        let timeout = Duration::from_millis(config.timeout_ms);
        let address = format!("{}:{}", config.ip_address, config.port);
        let socket_addr = address
            .to_socket_addrs()
            .with_context(|| format!("failed to resolve {}", address))?
            .next()
            .ok_or_else(|| anyhow!("{} resolved to no socket addresses", address))?;
        let stream = TcpStream::connect_timeout(&socket_addr, timeout)
            .with_context(|| format!("failed to connect to {}", socket_addr))?;
        stream
            .set_read_timeout(Some(timeout))
            .context("failed to set read timeout")?;
        stream
            .set_write_timeout(Some(timeout))
            .context("failed to set write timeout")?;

        Ok(Self {
            reader: BufReader::new(stream),
        })
    }

    fn query(&mut self, command: &str) -> Result<String> {
        let stream = self.reader.get_mut();
        stream
            .write_all(command.as_bytes())
            .with_context(|| format!("failed to write SCPI command {}", command))?;
        stream
            .write_all(b"\n")
            .context("failed to write SCPI command terminator")?;
        stream.flush().context("failed to flush SCPI command")?;

        let mut response = String::new();
        self.reader
            .read_line(&mut response)
            .with_context(|| format!("failed to read SCPI response for {}", command))?;
        if response.is_empty() {
            bail!("empty SCPI response for {}", command);
        }
        Ok(response.trim().to_string())
    }

    fn sample(&mut self) -> Result<(f64, f64)> {
        let response = self.query("MEAS:VOLT?;:MEAS:CURR?")?;
        let (voltage, current) = response
            .split_once(';')
            .ok_or_else(|| anyhow!("unexpected measurement response: {}", response))?;
        Ok((
            voltage
                .trim()
                .parse::<f64>()
                .with_context(|| format!("invalid voltage response: {}", voltage))?,
            current
                .trim()
                .parse::<f64>()
                .with_context(|| format!("invalid current response: {}", current))?,
        ))
    }
}

fn main() -> Result<()> {
    let mut io = stdio()?;
    let config = io.config::<RigolConfig>().unwrap_or_default();
    let sample_period = sample_period(config.sample_rate_hz);

    io.send_hello(PLUGIN_ID, env!("CARGO_PKG_VERSION"), "rust")?;
    io.declare_channels(
        PLUGIN_ID,
        vec![
            channel_descriptor(
                "power.voltage",
                "Voltage",
                Some("V"),
                "Measured output voltage",
            ),
            channel_descriptor(
                "power.current",
                "Current",
                Some("A"),
                "Measured output current",
            ),
            channel_descriptor("power.power", "Power", Some("W"), "Computed output power"),
            channel_descriptor(
                "instrument.id",
                "Instrument",
                None::<&str>,
                "Instrument identity",
            ),
        ],
    )?;

    let mut sequence = 0_u64;
    let mut emitted_updates = 0_u64;
    let mut dropped_updates = 0_u64;

    loop {
        let last_error = match RigolConnection::connect(&config) {
            Ok(mut rigol) => {
                let mut last_error = match rigol.query("*IDN?") {
                    Ok(idn) => {
                        sequence += 1;
                        emitted_updates += 1;
                        io.log(PLUGIN_ID, "info", &format!("connected to {}", idn))?;
                        io.send_samples(
                            PLUGIN_ID,
                            vec![sample(
                                "instrument.id",
                                unix_timestamp_ns(),
                                sequence,
                                value_text(idn),
                            )],
                        )?;
                        None
                    }
                    Err(error) => {
                        dropped_updates += 1;
                        Some(error.to_string())
                    }
                };

                while last_error.is_none() {
                    thread::sleep(sample_period);
                    match rigol.sample() {
                        Ok((voltage, current)) => {
                            sequence += 1;
                            emitted_updates += 1;
                            let timestamp = unix_timestamp_ns();
                            io.send_samples(
                                PLUGIN_ID,
                                vec![
                                    sample(
                                        "power.voltage",
                                        timestamp,
                                        sequence,
                                        value_float(voltage),
                                    ),
                                    sample(
                                        "power.current",
                                        timestamp,
                                        sequence,
                                        value_float(current),
                                    ),
                                    sample(
                                        "power.power",
                                        timestamp,
                                        sequence,
                                        value_float(voltage * current),
                                    ),
                                ],
                            )?;
                            io.send_health(
                                PLUGIN_ID,
                                health(PLUGIN_ID, emitted_updates, dropped_updates, None::<String>),
                            )?;
                        }
                        Err(error) => {
                            dropped_updates += 1;
                            last_error = Some(error.to_string());
                        }
                    }
                }

                last_error
            }
            Err(error) => {
                dropped_updates += 1;
                Some(error.to_string())
            }
        };

        io.send_health(
            PLUGIN_ID,
            health(
                PLUGIN_ID,
                emitted_updates,
                dropped_updates,
                last_error.clone(),
            ),
        )?;
        thread::sleep(Duration::from_secs(2));
    }
}

fn sample_period(sample_rate_hz: f64) -> Duration {
    let bounded_rate = if sample_rate_hz.is_finite() {
        sample_rate_hz.clamp(0.2, 200.0)
    } else {
        default_sample_rate_hz()
    };
    Duration::from_secs_f64(1.0 / bounded_rate)
}

fn unix_timestamp_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

fn default_port() -> u16 {
    5555
}

fn default_sample_rate_hz() -> f64 {
    20.0
}

fn default_timeout_ms() -> u64 {
    5000
}
