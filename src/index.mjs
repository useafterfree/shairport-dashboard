import { h, render } from "https://cdn.jsdelivr.net/npm/preact@10.29.1/+esm";
import { useEffect, useRef } from "https://cdn.jsdelivr.net/npm/preact@10.29.1/hooks/+esm";
import htm from "https://cdn.jsdelivr.net/npm/htm@3.1.1/+esm";
import ChartJS from "https://cdn.jsdelivr.net/npm/chart.js@4.4.3/auto/+esm";

const html = htm.bind(h);

const MAX_POINTS = 120;

const state = {
    latestShairport: null,
    latestShairportMetadata: null,
    nowPlayingSignature: "",
    nowPlayingPulseToken: 0,
    latestWifi: null,
    latestSystem: null,
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
    systemSeries: {
        cpu_temp_c: [],
        cpu_usage_pct: [],
        ram_usage_pct: [],
        fan_speed_rpm: [],
        throttled: [],
    },
};

function keepLastN(points, value, size = MAX_POINTS) {
    return [...points, value].slice(-size);
}

function parseShairportTime(timestamp) {
    const parsed = Date.parse(timestamp);
    if (Number.isNaN(parsed)) {
        return Date.now();
    }

    return parsed;
}

function pushShairport(sample) {
    state.latestShairport = sample;
    const x = parseShairportTime(sample.timestamp);
    state.shairportSeries.av_sync_error_ms = keepLastN(state.shairportSeries.av_sync_error_ms, {
        x,
        y: sample.av_sync_error_ms,
    });
    state.shairportSeries.ppm = keepLastN(state.shairportSeries.ppm, { x, y: sample.ppm });
    state.shairportSeries.sync_window_ms = keepLastN(state.shairportSeries.sync_window_ms, {
        x,
        y: sample.sync_window_ms,
    });
}

function artworkDataUrl(base64Data) {
    if (!base64Data) {
        return null;
    }

    const trimmed = base64Data.trim();
    if (trimmed.startsWith("iVBOR")) {
        return `data:image/png;base64,${trimmed}`;
    }
    if (trimmed.startsWith("/9j/")) {
        return `data:image/jpeg;base64,${trimmed}`;
    }

    return `data:image/*;base64,${trimmed}`;
}

function pushShairportMetadata(sample) {
    const signature = [sample.track ?? "", sample.artist ?? "", sample.album ?? ""].join("|");
    const hasAnyValue = Boolean(sample.track || sample.artist || sample.album);
    if (hasAnyValue && signature !== state.nowPlayingSignature) {
        state.nowPlayingPulseToken += 1;
    }
    state.nowPlayingSignature = signature;

    state.latestShairportMetadata = {
        ...sample,
        artwork_url: artworkDataUrl(sample.artwork_base64),
    };
}

function pushWifi(sample) {
    state.latestWifi = sample;
    const x = sample.current_time_ms || Date.now();
    state.wifiSeries.signal_dbm = keepLastN(state.wifiSeries.signal_dbm, {
        x,
        y: sample.signal_dbm,
    });
    state.wifiSeries.tx_bitrate_mbit_s = keepLastN(state.wifiSeries.tx_bitrate_mbit_s, {
        x,
        y: sample.tx_bitrate_mbit_s,
    });
    state.wifiSeries.rx_bitrate_mbit_s = keepLastN(state.wifiSeries.rx_bitrate_mbit_s, {
        x,
        y: sample.rx_bitrate_mbit_s,
    });
    state.wifiSeries.tx_failed = keepLastN(state.wifiSeries.tx_failed, {
        x,
        y: sample.tx_failed,
    });
}

function pushSystem(sample) {
    state.latestSystem = sample;
    const x = sample.timestamp_ms || Date.now();

    state.systemSeries.cpu_temp_c = keepLastN(state.systemSeries.cpu_temp_c, {
        x,
        y: sample.cpu_temp_c,
    });
    state.systemSeries.cpu_usage_pct = keepLastN(state.systemSeries.cpu_usage_pct, {
        x,
        y: sample.cpu_usage_pct,
    });
    state.systemSeries.ram_usage_pct = keepLastN(state.systemSeries.ram_usage_pct, {
        x,
        y: sample.ram_usage_pct,
    });

    if (sample.fan_speed_rpm !== null && sample.fan_speed_rpm !== undefined) {
        state.systemSeries.fan_speed_rpm = keepLastN(state.systemSeries.fan_speed_rpm, {
            x,
            y: sample.fan_speed_rpm,
        });
    }

    if (sample.throttled_now !== null && sample.throttled_now !== undefined) {
        state.systemSeries.throttled = keepLastN(state.systemSeries.throttled, {
            x,
            y: sample.throttled_now ? 1 : 0,
        });
    }
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

function MetricChart(props) {
    const canvasRef = useRef(null);
    const chartRef = useRef(null);

    useEffect(() => {
        if (!canvasRef.current) {
            return;
        }

        chartRef.current = new ChartJS(canvasRef.current, {
            type: "line",
            data: {
                datasets: [
                    {
                        data: props.points || [],
                        parsing: false,
                        borderColor: "#77f7d8",
                        borderWidth: 2.2,
                        pointRadius: 2,
                        pointHoverRadius: 3,
                        tension: 0.2,
                        stepped: props.stepped || false,
                    },
                ],
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                animation: false,
                plugins: {
                    legend: { display: false },
                    tooltip: {
                        callbacks: {
                            title(items) {
                                if (!items.length) {
                                    return "";
                                }
                                return new Date(items[0].parsed.x).toLocaleTimeString();
                            },
                        },
                    },
                },
                scales: {
                    x: {
                        type: "linear",
                        ticks: {
                            color: "#9bc7cd",
                            callback(value) {
                                return new Date(Number(value)).toLocaleTimeString();
                            },
                            maxTicksLimit: 4,
                        },
                        grid: {
                            color: "rgba(167, 213, 222, 0.14)",
                        },
                    },
                    y: {
                        min: props.yMin,
                        max: props.yMax,
                        ticks: {
                            color: "#9bc7cd",
                            stepSize: props.yStep,
                            callback(value) {
                                if (props.yTickFormatter) {
                                    return props.yTickFormatter(value);
                                }
                                return value;
                            },
                        },
                        grid: {
                            color: "rgba(167, 213, 222, 0.12)",
                        },
                    },
                },
            },
        });

        return () => {
            if (chartRef.current) {
                chartRef.current.destroy();
            }
        };
    }, []);

    useEffect(() => {
        if (!chartRef.current) {
            return;
        }

        chartRef.current.data.datasets[0].data = props.points || [];
        chartRef.current.update("none");
    }, [props.points]);

    return html`
        <article class="chart-card">
            <header>
                <h3>${props.title}</h3>
                <span>${props.value}</span>
            </header>
            <div class="chart-canvas-wrap">
                <canvas ref=${canvasRef} class="chart-canvas"></canvas>
            </div>
        </article>
    `;
}

function App(props) {
    const latestWifi = props.state.latestWifi;
    const latestShairport = props.state.latestShairport;
    const latestShairportMetadata = props.state.latestShairportMetadata;
    const latestSystem = props.state.latestSystem;
    const nowPlayingPulseClass = props.state.nowPlayingPulseToken > 0 ? "pulse-highlight" : "";

    return html`
    <main>
        <section class="title-row">
            <h1>Raspberry Pi Stats Live</h1>
            <p>Realtime shairport and wlan0 station telemetry</p>
        </section>

        <section class="meta-grid">
            <article key=${`top-now-playing-${props.state.nowPlayingPulseToken}`} class=${`meta-card ${nowPlayingPulseClass}`}>
                <h2>Now Playing</h2>
                <p>Track: ${latestShairportMetadata?.track || "-"}</p>
                <p>Artist: ${latestShairportMetadata?.artist || "-"}</p>
                <p>Album: ${latestShairportMetadata?.album || "-"}</p>
            </article>
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
            <article class="meta-card">
                <h2>Raspberry Pi 5 System</h2>
                <p>CPU Temp: ${latestSystem ? `${fmt(latestSystem.cpu_temp_c)} C` : "-"}</p>
                <p>CPU Usage: ${latestSystem ? `${fmt(latestSystem.cpu_usage_pct)} %` : "-"}</p>
                <p>RAM Usage: ${latestSystem ? `${fmt(latestSystem.ram_usage_pct)} %` : "-"}</p>
                <p>Fan: ${latestSystem ? `${latestSystem.fan_speed_rpm ?? "-"} rpm` : "-"}</p>
                <p>
                    Throttled:
                    ${latestSystem && latestSystem.throttled_now !== null
            ? latestSystem.throttled_now
                ? "yes"
                : "no"
            : "unknown"}
                </p>
            </article>
        </section>

        <section class="stream-section">
            <header class="stream-header">
                <h2>Raspberry Pi 5 System</h2>
                <p>Thermals, utilization, cooling, and throttling</p>
            </header>
            <div class="chart-grid">
                <${MetricChart}
                    title="CPU Temp (C)"
                    value=${latestSystem ? `${fmt(latestSystem.cpu_temp_c)} C` : "-"}
                    points=${props.state.systemSeries.cpu_temp_c}
                />
                <${MetricChart}
                    title="CPU Usage (%)"
                    value=${latestSystem ? `${fmt(latestSystem.cpu_usage_pct)} %` : "-"}
                    points=${props.state.systemSeries.cpu_usage_pct}
                    yMin=${0}
                    yMax=${100}
                />
                <${MetricChart}
                    title="RAM Usage (%)"
                    value=${latestSystem ? `${fmt(latestSystem.ram_usage_pct)} %` : "-"}
                    points=${props.state.systemSeries.ram_usage_pct}
                    yMin=${0}
                    yMax=${100}
                />
                <${MetricChart}
                    title="Fan Speed (RPM)"
                    value=${latestSystem && latestSystem.fan_speed_rpm != null
            ? `${latestSystem.fan_speed_rpm} rpm`
            : "-"}
                    points=${props.state.systemSeries.fan_speed_rpm}
                />
                <${MetricChart}
                    title="Processor Throttled"
                    value=${latestSystem && latestSystem.throttled_now !== null
            ? latestSystem.throttled_now
                ? "yes"
                : "no"
            : "unknown"}
                    points=${props.state.systemSeries.throttled}
                    yMin=${-0.1}
                    yMax=${1.1}
                    yStep=${1}
                    stepped=${true}
                    yTickFormatter=${(value) => (Number(value) === 1 ? "yes" : "no")}
                />
            </div>
        </section>

        <section class="stream-section">
            <header class="stream-header">
                <h2>Wi-Fi Stream</h2>
                <p>wlan0 station quality and throughput</p>
            </header>
            <div class="chart-grid">
                <${MetricChart}
                    title="Signal (dBm)"
                    value=${latestWifi ? `${fmt(latestWifi.signal_dbm, 0)} dBm` : "-"}
                    points=${props.state.wifiSeries.signal_dbm}
                />
                <${MetricChart}
                    title="TX Bitrate (MBit/s)"
                    value=${latestWifi ? `${fmt(latestWifi.tx_bitrate_mbit_s)} Mb/s` : "-"}
                    points=${props.state.wifiSeries.tx_bitrate_mbit_s}
                />
                <${MetricChart}
                    title="RX Bitrate (MBit/s)"
                    value=${latestWifi ? `${fmt(latestWifi.rx_bitrate_mbit_s)} Mb/s` : "-"}
                    points=${props.state.wifiSeries.rx_bitrate_mbit_s}
                />
                <${MetricChart}
                    title="TX Failed"
                    value=${latestWifi ? fmt(latestWifi.tx_failed, 0) : "-"}
                    points=${props.state.wifiSeries.tx_failed}
                />
            </div>
        </section>

        <section class="stream-section">
            <header class="stream-header">
                <h2>Shairport Stream</h2>
                <p>Audio sync drift and window behavior</p>
            </header>
            <div class="shairport-meta-wrap">
                <article key=${`stream-now-playing-${props.state.nowPlayingPulseToken}`} class=${`meta-card now-playing-card ${nowPlayingPulseClass}`}>
                    <h2>Track Metadata</h2>
                    <p>Track: ${latestShairportMetadata?.track || "-"}</p>
                    <p>Artist: ${latestShairportMetadata?.artist || "-"}</p>
                    <p>Album: ${latestShairportMetadata?.album || "-"}</p>
                </article>
                <article class="meta-card art-card">
                    <h2>Artwork</h2>
                    ${latestShairportMetadata?.artwork_url
            ? html`<img src=${latestShairportMetadata.artwork_url} alt="album art" class="art-image" />`
            : html`<div class="art-placeholder">No art</div>`}
                </article>
            </div>
            <div class="chart-grid">
                <${MetricChart}
                    title="AV Sync Error (ms)"
                    value=${latestShairport ? `${fmt(latestShairport.av_sync_error_ms)} ms` : "-"}
                    points=${props.state.shairportSeries.av_sync_error_ms}
                />
                <${MetricChart}
                    title="PPM"
                    value=${latestShairport ? fmt(latestShairport.ppm) : "-"}
                    points=${props.state.shairportSeries.ppm}
                />
                <${MetricChart}
                    title="Sync Window (ms)"
                    value=${latestShairport ? `${fmt(latestShairport.sync_window_ms)} ms` : "-"}
                    points=${props.state.shairportSeries.sync_window_ms}
                />
            </div>
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
    if (event.kind === "System") {
        pushSystem(event.payload);
    }
    if (event.kind === "ShairportMetadata") {
        pushShairportMetadata(event.payload);
    }

    redraw();
};

ws.onopen = () => redraw();