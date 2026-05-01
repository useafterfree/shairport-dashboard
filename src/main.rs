use rust_stats::{WifiStationParser, WifiStationSample};
use serde::Serialize;

#[derive(Serialize, Clone)]
struct ShairportSample {
    timestamp: String,
    av_sync_error_ms: f64,
    ppm: f64,
    sync_window_ms: f64,
    missing: u32,
    resend: u32,
    fps_r: Option<f64>,
    fps_c: Option<f64>,
}

#[derive(Serialize, Clone)]
#[serde(tag = "kind", content = "payload")]
enum SampleEvent {
    Shairport(ShairportSample),
    WifiStation(WifiStationSample),
}

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::{Duration, MissedTickBehavior, interval};

async fn stream_shairport_logs(tx: tokio::sync::broadcast::Sender<SampleEvent>) {
    let mut child = Command::new("journalctl")
        .args(["-u", "shairport-sync.service", "-f", "-o", "short-iso"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to start journalctl");

    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(sample) = parse_shairport_line(&line) {
            let _ = tx.send(SampleEvent::Shairport(sample));
        }
    }
}

async fn stream_wlan_station_dump(tx: tokio::sync::broadcast::Sender<SampleEvent>) {
    let mut ticker = interval(Duration::from_secs(1));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        ticker.tick().await;

        let output = match Command::new("iw")
            .args(["dev", "wlan0", "station", "dump"])
            .output()
            .await
        {
            Ok(output) => output,
            Err(err) => {
                eprintln!("failed to run iw station dump: {err}");
                continue;
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("iw station dump failed: {}", stderr.trim());
            continue;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut parser = WifiStationParser::new();

        for raw_line in stdout.lines() {
            if let Some(sample) = parser.parse_line(raw_line) {
                let _ = tx.send(SampleEvent::WifiStation(sample));
            }
        }
    }
}

use regex::Regex;

fn parse_shairport_line(line: &str) -> Option<ShairportSample> {
    let re = Regex::new(r"(?P<ts>\S+ \S+).*shairport-sync.*:\s+(-?\d+\.\d+)\s+(-?\d+\.\d+)\s+(-?\d+\.\d+)\s+(\d+)\s+(\d+)\s+(\S+)\s+(\S+)").ok()?;

    let caps = re.captures(line)?;

    let parse_opt = |s: &str| if s == "N/A" { None } else { s.parse().ok() };

    Some(ShairportSample {
        timestamp: caps[1].to_string(),
        av_sync_error_ms: caps[2].parse().ok()?,
        ppm: caps[3].parse().ok()?,
        sync_window_ms: caps[4].parse().ok()?,
        missing: caps[5].parse().ok()?,
        resend: caps[6].parse().ok()?,
        fps_r: parse_opt(&caps[7]),
        fps_c: parse_opt(&caps[8]),
    })
}

use axum::{
    Router,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    http::Response,
    response::{Html, IntoResponse},
    routing::get,
};
use tokio::net::TcpListener;
use tokio::sync::broadcast;

async fn ws_handler(ws: WebSocketUpgrade, tx: broadcast::Sender<SampleEvent>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, tx.subscribe()))
}

async fn handle_socket(mut socket: WebSocket, mut rx: broadcast::Receiver<SampleEvent>) {
    while let Ok(sample) = rx.recv().await {
        let msg = serde_json::to_string(&sample).unwrap();
        if socket.send(Message::Text(msg.into())).await.is_err() {
            break;
        }
    }
}

#[tokio::main]
async fn main() {
    let (tx, _) = tokio::sync::broadcast::channel(100);

    tokio::spawn(stream_shairport_logs(tx.clone()));
    tokio::spawn(stream_wlan_station_dump(tx.clone()));

    let app = Router::new()
        .route(
            "/ws",
            get({
                let tx = tx.clone();
                move |ws| ws_handler(ws, tx.clone())
            }),
        )
        .route("/", get(root_get))
        .route("/index.mjs", get(indexmjs_get))
        .route("/index.css", get(indexcss_get));

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
