use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::watch;
use voxy_ipc::{ClientCommand, DaemonMessage, VoxyIpcClient};

/// High-level IPC bridge for the desktop UI.
/// Connects to the running VOXY Daemon over named pipe (`\\.\pipe\voxy-com-ipc`).
#[derive(Clone)]
pub struct IpcBridge {
    client: Arc<VoxyIpcClient>,
}

impl IpcBridge {
    pub fn new() -> Self {
        let client = Arc::new(VoxyIpcClient::new());
        client.start();
        Self { client }
    }

    /// Subscribe to broadcast messages emitted by the daemon.
    pub fn subscribe(&self) -> broadcast::Receiver<DaemonMessage> {
        self.client.subscribe()
    }

    /// Watch connection status to the daemon pipe.
    pub fn watch_connected(&self) -> watch::Receiver<bool> {
        self.client.watch_connected()
    }

    /// Send a command to the running daemon.
    pub async fn send_command(&self, cmd: ClientCommand) -> Result<(), String> {
        self.client.send_command(cmd).await
    }
}

impl Default for IpcBridge {
    fn default() -> Self {
        Self::new()
    }
}
