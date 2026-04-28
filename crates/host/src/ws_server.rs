//! WS server task: accepts connections and runs per-connection handlers.
//!
//! Per-connection flow (ADR 0013, "Connection lifecycle"):
//!   1. `tokio_tungstenite::accept_async` → upgrade to WebSocket.
//!   2. Send `ServerMessage::Hello`.
//!   3. Read first frame, parse as `ClientMessage::ClientHello`
//!      (`last_known_tick` is parsed-but-ignored at v0).
//!   4. Read latest snapshot from `watch`, send `ServerMessage::Init`.
//!   5. Loop: forward `Snapshot` messages on every `watch::changed()`;
//!      forward inbound `PlayerInput` frames to `input_tx`.
//!
//! Multi-client falls out of `tokio::sync::watch` for free: every
//! per-connection task subscribes its own `Receiver`. Inputs from any
//! client apply, with no sender attribution (v0).

use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use gecko_sim_core::{Snapshot, WorldLayout};
use gecko_sim_protocol::{
    ClientMessage, PlayerInput, ServerMessage, WireFormat, PROTOCOL_VERSION,
};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::Message as WsMessage;

/// Run the accept loop on `listener`. Each accepted connection spawns a
/// per-connection task. This function returns only when the listener
/// errors fatally; `host::main` aborts the join handle on shutdown.
pub async fn run(
    listener: TcpListener,
    input_tx: mpsc::UnboundedSender<PlayerInput>,
    snapshot_rx: watch::Receiver<Snapshot>,
    world: WorldLayout,
) {
    let world = Arc::new(world);
    loop {
        let (stream, peer_addr) = match listener.accept().await {
            Ok(pair) => pair,
            Err(e) => {
                tracing::warn!(?e, "accept failed");
                continue;
            }
        };
        let input_tx = input_tx.clone();
        let snapshot_rx = snapshot_rx.clone();
        let world = Arc::clone(&world);
        tokio::spawn(async move {
            if let Err(e) =
                handle_connection(stream, peer_addr, input_tx, snapshot_rx, world).await
            {
                tracing::info!(%peer_addr, error = %e, "connection ended");
            }
        });
    }
}

async fn handle_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
    input_tx: mpsc::UnboundedSender<PlayerInput>,
    mut snapshot_rx: watch::Receiver<Snapshot>,
    world: Arc<WorldLayout>,
) -> anyhow::Result<()> {
    tracing::info!(%peer_addr, "ws connection accepted");
    let ws = tokio_tungstenite::accept_async(stream).await?;
    let (mut tx, mut rx) = ws.split();

    // 1. Hello.
    let hello = ServerMessage::Hello {
        protocol_version: PROTOCOL_VERSION,
        format: WireFormat::Json,
    };
    tx.send(WsMessage::Text(serde_json::to_string(&hello)?)).await?;

    // 2. Wait for ClientHello (parsed but field ignored at v0).
    let first = rx
        .next()
        .await
        .ok_or_else(|| anyhow::anyhow!("client closed before ClientHello"))??;
    match first {
        WsMessage::Text(text) => {
            let _client_hello: ClientMessage = serde_json::from_str(&text)?;
        }
        other => {
            return Err(anyhow::anyhow!(
                "expected text ClientHello, got {other:?}"
            ));
        }
    }

    // 3. Init.
    let snap = snapshot_rx.borrow_and_update().clone();
    let init = ServerMessage::Init {
        current_tick: snap.tick,
        world: (*world).clone(),
        snapshot: snap,
    };
    tx.send(WsMessage::Text(serde_json::to_string(&init)?)).await?;

    // 4. Stream loop.
    loop {
        tokio::select! {
            changed = snapshot_rx.changed() => {
                if changed.is_err() {
                    // sim_driver dropped — host is shutting down.
                    break;
                }
                let snap = snapshot_rx.borrow_and_update().clone();
                let msg = ServerMessage::Snapshot { snapshot: snap };
                tx.send(WsMessage::Text(serde_json::to_string(&msg)?)).await?;
            }
            frame = rx.next() => {
                match frame {
                    Some(Ok(WsMessage::Text(text))) => {
                        match serde_json::from_str::<ClientMessage>(&text) {
                            Ok(ClientMessage::PlayerInput(input)) => {
                                let _ = input_tx.send(input);
                            }
                            Ok(ClientMessage::ClientHello { .. }) => {
                                // No reconnect handshake mid-stream at v0; ignore.
                            }
                            Err(e) => {
                                tracing::debug!(%peer_addr, error = %e, "bad client frame, ignoring");
                            }
                        }
                    }
                    Some(Ok(WsMessage::Close(_))) | None => break,
                    Some(Ok(_)) => {
                        // Binary frames aren't part of v0; tungstenite handles
                        // ping/pong frames internally before we see them.
                    }
                    Some(Err(e)) => return Err(e.into()),
                }
            }
        }
    }

    tracing::info!(%peer_addr, "ws connection closed");
    Ok(())
}
