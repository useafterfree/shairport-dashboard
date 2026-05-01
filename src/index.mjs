import { h, render } from "https://unpkg.com/preact?module";
import htm from "https://unpkg.com/htm?module";

const html = htm.bind(h);

const MAX_POINTS = 120;

const state = {
    latestShairport: null,
    latestWifi: null,
    shairportSeries: {
        av_sync_error_ms: [],
        ppm: [],
        sync_window_ms: [],
    },
    wifiSeries: {
        signal_dbm: [],
        tx_bitrate_mbit_s: [],
        rx_bitrate_mbit_s: [],
        tx_failed: [],
    },
};

function keepLastN(points, value, size = MAX_POINTS) {
    points.push(value);
    if (points.length > size) {
        points.shift();
    }
}

function pushShairport(sample) {
    state.latestShairport = sample;
    keepLastN(state.shairportSeries.av_sync_error_ms, sample.av_sync_error_ms);
    keepLastN(state.shairportSeries.ppm, sample.ppm);
    keepLastN(state.shairportSeries.sync_window_ms, sample.sync_window_ms);
}

function pushWifi(sample) {
    state.latestWifi = sample;
    keepLastN(state.wifiSeries.signal_dbm, sample.signal_dbm);
    keepLastN(state.wifiSeries.tx_bitrate_mbit_s, sample.tx_bitrate_mbit_s);
    keepLastN(state.wifiSeries.rx_bitrate_mbit_s, sample.rx_bitrate_mbit_s);
    keepLastN(state.wifiSeries.tx_failed, sample.tx_failed);
}

function toPolyline(points, width, height) {
    if (points.length < 2) {
        return "";
    }

    let min = Math.min(...points);
    let max = Math.max(...points);

    if (min === max) {
        min -= 1;
        max += 1;
    }

    return points
        .map((point, idx) => {
            const x = (idx / (points.length - 1)) * width;
            const ratio = (point - min) / (max - min);
            const y = height - ratio * height;
            return `${x.toFixed(2)},${y.toFixed(2)}`;
        })
        .join(" ");
}

function fmt(value, digits = 2) {
    if (value === null || value === undefined) {
        return "-";
    }

    if (typeof value === "number") {
        return value.toFixed(digits);
    }

    return String(value);
}

function Chart(props) {
    const width = 420;
    const height = 130;
    const points = props.points || [];
    const polyline = toPolyline(points, width, height);

    return html`
        <article class="chart-card">
            <header>
                <h3>${props.title}</h3>
                <span>${props.value}</span>
            </header>
            <svg viewBox="0 0 ${width} ${height}" preserveAspectRatio="none">
                <polyline points=${polyline}></polyline>
            </svg>
        </article>
    `;
}

function App(props) {
    const latestWifi = props.state.latestWifi;
    const latestShairport = props.state.latestShairport;

    return html`
    <main>
        <section class="title-row">
            <h1>Raspberry Pi Stats Live</h1>
            <p>Realtime shairport and wlan0 station telemetry</p>
        </section>

        <section class="meta-grid">
            <article class="meta-card">
                <h2>Wi-Fi Station</h2>
                <p>MAC: ${latestWifi ? latestWifi.station_mac : "-"}</p>
                <p>Interface: ${latestWifi ? latestWifi.interface_name : "-"}</p>
                <p>Connected: ${latestWifi ? `${latestWifi.connected_time_seconds}s` : "-"}</p>
            </article>
            <article class="meta-card">
                <h2>Shairport</h2>
                <p>Timestamp: ${latestShairport ? latestShairport.timestamp : "-"}</p>
                <p>Missing: ${latestShairport ? latestShairport.missing : "-"}</p>
                <p>Resend: ${latestShairport ? latestShairport.resend : "-"}</p>
            </article>
        </section>

        <section class="chart-grid">
            <${Chart}
                title="Signal (dBm)"
                value=${latestWifi ? `${fmt(latestWifi.signal_dbm, 0)} dBm` : "-"}
                points=${props.state.wifiSeries.signal_dbm}
            />
            <${Chart}
                title="TX Bitrate (MBit/s)"
                value=${latestWifi ? `${fmt(latestWifi.tx_bitrate_mbit_s)} Mb/s` : "-"}
                points=${props.state.wifiSeries.tx_bitrate_mbit_s}
            />
            <${Chart}
                title="RX Bitrate (MBit/s)"
                value=${latestWifi ? `${fmt(latestWifi.rx_bitrate_mbit_s)} Mb/s` : "-"}
                points=${props.state.wifiSeries.rx_bitrate_mbit_s}
            />
            <${Chart}
                title="TX Failed"
                value=${latestWifi ? fmt(latestWifi.tx_failed, 0) : "-"}
                points=${props.state.wifiSeries.tx_failed}
            />
            <${Chart}
                title="AV Sync Error (ms)"
                value=${latestShairport ? `${fmt(latestShairport.av_sync_error_ms)} ms` : "-"}
                points=${props.state.shairportSeries.av_sync_error_ms}
            />
            <${Chart}
                title="PPM"
                value=${latestShairport ? fmt(latestShairport.ppm) : "-"}
                points=${props.state.shairportSeries.ppm}
            />
            <${Chart}
                title="Sync Window (ms)"
                value=${latestShairport ? `${fmt(latestShairport.sync_window_ms)} ms` : "-"}
                points=${props.state.shairportSeries.sync_window_ms}
            />
        </section>
    </main>
  `;
}

function redraw() {
    render(html`<${App} state=${state}></${App}>`, document.body);
}

let url = new URL("/ws", window.location.href);
// http => ws
// https => wss
url.protocol = url.protocol.replace("http", "ws");

let ws = new WebSocket(url.href);
ws.onmessage = (ev) => {
    let event = JSON.parse(ev.data);
    if (event.kind === "Shairport") {
        pushShairport(event.payload);
    }
    if (event.kind === "WifiStation") {
        pushWifi(event.payload);
    }

    redraw();
};

ws.onopen = () => redraw();