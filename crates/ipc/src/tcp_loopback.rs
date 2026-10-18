//! Loopback TCP IPC transport for the OSMOO Pipecat Python sidecar.
//!
//! The primary desktop transport remains the Windows named pipe
//! (`\\.\pipe\voxy-com-ipc`, local-only, `reject_remote_clients`). This module
//! provides the *sidecar-only* loopback TCP transport the Pipecat Python process
//! speaks, because the sidecar is a cross-platform Python process and cannot
//! reliably open a Windows named pipe from asyncio.
//!
//! Security invariants (fail-closed):
//! - The listener binds 127.0.0.1 only, never a routable address.
//! - The listener refuses to start unless an auth token is configured:
//!   an unauthenticated network voice listener is never created.
//! - Every connection must complete a `ClientHandshake` *before* any command.
//! - Privileged commands are rejected unless the handshake presented the token
//!   (constant-time equality, same policy as the named-pipe server).
//! - Frames are length-delimited (4-byte BE) with the same 1 MB cap as the pipe.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tracing::{debug, error, info, warn};

use crate::named_pipe::{read_framed_message, write_framed_message, DaemonStateSnapshot};
use crate::voxy_protocol::{ClientCommand, DaemonMessage, IpcEnvelope};

/// Constant-time byte slice comparison to mitigate timing attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut res = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        res |= x ^ y;
    }
    res == 0
}

/// Loopback TCP IPC server dedicated to the Pipecat Python sidecar.
pub struct TcpLoopbackServer {
    host: String,
    port: u16,
    auth_secret: Option<String>,
    snapshot: Arc<tokio::sync::RwLock<DaemonStateSnapshot>>,
    broadcast_tx: tokio::sync::broadcast::Sender<IpcEnvelope<DaemonMessage>>,
    command_tx: tokio::sync::mpsc::Sender<ClientCommand>,
    is_running: Arc<AtomicBool>,
}

impl TcpLoopbackServer {
    /// Create a sidecar TCP server. Fails closed when no auth token is
    /// configured: an unauthenticated network voice listener is never started.
    pub fn new(
        host: impl Into<String>,
        port: u16,
        auth_secret: Option<String>,
        broadcast_tx: tokio::sync::broadcast::Sender<IpcEnvelope<DaemonMessage>>,
        command_tx: tokio::sync::mpsc::Sender<ClientCommand>,
    ) -> Result<Self, String> {
        if auth_secret.is_none() {
            return Err(
                "TCP loopback IPC listener requires an auth token (VOXY_IPC_AUTH_TOKEN); \
                 refusing to start an unauthenticated network voice listener"
                    .to_string(),
            );
        }
        let host_string = host.into();
        if host_string != "127.0.0.1" && host_string != "localhost" && !host_string.is_empty() {
            return Err(format!(
                "TCP loopback IPC listener must bind a loopback address, got '{host_string}'"
            ));
        }
        let bind_host = if host_string == "localhost" {
            "127.0.0.1".to_string()
        } else {
            host_string
        };
        Ok(Self {
            host: bind_host,
            port,
            auth_secret,
            snapshot: Arc::new(tokio::sync::RwLock::new(DaemonStateSnapshot::default())),
            broadcast_tx,
            command_tx,
            is_running: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub fn bound_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Share the daemon state snapshot so sidecar connections receive live state.
    pub fn set_snapshot(&mut self, snapshot: Arc<tokio::sync::RwLock<DaemonStateSnapshot>>) {
        self.snapshot = snapshot;
    }

    /// Start the accept loop. Returns the actual bound port (useful when the
    /// caller passed port 0 to let the OS pick one).
    pub async fn start(&self) -> Result<u16, String> {
        if self.is_running.swap(true, Ordering::SeqCst) {
            return Ok(self.port);
        }
        let listener = TcpListener::bind((self.host.as_str(), self.port))
            .await
            .map_err(|e| format!("Failed to bind loopback IPC {}: {e}", self.bound_addr()))?;
        let bound_port = listener
            .local_addr()
            .map_err(|e| format!("Failed to read bound IPC port: {e}"))?
            .port();

        info!(
            addr = %self.bound_addr(),
            actual_port = bound_port,
            "VOXY sidecar loopback TCP IPC listening (fail-closed auth)"
        );

        let listener = Arc::new(listener);
        let auth = self.auth_secret.clone();
        let snapshot = Arc::clone(&self.snapshot);
        let broadcast_tx = self.broadcast_tx.clone();
        let command_tx = self.command_tx.clone();
        let running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            while running.load(Ordering::SeqCst) {
                let accepted = tokio::select! {
                    r = listener.accept() => r,
                    _ = tokio::time::sleep(std::time::Duration::from_millis(250)) => {
                        if !running.load(Ordering::SeqCst) {
                            break;
                        }
                        continue;
                    }
                };
                match accepted {
                    Ok((stream, peer)) => {
                        if !peer.ip().is_loopback() {
                            warn!(%peer, "Rejecting non-loopback IPC connection attempt");
                            continue;
                        }
                        let auth = auth.clone();
                        let snapshot = Arc::clone(&snapshot);
                        let broadcast_tx = broadcast_tx.clone();
                        let command_tx = command_tx.clone();
                        tokio::spawn(async move {
                            handle_sidecar_connection(
                                stream,
                                snapshot,
                                broadcast_tx,
                                command_tx,
                                auth,
                            )
                            .await;
                        });
                    }
                    Err(e) => {
                        error!(error = %e, "Loopback IPC accept failed; retrying");
                        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                    }
                }
            }
            debug!("Loopback sidecar IPC accept loop ended");
        });

        Ok(bound_port)
    }

    /// Stop the accept loop.
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
        info!("VOXY sidecar loopback TCP IPC stopped");
    }
}

async fn handle_sidecar_connection<S>(
    stream: S,
    snapshot: Arc<tokio::sync::RwLock<DaemonStateSnapshot>>,
    broadcast_tx: tokio::sync::broadcast::Sender<IpcEnvelope<DaemonMessage>>,
    command_tx: tokio::sync::mpsc::Sender<ClientCommand>,
    auth_secret: Option<String>,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    // Handshake-first policy: nothing (not even the snapshot) is served until a
    // valid ClientHandshake arrives.
    let (mut reader, mut writer) = tokio::io::split(stream);

    let handshake: IpcEnvelope<ClientCommand> = match read_framed_message(&mut reader).await {
        Ok(h) => h,
        Err(e) => {
            warn!(error = %e, "Sidecar TCP IPC client failed handshake read; dropping");
            return;
        }
    };

    let (client_name, authenticated) = match &handshake.payload {
        ClientCommand::ClientHandshake {
            client_name,
            auth_token,
            ..
        } => {
            let ok = matches!((&auth_secret, auth_token),
                (Some(expected), Some(token)) if constant_time_eq(token.as_bytes(), expected.as_bytes()));
            (client_name.clone(), ok)
        }
        _ => {
            warn!("Sidecar TCP IPC first frame was not a ClientHandshake; dropping");
            return;
        }
    };

    if !authenticated {
        warn!(%client_name, "Sidecar handshake rejected: invalid or missing auth token; dropping");
        return;
    }

    info!(client = %client_name, "Sidecar IPC authenticated and connected");

    // Serve snapshot immediately after successful auth.
    let snap = snapshot.read().await.clone();
    if let Err(e) =
        write_framed_message(&mut writer, &IpcEnvelope::new(snap.to_snapshot_message())).await
    {
        debug!(error = %e, "Failed to send snapshot to sidecar");
        return;
    }

    let mut broadcast_rx = broadcast_tx.subscribe();

    // Writer task: daemon → sidecar event stream.
    let writer_task = tokio::spawn(async move {
        loop {
            match broadcast_rx.recv().await {
                Ok(msg) => {
                    if let Err(e) = write_framed_message(&mut writer, &msg).await {
                        debug!(error = %e, "Sidecar IPC write failed; closing writer");
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    warn!(missed = n, "Sidecar IPC lagged behind broadcast stream");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    // Reader task: sidecar → daemon commands (auth pre-verified).
    let reader_task = tokio::spawn(async move {
        loop {
            match read_framed_message::<_, ClientCommand>(&mut reader).await {
                Ok(envelope) => {
                    if envelope.payload.is_privileged() && !authenticated {
                        warn!(cmd = ?envelope.payload, "Privileged sidecar command rejected");
                        continue;
                    }
                    if command_tx.send(envelope.payload).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    debug!(error = %e, "Sidecar IPC read ended");
                    break;
                }
            }
        }
    });

    tokio::select! {
        _ = writer_task => {},
        _ = reader_task => {},
    };
    info!("Sidecar IPC disconnected");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::named_pipe::read_framed_message;
    use crate::voxy_protocol::encode_ipc_frame;

    fn handshake_frame(token: Option<&str>) -> Vec<u8> {
        let cmd = ClientCommand::ClientHandshake {
            client_name: "osmoo-pipecat-sidecar".to_string(),
            client_version: "test".to_string(),
            auth_token: token.map(String::from),
            nonce: None,
            timestamp_ms: None,
        };
        encode_ipc_frame(&IpcEnvelope::new(cmd)).expect("encode handshake")
    }

    fn text_frame(text: &str) -> Vec<u8> {
        let cmd = ClientCommand::SendTextInput {
            text: text.to_string(),
        };
        encode_ipc_frame(&IpcEnvelope::new(cmd)).expect("encode text frame")
    }

    #[tokio::test]
    async fn start_fails_closed_without_token() {
        let (btx, _brx) = tokio::sync::broadcast::channel(16);
        let (ctx, mut crx) = tokio::sync::mpsc::channel(16);
        drop(crx.recv());
        let server = TcpLoopbackServer::new("127.0.0.1", 0, None, btx, ctx);
        assert!(
            server.is_err(),
            "listener must refuse to start without a token"
        );
    }

    #[tokio::test]
    async fn authenticated_sidecar_receives_snapshot_and_delivers_command() {
        use tokio::io::AsyncWriteExt;

        let (btx, _brx) = tokio::sync::broadcast::channel(16);
        let (ctx, mut crx) = tokio::sync::mpsc::channel(16);
        let server =
            TcpLoopbackServer::new("127.0.0.1", 0, Some("secret-token".to_string()), btx, ctx)
                .expect("server with token");
        let port = server.start().await.expect("bind");
        assert_ne!(port, 0);

        let mut conn = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect");
        conn.write_all(&handshake_frame(Some("secret-token")))
            .await
            .unwrap();

        // Snapshot must arrive, proving the connection was authenticated.
        let snapshot = read_framed_message::<_, DaemonMessage>(&mut conn)
            .await
            .expect("snapshot after handshake");
        assert!(matches!(
            snapshot.payload,
            DaemonMessage::StateSnapshot { .. }
        ));

        // A SendTextInput command must be delivered to the daemon command channel.
        conn.write_all(&text_frame("hello from sidecar"))
            .await
            .unwrap();
        let delivered = tokio::time::timeout(std::time::Duration::from_secs(2), crx.recv())
            .await
            .expect("command delivered in time");
        match delivered {
            Some(ClientCommand::SendTextInput { text }) => assert_eq!(text, "hello from sidecar"),
            other => panic!("unexpected command: {other:?}"),
        }

        conn.shutdown().await.ok();
        server.stop();
    }

    #[tokio::test]
    async fn wrong_token_connection_is_dropped_without_snapshot() {
        use tokio::io::AsyncWriteExt;

        let (btx, _brx) = tokio::sync::broadcast::channel(16);
        let (ctx, mut crx) = tokio::sync::mpsc::channel(16);
        drop(crx.recv());
        let server =
            TcpLoopbackServer::new("127.0.0.1", 0, Some("secret-token".to_string()), btx, ctx)
                .expect("server with token");
        let port = server.start().await.expect("bind");

        let mut conn = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect");
        conn.write_all(&handshake_frame(Some("wrong-token")))
            .await
            .unwrap();

        // The server must not send a snapshot for an unauthenticated connection:
        // the read must either be dropped (eof) or still pending, never Ok.
        let result = tokio::time::timeout(std::time::Duration::from_millis(500), async {
            read_framed_message::<_, DaemonMessage>(&mut conn).await
        })
        .await;
        if let Ok(Ok(envelope)) = result {
            panic!(
                "unauthenticated sidecar received a snapshot: {:?}",
                envelope.payload
            );
        }
        server.stop();
    }
}
