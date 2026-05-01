mod collectors;

use collectors::shairport::stream_shairport_logs;
use collectors::shairport_metadata::stream_shairport_metadata;
use collectors::system::stream_system_stats;
use collectors::wifi::stream_wlan_station_dump;
use rust_stats::models::SampleEvent;

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
    tokio::spawn(stream_shairport_metadata(tx.clone()));
    tokio::spawn(stream_wlan_station_dump(tx.clone()));
    tokio::spawn(stream_system_stats(tx.clone()));

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
