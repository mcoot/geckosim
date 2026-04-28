//! End-to-end smoke test: spawn the host's driver + server tasks in
//! the same process on an ephemeral port; drive the protocol with a
//! `tokio-tungstenite` client.
//!
//! Exercises: `Hello`/`Init` handshake, periodic `Snapshot` streaming with
//! monotonic tick, `TogglePause` stops ticks, second `TogglePause` resumes.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use gecko_sim_core::{ContentBundle, Sim, Snapshot, WorldLayout};
use gecko_sim_host::{sim_driver, ws_server};
use gecko_sim_protocol::{
    ClientMessage, PlayerInput, ServerMessage, WireFormat, PROTOCOL_VERSION,
};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message as WsMessage;

async fn next_text<S>(stream: &mut S) -> String
where
    S: StreamExt<Item = Result<WsMessage, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        let msg = stream
            .next()
            .await
            .expect("stream ended early")
            .expect("ws error");
        if let WsMessage::Text(s) = msg {
            return s;
        }
    }
}

async fn next_server_msg<S>(stream: &mut S) -> ServerMessage
where
    S: StreamExt<Item = Result<WsMessage, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    let text = next_text(stream).await;
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("bad server msg {text}: {e}"))
}

async fn next_snapshot_tick<S>(stream: &mut S) -> u64
where
    S: StreamExt<Item = Result<WsMessage, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        match next_server_msg(stream).await {
            ServerMessage::Snapshot { snapshot } => return snapshot.tick,
            ServerMessage::Init { current_tick, .. } => return current_tick,
            ServerMessage::Hello { .. } => {}
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[expect(
    clippy::too_many_lines,
    reason = "linear end-to-end protocol script reads better as one function"
)]
async fn ws_handshake_and_pause_resume() {
    // 1. Build a sim with three agents.
    let mut sim = Sim::new(0x00C0_FFEE, ContentBundle::default());
    sim.spawn_test_agent("Alice");
    sim.spawn_test_agent("Bob");
    sim.spawn_test_agent("Charlie");
    let world_layout = WorldLayout::from(sim.world_graph());
    let initial = sim.snapshot();

    // 2. Wire channels and bind a listener on an ephemeral port.
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral");
    let local_addr = listener.local_addr().expect("local_addr");

    let (input_tx, input_rx) = tokio::sync::mpsc::unbounded_channel();
    let (snapshot_tx, snapshot_rx) = tokio::sync::watch::channel::<Snapshot>(initial);

    let driver = tokio::spawn(sim_driver::run(sim, input_rx, snapshot_tx));
    let server = tokio::spawn(ws_server::run(
        listener,
        input_tx,
        snapshot_rx,
        world_layout,
    ));

    // Give the server a moment to start the accept loop.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 3. Connect a WS client.
    let url = format!("ws://{local_addr}/");
    let (ws, _resp) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("connect");
    let (mut tx, mut rx) = ws.split();

    // 4. Server sends Hello first.
    match next_server_msg(&mut rx).await {
        ServerMessage::Hello {
            protocol_version,
            format,
        } => {
            assert_eq!(protocol_version, PROTOCOL_VERSION);
            assert_eq!(format, WireFormat::Json);
        }
        other => panic!("expected Hello, got {other:?}"),
    }

    // 5. Client sends ClientHello.
    let hello = ClientMessage::ClientHello {
        last_known_tick: None,
    };
    tx.send(WsMessage::Text(serde_json::to_string(&hello).unwrap()))
        .await
        .expect("send ClientHello");

    // 6. Server sends Init (carries world layout per ADR 0007).
    match next_server_msg(&mut rx).await {
        ServerMessage::Init {
            current_tick,
            world,
            snapshot,
        } => {
            assert_eq!(current_tick, snapshot.tick);
            assert_eq!(snapshot.agents.len(), 3);
            assert!(!world.leaves.is_empty(), "world.leaves must not be empty");
        }
        other => panic!("expected Init, got {other:?}"),
    }

    // 7. Observe a few Snapshot messages with monotonic non-decreasing tick.
    let mut last_tick = 0u64;
    let mut saw_advance = false;
    let deadline = tokio::time::Instant::now() + Duration::from_millis(2_000);
    while tokio::time::Instant::now() < deadline {
        let t = tokio::time::timeout(Duration::from_millis(500), next_snapshot_tick(&mut rx))
            .await
            .expect("snapshot timeout");
        assert!(t >= last_tick, "tick went backward: {t} < {last_tick}");
        if t > last_tick {
            saw_advance = true;
        }
        last_tick = t;
        if saw_advance && last_tick >= 1 {
            break;
        }
    }
    assert!(saw_advance, "expected at least one tick advance within 2s");

    // 8. TogglePause; ticks should freeze.
    let pause = ClientMessage::PlayerInput(PlayerInput::TogglePause);
    tx.send(WsMessage::Text(serde_json::to_string(&pause).unwrap()))
        .await
        .expect("send TogglePause (pause)");

    // Allow a few sample cycles to flush, then assert no advance over 500 ms.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let frozen_tick =
        tokio::time::timeout(Duration::from_millis(200), next_snapshot_tick(&mut rx))
            .await
            .expect("snapshot timeout while paused");
    let later_tick = tokio::time::timeout(Duration::from_millis(500), async {
        let mut latest = frozen_tick;
        for _ in 0..10 {
            latest = next_snapshot_tick(&mut rx).await;
        }
        latest
    })
    .await
    .expect("paused snapshots timeout");
    assert_eq!(
        later_tick, frozen_tick,
        "tick advanced while paused: frozen={frozen_tick}, later={later_tick}"
    );

    // 9. TogglePause again; ticks should resume.
    let resume = ClientMessage::PlayerInput(PlayerInput::TogglePause);
    tx.send(WsMessage::Text(serde_json::to_string(&resume).unwrap()))
        .await
        .expect("send TogglePause (resume)");

    let mut resumed_advance = false;
    let resume_deadline = tokio::time::Instant::now() + Duration::from_millis(2_500);
    while tokio::time::Instant::now() < resume_deadline {
        let t = tokio::time::timeout(Duration::from_millis(500), next_snapshot_tick(&mut rx))
            .await
            .expect("post-resume snapshot timeout");
        if t > later_tick {
            resumed_advance = true;
            break;
        }
    }
    assert!(resumed_advance, "tick did not resume after second TogglePause");

    // 10. Tear down.
    driver.abort();
    server.abort();
}
