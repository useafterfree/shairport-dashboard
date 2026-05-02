# Shairport Dashboard

Realtime dashboard for Raspberry Pi audio + system telemetry, built with Rust (Axum + Tokio) and a lightweight browser UI (Preact + Chart.js).

## What This App Does

- Streams shairport-sync timing metrics from systemd logs
- Streams shairport metadata (track, artist, album, genre, artwork)
- Streams Wi-Fi station metrics from `iw dev wlan0 station dump`
- Streams Raspberry Pi system metrics (CPU temp, CPU/RAM usage, fan RPM, throttling)
- Pushes all samples to the browser over WebSocket for live charts
- Persists recent history so reconnecting clients get immediate context

## Architecture Overview

The app is split into three main layers:

1. Data collectors (server-side)
2. Event bus + history (server-side)
3. Realtime dashboard UI (client-side)

### 1) Data Collectors

Collectors run as independent Tokio tasks and emit typed samples as `SampleEvent` values.

- `src/collectors/shairport.rs`
  - Tails `journalctl -u shairport-sync.service -f -o short-iso`
  - Regex-parses timing metrics into `ShairportSample`
- `src/collectors/shairport_metadata.rs`
  - Reads metadata FIFO at `/tmp/shairport-sync-metadata`
  - Decodes XML/hex payload fields and optional artwork
- `src/collectors/wifi.rs`
  - Polls `iw dev wlan0 station dump` every second
  - Uses `WifiStationParser` from `src/lib.rs`
- `src/collectors/system.rs`
  - Polls `/proc` and system interfaces every second
  - Produces CPU/RAM/temp/fan/throttling samples

### 2) Event Bus and History

`src/main.rs` wires collectors into a broadcast pipeline:

- Creates a Tokio broadcast channel `tx`
- Spawns each collector and forwards events via `tx.send(...)`
- Maintains in-memory history in `HistoryState`
- Persists history to `/tmp/shairport-dashboard.history`

History behavior:

- Keeps up to `HISTORY_MAX_POINTS` telemetry samples
- Stores latest useful track metadata separately
- Replays history to each new WebSocket client before live stream begins

### 3) HTTP + WebSocket API

The Axum server listens on `0.0.0.0:3000` and serves:

- `GET /` -> `src/index.html`
- `GET /index.mjs` -> frontend app code
- `GET /index.css` -> styles/themes
- `GET /ws` -> realtime event stream

WebSocket messages include:

- Event payload (flattened from `SampleEvent`)
- `recorded_at_ms` timestamp generated on the server

## Frontend Architecture

The frontend is a small Preact app in `src/index.mjs`:

- Opens a WebSocket to `/ws`
- Maintains rolling series per metric in local app state
- Renders grouped metric cards and line charts
- Supports theme switching (`terminal`, `purple`, `high-contrast`, `black-white`)

Charts:

- Implemented with Chart.js in `MetricChart`
- Time series are downsampled for rendering performance
- Chart colors are read from CSS custom properties so themes drive line/dot/grid colors

## Data Model

Core event/sample structs are in `src/models.rs`.

Main sample types:

- `ShairportSample`
- `ShairportMetadataSample`
- `WifiStationSample`
- `SystemSample`

These are wrapped in `SampleEvent` for transport and replay.

## Runtime Flow

1. App starts and loads persisted history from `/tmp/shairport-dashboard.history`.
2. Collector tasks start producing samples.
3. Each sample is broadcast to all live WebSocket subscribers.
4. The same sample is applied to in-memory history and persisted.
5. New clients connecting to `/ws` receive replay first, then live updates.

## Build and Run

Requirements:

- Rust toolchain
- Linux environment with:
  - `journalctl` access for `shairport-sync.service`
  - `iw` command for Wi-Fi metrics
  - Raspberry Pi sysfs/procfs paths for full system stats

Commands:

```bash
cargo run --release
```

Or via Makefile:

```bash
make run
```

Open dashboard:

- `http://localhost:3000`

## Install as a Service

The repo includes a systemd unit file and Makefile targets:

```bash
make install
```

This installs:

- Binary to `/usr/local/bin/shairport-dashboard`
- Service unit to `/etc/systemd/system/shairport-dashboard.service`

To remove:

```bash
make uninstall
```

## Screenshots

Screenshots are intentionally deferred for now and can be added later under a section like:

- Dashboard overview
- Purple theme example
- System metrics section
