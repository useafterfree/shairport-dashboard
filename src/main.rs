mod collectors;

use collectors::shairport::stream_shairport_logs;
use collectors::shairport_metadata::stream_shairport_metadata;
use collectors::system::stream_system_stats;
use collectors::wifi::stream_wlan_station_dump;
use shairport_dashboard::models::{SampleEvent, ShairportMetadataSample};

use axum::{
    Router,
    extract::State,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    http::Response,
    response::{Html, IntoResponse},
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio::sync::{RwLock, broadcast};

const HISTORY_MAX_POINTS: usize = 120;

#[derive(Clone)]
struct AppState {
    tx: broadcast::Sender<SampleEvent>,
    history: Arc<RwLock<HistoryState>>,
}

#[derive(Default)]
struct HistoryState {
    telemetry: VecDeque<TimedEvent>,
    last_track: Option<TimedEvent>,
}

#[derive(Serialize, Deserialize, Clone)]
struct TimedEvent {
    recorded_at_ms: u64,
    event: SampleEvent,
}

#[derive(Serialize)]
struct OutboundEvent<'a> {
    #[serde(flatten)]
    event: &'a SampleEvent,
    recorded_at_ms: u64,
}

#[derive(Serialize, Deserialize, Default)]
struct PersistedHistory {
    telemetry: Vec<TimedEvent>,
    last_track: Option<TimedEvent>,
}

#[derive(Deserialize, Default)]
struct LegacyPersistedHistory {
    telemetry: Vec<SampleEvent>,
    last_track: Option<ShairportMetadataSample>,
}

impl LegacyPersistedHistory {
    fn into_timed(self) -> PersistedHistory {
        let now = now_ms();
        let telemetry = self
            .telemetry
            .into_iter()
            .map(|event| TimedEvent {
                recorded_at_ms: now,
                event,
            })
            .collect();

        let last_track = self.last_track.map(|metadata| TimedEvent {
            recorded_at_ms: now,
            event: SampleEvent::ShairportMetadata(metadata),
        });

        PersistedHistory {
            telemetry,
            last_track,
        }
    }
}

impl HistoryState {
    fn from_persisted(persisted: PersistedHistory) -> Self {
        let mut telemetry = VecDeque::from(persisted.telemetry);
        while telemetry.len() > HISTORY_MAX_POINTS {
            let _ = telemetry.pop_front();
        }

        Self {
            telemetry,
            last_track: persisted.last_track,
        }
    }

    fn to_persisted(&self) -> PersistedHistory {
        PersistedHistory {
            telemetry: self.telemetry.iter().cloned().collect(),
            last_track: self.last_track.clone(),
        }
    }

    fn apply_event(&mut self, event: SampleEvent, recorded_at_ms: u64) {
        let timed = TimedEvent {
            recorded_at_ms,
            event,
        };

        match &timed.event {
            SampleEvent::Shairport(_) | SampleEvent::WifiStation(_) | SampleEvent::System(_) => {
                self.telemetry.push_back(timed);
                if self.telemetry.len() > HISTORY_MAX_POINTS {
                    let _ = self.telemetry.pop_front();
                }
            }
            SampleEvent::ShairportMetadata(metadata) => {
                if has_track_data(metadata) {
                    self.last_track = Some(timed);
                }
            }
        }
    }

    fn replay_events(&self) -> Vec<TimedEvent> {
        let mut events: Vec<TimedEvent> = self.telemetry.iter().cloned().collect();
        if let Some(metadata) = &self.last_track {
            events.push(metadata.clone());
        }
        events
    }
}

fn history_path() -> String {
    format!("/tmp/{}.history", env!("CARGO_PKG_NAME"))
}

fn has_track_data(metadata: &ShairportMetadataSample) -> bool {
    metadata.track.is_some()
        || metadata.artist.is_some()
        || metadata.album.is_some()
        || metadata.genre.is_some()
        || metadata.artwork_base64.is_some()
}

async fn load_history(path: &str) -> HistoryState {
    match tokio::fs::read_to_string(path).await {
        Ok(raw) => {
            if let Ok(parsed) = serde_json::from_str::<PersistedHistory>(&raw) {
                return HistoryState::from_persisted(parsed);
            }

            if let Ok(parsed) = serde_json::from_str::<LegacyPersistedHistory>(&raw) {
                return HistoryState::from_persisted(parsed.into_timed());
            }

            HistoryState::default()
        }
        Err(_) => HistoryState::default(),
    }
}

async fn save_history(path: &str, history: PersistedHistory) {
    if let Ok(raw) = serde_json::to_string(&history) {
        let tmp_path = format!("{path}.tmp");
        if tokio::fs::write(&tmp_path, raw).await.is_ok() {
            let _ = tokio::fs::rename(&tmp_path, path).await;
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn encode_outbound(event: &SampleEvent, recorded_at_ms: u64) -> String {
    serde_json::to_string(&OutboundEvent {
        event,
        recorded_at_ms,
    })
    .unwrap()
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    let rx = state.tx.subscribe();
    let history = state.history.clone();
    ws.on_upgrade(move |socket| handle_socket(socket, rx, history))
}

async fn handle_socket(
    mut socket: WebSocket,
    mut rx: broadcast::Receiver<SampleEvent>,
    history: Arc<RwLock<HistoryState>>,
) {
    let replay = {
        let history = history.read().await;
        history.replay_events()
    };

    for sample in replay {
        let msg = encode_outbound(&sample.event, sample.recorded_at_ms);
        if socket.send(Message::Text(msg.into())).await.is_err() {
            return;
        }
    }

    while let Ok(sample) = rx.recv().await {
        let msg = encode_outbound(&sample, now_ms());
        if socket.send(Message::Text(msg.into())).await.is_err() {
            break;
        }
    }
}

#[tokio::main]
async fn main() {
    let (tx, _) = tokio::sync::broadcast::channel(100);
    let history_file = history_path();
    let history = Arc::new(RwLock::new(load_history(&history_file).await));

    {
        let history_file = history_file.clone();
        let history = history.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut rx = tx.subscribe();
            while let Ok(sample) = rx.recv().await {
                let recorded_at_ms = now_ms();
                let persisted = {
                    let mut state = history.write().await;
                    state.apply_event(sample, recorded_at_ms);
                    state.to_persisted()
                };
                save_history(&history_file, persisted).await;
            }
        });
    }

    tokio::spawn(stream_shairport_logs(tx.clone()));
    tokio::spawn(stream_shairport_metadata(tx.clone()));
    tokio::spawn(stream_wlan_station_dump(tx.clone()));
    tokio::spawn(stream_system_stats(tx.clone()));

    let app_state = AppState { tx, history };

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/", get(root_get))
        .route("/index.mjs", get(indexmjs_get))
        .route("/index.css", get(indexcss_get))
        .with_state(app_state);

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    let addr = listener.local_addr().unwrap();

    println!("Listening on http://{addr} and ws://{addr}/ws");

    axum::serve(listener, app).await.unwrap();
}

#[axum::debug_handler]
async fn root_get() -> impl IntoResponse {
    let markup = tokio::fs::read_to_string("src/index.html").await.unwrap();

    Html(markup)
}

#[axum::debug_handler]
async fn indexmjs_get() -> impl IntoResponse {
    let markup = tokio::fs::read_to_string("src/index.mjs").await.unwrap();

    Response::builder()
        .header("content-type", "application/javascript;charset=utf-8")
        .body(markup)
        .unwrap()
}

#[axum::debug_handler]
async fn indexcss_get() -> impl IntoResponse {
    let markup = tokio::fs::read_to_string("src/index.css").await.unwrap();

    Response::builder()
        .header("content-type", "text/css;charset=utf-8")
        .body(markup)
        .unwrap()
}
