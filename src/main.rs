use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Serialize;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tokio::time::interval;
use vu_meter_service::capture::{self, MeterState};
use vu_meter_service::protocol;

#[derive(Clone)]
struct AppState {
    meter: Arc<Mutex<MeterState>>,
    clients: Arc<AtomicUsize>,
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("VU_METER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(2717);

    let target = std::env::var("VU_METER_TARGET").ok();

    eprintln!("Starting VU meter service on port {}", port);
    if let Some(ref t) = target {
        eprintln!("Target: {}", t);
    }

    // Start PipeWire capture in background threads
    let (meter_state, quit_flag, clients) = capture::start_capture(target);

    let state = AppState {
        meter: meter_state,
        clients,
    };

    let app = Router::new()
        .route("/api/v1/levels", get(ws_handler))
        .route("/api/v1/version", get(get_version))
        .route("/version", get(get_version))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .expect("Failed to bind");

    eprintln!("Listening on 0.0.0.0:{}", port);

    // Graceful shutdown on SIGTERM/SIGINT
    let quit_for_shutdown = quit_flag.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        quit_for_shutdown.store(true, Ordering::Relaxed);
    });

    axum::serve(listener, app).await.expect("Server failed");

    quit_flag.store(true, Ordering::Relaxed);
}

#[derive(Serialize)]
struct VersionResponse {
    version: &'static str,
    api_version: &'static str,
}

async fn get_version() -> Json<VersionResponse> {
    Json(VersionResponse {
        version: env!("CARGO_PKG_VERSION"),
        api_version: "1.0",
    })
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    // Track this client
    let prev = state.clients.fetch_add(1, Ordering::Relaxed);
    eprintln!("Client connected ({} active)", prev + 1);

    // Send binary frames at ~10 Hz
    let mut tick = interval(Duration::from_millis(100));

    loop {
        tick.tick().await;

        let frame = {
            let meter = match state.meter.lock() {
                Ok(m) => m,
                Err(_) => break,
            };
            protocol::build_levels_frame(&meter.channels)
        };

        if socket.send(Message::Binary(frame.to_vec().into())).await.is_err() {
            break; // Client disconnected
        }
    }

    let remaining = state.clients.fetch_sub(1, Ordering::Relaxed) - 1;
    eprintln!("Client disconnected ({} active)", remaining);
}
