use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::watch;
use voxy_ipc::{ClientCommand, DaemonMessage, VoxyIpcClient, VoxyIpcServer};

/// High-level IPC bridge for the desktop UI.
/// Connects to the running VOXY Daemon over named pipe (`\\.\pipe\voxy-com-ipc`)
/// or hosts the embedded named pipe server directly in Single-Binary mode.
#[derive(Clone)]
pub struct IpcBridge {
    client: Arc<VoxyIpcClient>,
    server: Arc<VoxyIpcServer>,
}

impl IpcBridge {
    pub fn new() -> Self {
        let server = Arc::new(VoxyIpcServer::new());
        // Start server in background; if pipe already in use by daemon, server fails gracefully
        server.start();

        let client = Arc::new(VoxyIpcClient::new());
        client.start();
        Self { client, server }
    }

    /// Subscribe to broadcast messages emitted by the daemon or local server.
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

    /// Broadcast a daemon message from the embedded server to connected clients (e.g. overlay).
    pub fn broadcast(&self, msg: DaemonMessage) {
        self.server.broadcast(msg);
    }

    /// Access the embedded server directly.
    pub fn server(&self) -> Arc<VoxyIpcServer> {
        self.server.clone()
    }
}

impl Default for IpcBridge {
    fn default() -> Self {
        Self::new()
    }
}
