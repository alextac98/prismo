use anyhow::Result;
use telemetry_core::TelemetryUpdate;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub type PluginHandle = JoinHandle<Result<()>>;

pub trait SourcePlugin: Send + 'static {
    fn id(&self) -> &'static str;
    fn spawn(self: Box<Self>, tx: mpsc::Sender<TelemetryUpdate>) -> PluginHandle;
}
