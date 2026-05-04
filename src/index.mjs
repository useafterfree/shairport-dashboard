import { h, render } from "https://cdn.jsdelivr.net/npm/preact@10.29.1/+esm";
import { useEffect, useRef } from "https://cdn.jsdelivr.net/npm/preact@10.29.1/hooks/+esm";
import htm from "https://cdn.jsdelivr.net/npm/htm@3.1.1/+esm";
import ChartJS from "https://cdn.jsdelivr.net/npm/chart.js@4.4.3/auto/+esm";

const html = htm.bind(h);
const THEMES = ["terminal", "purple", "high-contrast", "black-white"];

const state = {
    theme: "terminal",
    latestShairport: null,
    latestShairportMetadata: null,
    nowPlayingSignature: "",
    nowPlayingPulseToken: 0,
    wsState: "connecting",
    wsLastMessageMs: null,
    lastUpdateMs: {
        shairport: null,
        metadata: null,
        wifi: null,
        system: null,
        iw_event: null,
    },
    latestWifi: null,
    latestSystem: null,
    iwEventLog: [],
    iwEventFilter: "",
    shairportSeries: {
        av_sync_error_ms: [],
        ppm: [],
        sync_window_ms: [],
        missing: [],
        resend: [],
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

const MAX_IW_EVENT_LINES = 200;

function keepLastN(points, value) {
    return [...points, value];
}

function appendBounded(items, value, maxItems) {
    const next = [...items, value];
    if (next.length <= maxItems) {
        return next;
    }

    return next.slice(next.length - maxItems);
}

function applyTheme(themeName, persist = true) {
    const nextTheme = THEMES.includes(themeName) ? themeName : "terminal";
    state.theme = nextTheme;
    document.body.dataset.theme = nextTheme;

    if (!persist) {
        return;
    }

    try {
        localStorage.setItem("dashboard-theme", nextTheme);
    } catch (_err) {
        // Ignore storage failures (private mode, blocked storage, etc.)
    }
}

function initTheme() {
    let savedTheme = "terminal";
    try {
        const value = localStorage.getItem("dashboard-theme");
        if (value) {
            savedTheme = value;
        }
    } catch (_err) {
        // Keep default theme when storage is unavailable.
    }

    applyTheme(savedTheme, false);
}

function getChartColors() {
    const style = getComputedStyle(document.body);
    const lineColor = style.getPropertyValue("--line").trim();
    const mutedColor = style.getPropertyValue("--muted").trim();
    const gridColor = style.getPropertyValue("--chart-grid").trim();
    return { lineColor, mutedColor, gridColor };
}

function downsampleSeries(points, maxPoints) {
    if (!Array.isArray(points) || points.length <= maxPoints) {
        return points || [];
    }

    const total = points.length;
    const interior = total - 2;
    const bucketCount = Math.max(1, Math.floor(maxPoints / 2));
    const bucketSize = interior / bucketCount;
    const sampled = [points[0]];

    for (let bucket = 0; bucket < bucketCount; bucket++) {
        const start = 1 + Math.floor(bucket * bucketSize);
        const end = 1 + Math.floor((bucket + 1) * bucketSize);
        if (start >= total - 1) {
            break;
        }

        let minIdx = start;
        let maxIdx = start;
        for (let i = start; i < Math.min(end, total - 1); i++) {
            if (points[i].y < points[minIdx].y) {
                minIdx = i;
            }
            if (points[i].y > points[maxIdx].y) {
                maxIdx = i;
            }
        }

        if (minIdx === maxIdx) {
            sampled.push(points[minIdx]);
            continue;
        }

        if (minIdx < maxIdx) {
            sampled.push(points[minIdx], points[maxIdx]);
        } else {
            sampled.push(points[maxIdx], points[minIdx]);
        }
    }

    sampled.push(points[total - 1]);
    return sampled;
}

function eventTimeMs(recordedAtMs) {
    return recordedAtMs || Date.now();
}

function updateStreamTimestamp(streamName, recordedAtMs) {
    state.lastUpdateMs[streamName] = eventTimeMs(recordedAtMs);
}

function freshnessClass(timestampMs) {
    if (!timestampMs) {
        return "is-unknown";
    }

    const ageMs = Date.now() - timestampMs;
    if (ageMs <= 5000) {
        return "is-live";
    }
    if (ageMs <= 15000) {
        return "is-warn";
    }
    return "is-stale";
}

function freshnessLabel(timestampMs) {
    if (!timestampMs) {
        return "unknown";
    }

    const ageSec = Math.floor((Date.now() - timestampMs) / 1000);
    if (ageSec <= 5) {
        return "live";
    }
    return `stale ${ageSec}s`;
}

function metadataLastUpdatedLabel(timestampMs) {
    if (!timestampMs) {
        return "Metadata last updated: never";
    }

    return `Metadata last updated: ${new Date(timestampMs).toLocaleTimeString()}`;
}

function wsLabel() {
    if (state.wsState === "live") {
        return "connected";
    }
    if (state.wsState === "reconnecting") {
        return "reconnecting";
    }
    if (state.wsState === "error") {
        return "error";
    }
    return "connecting";
}

function parseShairportTime(timestamp, recordedAtMs) {
    const parsed = Date.parse(timestamp);
    if (Number.isNaN(parsed)) {
        return recordedAtMs || Date.now();
    }

    return parsed;
}

function pushShairport(sample, recordedAtMs) {
    updateStreamTimestamp("shairport", recordedAtMs);
    state.latestShairport = sample;
    const x = parseShairportTime(sample.timestamp, recordedAtMs);
    state.shairportSeries.av_sync_error_ms = keepLastN(state.shairportSeries.av_sync_error_ms, {
        x,
        y: sample.av_sync_error_ms,
    });
    state.shairportSeries.ppm = keepLastN(state.shairportSeries.ppm, { x, y: sample.ppm });
    state.shairportSeries.sync_window_ms = keepLastN(state.shairportSeries.sync_window_ms, {
        x,
        y: sample.sync_window_ms,
    });
    state.shairportSeries.missing = keepLastN(state.shairportSeries.missing, {
        x,
        y: sample.missing,
    });
    state.shairportSeries.resend = keepLastN(state.shairportSeries.resend, {
        x,
        y: sample.resend,
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

function pushShairportMetadata(sample, recordedAtMs) {
    updateStreamTimestamp("metadata", recordedAtMs);
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

function pushWifi(sample, recordedAtMs) {
    updateStreamTimestamp("wifi", recordedAtMs);
    state.latestWifi = sample;
    const x = sample.current_time_ms || recordedAtMs || Date.now();
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

function pushSystem(sample, recordedAtMs) {
    updateStreamTimestamp("system", recordedAtMs);
    state.latestSystem = sample;
    const x = sample.timestamp_ms || recordedAtMs || Date.now();

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

function pushIwEvent(sample, recordedAtMs) {
    updateStreamTimestamp("iw_event", recordedAtMs);
    const timestampMs = sample.timestamp_ms || recordedAtMs || Date.now();
    const line = String(sample.line || "").trim();
    if (!line) {
        return;
    }

    state.iwEventLog = appendBounded(
        state.iwEventLog,
        { timestampMs, line },
        MAX_IW_EVENT_LINES,
    );
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

function formatHms(totalSeconds) {
    if (totalSeconds === null || totalSeconds === undefined) {
        return "-";
    }

    const seconds = Math.max(0, Number(totalSeconds) || 0);
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = Math.floor(seconds % 60);

    return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

function MetricChart(props) {
    const canvasRef = useRef(null);
    const chartRef = useRef(null);

    useEffect(() => {
        if (!canvasRef.current) {
            return;
        }

        const { lineColor, mutedColor, gridColor } = getChartColors();

        chartRef.current = new ChartJS(canvasRef.current, {
            type: "line",
            data: {
                datasets: [
                    {
                        data: props.points || [],
                        parsing: false,
                        borderColor: lineColor,
                        pointBackgroundColor: lineColor,
                        borderWidth: 1.4,
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
                            color: mutedColor,
                            callback(value) {
                                return new Date(Number(value)).toLocaleTimeString();
                            },
                            maxTicksLimit: 4,
                        },
                        grid: {
                            color: gridColor,
                        },
                    },
                    y: {
                        min: props.yMin,
                        max: props.yMax,
                        ticks: {
                            color: mutedColor,
                            stepSize: props.yStep,
                            callback(value) {
                                if (props.yTickFormatter) {
                                    return props.yTickFormatter(value);
                                }
                                return value;
                            },
                        },
                        grid: {
                            color: gridColor,
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
    }, [props.theme]);

    useEffect(() => {
        if (!chartRef.current) {
            return;
        }

        const widthPx = canvasRef.current?.clientWidth || chartRef.current.width || 300;
        const targetRenderPoints = Math.max(80, Math.floor(widthPx / 2));
        chartRef.current.data.datasets[0].data = downsampleSeries(
            props.points || [],
            targetRenderPoints,
        );
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
    const iwEventLog = props.state.iwEventLog || [];
    const iwEventFilter = props.state.iwEventFilter || "";
    const normalizedIwFilter = iwEventFilter.trim().toLowerCase();
    const filteredIwEventLog = normalizedIwFilter
        ? iwEventLog.filter((entry) => entry.line.toLowerCase().includes(normalizedIwFilter))
        : iwEventLog;
    const nowPlayingPulseClass = props.state.nowPlayingPulseToken > 0 ? "pulse-highlight" : "";
    const shairportFreshness = freshnessClass(props.state.lastUpdateMs.shairport);
    const wifiFreshness = freshnessClass(props.state.lastUpdateMs.wifi);
    const systemFreshness = freshnessClass(props.state.lastUpdateMs.system);

    return html`
    <main>
        <section class="title-row">
            <div>
                <h1>Shairport Dashboard</h1>
                <p>Realtime shairport and wlan0 station telemetry</p>
            </div>
            <label class="theme-picker">
                Theme
                <select
                    class="theme-select"
                    value=${props.state.theme}
                    onchange=${(ev) => {
            applyTheme(ev.target.value);
            redraw();
        }}
                >
                    <option value="terminal">Terminal</option>
                    <option value="purple">Purple</option>
                    <option value="high-contrast">High Contrast</option>
                    <option value="black-white">Black / White</option>
                </select>
            </label>
        </section>

        <section class="status-strip" aria-live="polite">
            <span class=${`status-chip ws-chip ws-${props.state.wsState}`}>
                WS ${wsLabel()}
            </span>
            <span class=${`status-chip ${shairportFreshness}`}>
                Sync ${freshnessLabel(props.state.lastUpdateMs.shairport)}
            </span>
            <span class="status-chip metadata-chip">
                ${metadataLastUpdatedLabel(props.state.lastUpdateMs.metadata)}
            </span>
            <span class=${`status-chip ${wifiFreshness}`}>
                Wi-Fi ${freshnessLabel(props.state.lastUpdateMs.wifi)}
            </span>
            <span class=${`status-chip ${systemFreshness}`}>
                System ${freshnessLabel(props.state.lastUpdateMs.system)}
            </span>
        </section>

        <section class="meta-grid">
            <article key=${`top-now-playing-${props.state.nowPlayingPulseToken}`} class=${`meta-card ${nowPlayingPulseClass}`}>
                <h2>Now Playing</h2>
                <div class="meta-card-rows">
                    <span class="lbl">Track</span><span class="val">${latestShairportMetadata?.track || "-"}</span>
                    <span class="lbl">Artist</span><span class="val">${latestShairportMetadata?.artist || "-"}</span>
                    <span class="lbl">Album</span><span class="val">${latestShairportMetadata?.album || "-"}</span>
                    <span class="lbl">Genre</span><span class="val">${latestShairportMetadata?.genre || "-"}</span>
                </div>
            </article>
            <article class="meta-card">
                <h2>Wi-Fi Station</h2>
                <div class="meta-card-rows">
                    <span class="lbl">MAC</span><span class="val">${latestWifi ? latestWifi.station_mac : "-"}</span>
                    <span class="lbl">Interface</span><span class="val">${latestWifi ? latestWifi.interface_name : "-"}</span>
                    <span class="lbl">Connected</span><span class="val">${latestWifi ? formatHms(latestWifi.connected_time_seconds) : "-"}</span>
                    <span class="lbl">Power Save</span><span class="val">${latestWifi && latestWifi.power_save_enabled !== null ? (latestWifi.power_save_enabled ? "on" : "off") : "unknown"}</span>
                    <span class="lbl">Tx Failed</span><span class="val">${latestWifi ? latestWifi.tx_failed : "-"}</span>
                </div>
            </article>
            <article class="meta-card">
                <h2>Shairport</h2>
                <div class="meta-card-rows">
                    <span class="lbl">Timestamp</span><span class="val">${latestShairport ? latestShairport.timestamp : "-"}</span>
                    <span class="lbl">Missing</span><span class="val">${latestShairport ? latestShairport.missing : "-"}</span>
                    <span class="lbl">Resend</span><span class="val">${latestShairport ? latestShairport.resend : "-"}</span>
                </div>
            </article>
            <article class="meta-card">
                <h2>Raspberry Pi 5 System</h2>
                <div class="meta-card-rows">
                    <span class="lbl">Uptime</span><span class="val">${latestSystem && latestSystem.uptime_seconds !== null ? formatHms(latestSystem.uptime_seconds) : "unknown"}</span>
                    <span class="lbl">CPU Temp</span><span class="val">${latestSystem ? `${fmt(latestSystem.cpu_temp_c)} C` : "-"}</span>
                    <span class="lbl">CPU Usage</span><span class="val">${latestSystem ? `${fmt(latestSystem.cpu_usage_pct)} %` : "-"}</span>
                    <span class="lbl">RAM Usage</span><span class="val">${latestSystem ? `${fmt(latestSystem.ram_usage_pct)} %` : "-"}</span>
                    <span class="lbl">Fan</span><span class="val">${latestSystem ? `${latestSystem.fan_speed_rpm ?? "-"} rpm` : "-"}</span>
                    <span class="lbl">Throttled</span><span class="val">${latestSystem && latestSystem.throttled_now !== null ? (latestSystem.throttled_now ? "yes" : "no") : "unknown"}</span>
                </div>
            </article>
        </section>

        <section class="stream-section">
            <header class="stream-header">
                <h2>Now Playing</h2>
                <p>Audio sync drift and window behavior</p>
            </header>
            <div class="shairport-meta-wrap">
                <article key=${`stream-now-playing-${props.state.nowPlayingPulseToken}`} class=${`meta-card now-playing-card ${nowPlayingPulseClass}`}>
                    <p class="np-track">${latestShairportMetadata?.track || "\u2014"}</p>
                    <p class="np-artist">${latestShairportMetadata?.artist || "\u2014"}</p>
                    <div class="np-secondary">
                        <span class="np-album">${latestShairportMetadata?.album || "\u2014"}</span>
                        ${latestShairportMetadata?.genre
            ? html`<span class="np-sep">\u00b7</span><span class="np-genre">${latestShairportMetadata.genre}</span>`
            : ""}
                    </div>
                </article>
                <article class="meta-card art-card">
                    ${latestShairportMetadata?.artwork_url
            ? html`<img src=${latestShairportMetadata.artwork_url} alt="album art" class="art-image" />`
            : html`<div class="art-placeholder">No art</div>`}
                </article>
            </div>
            <div class="chart-grid">
                <${MetricChart}
                    theme=${props.state.theme}
                    title="AV Sync Error (ms)"
                    value=${latestShairport ? `${fmt(latestShairport.av_sync_error_ms)} ms` : "-"}
                    points=${props.state.shairportSeries.av_sync_error_ms}
                />
                <${MetricChart}
                    theme=${props.state.theme}
                    title="PPM"
                    value=${latestShairport ? fmt(latestShairport.ppm) : "-"}
                    points=${props.state.shairportSeries.ppm}
                />
                <${MetricChart}
                    theme=${props.state.theme}
                    title="Sync Window (ms)"
                    value=${latestShairport ? `${fmt(latestShairport.sync_window_ms)} ms` : "-"}
                    points=${props.state.shairportSeries.sync_window_ms}
                />
                <${MetricChart}
                    theme=${props.state.theme}
                    title="Missing"
                    value=${latestShairport ? fmt(latestShairport.missing, 0) : "-"}
                    points=${props.state.shairportSeries.missing}
                />
                <${MetricChart}
                    theme=${props.state.theme}
                    title="Resend"
                    value=${latestShairport ? fmt(latestShairport.resend, 0) : "-"}
                    points=${props.state.shairportSeries.resend}
                />
            </div>
        </section>



        <section class="stream-section">
            <header class="stream-header">
                <h2>Raspberry Pi 5 System</h2>
                <p>Thermals, utilization, cooling, and throttling</p>
            </header>
            <div class="chart-grid">
                <${MetricChart}
                    theme=${props.state.theme}
                    title="CPU Temp (C)"
                    value=${latestSystem ? `${fmt(latestSystem.cpu_temp_c)} C` : "-"}
                    points=${props.state.systemSeries.cpu_temp_c}
                />
                <${MetricChart}
                    theme=${props.state.theme}
                    title="CPU Usage (%)"
                    value=${latestSystem ? `${fmt(latestSystem.cpu_usage_pct)} %` : "-"}
                    points=${props.state.systemSeries.cpu_usage_pct}
                    yMin=${0}
                    yMax=${100}
                />
                <${MetricChart}
                    theme=${props.state.theme}
                    title="RAM Usage (%)"
                    value=${latestSystem ? `${fmt(latestSystem.ram_usage_pct)} %` : "-"}
                    points=${props.state.systemSeries.ram_usage_pct}
                    yMin=${0}
                    yMax=${100}
                />
                <${MetricChart}
                    theme=${props.state.theme}
                    title="Fan Speed (RPM)"
                    value=${latestSystem && latestSystem.fan_speed_rpm != null
            ? `${latestSystem.fan_speed_rpm} rpm`
            : "-"}
                    points=${props.state.systemSeries.fan_speed_rpm}
                />
                <${MetricChart}
                    theme=${props.state.theme}
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
                    theme=${props.state.theme}
                    title="Signal (dBm)"
                    value=${latestWifi ? `${fmt(latestWifi.signal_dbm, 0)} dBm` : "-"}
                    points=${props.state.wifiSeries.signal_dbm}
                />
                <${MetricChart}
                    theme=${props.state.theme}
                    title="TX Bitrate (MBit/s)"
                    value=${latestWifi ? `${fmt(latestWifi.tx_bitrate_mbit_s)} Mb/s` : "-"}
                    points=${props.state.wifiSeries.tx_bitrate_mbit_s}
                />
                <${MetricChart}
                    theme=${props.state.theme}
                    title="RX Bitrate (MBit/s)"
                    value=${latestWifi ? `${fmt(latestWifi.rx_bitrate_mbit_s)} Mb/s` : "-"}
                    points=${props.state.wifiSeries.rx_bitrate_mbit_s}
                />
                <${MetricChart}
                    theme=${props.state.theme}
                    title="TX Failed"
                    value=${latestWifi ? fmt(latestWifi.tx_failed, 0) : "-"}
                    points=${props.state.wifiSeries.tx_failed}
                />
            </div>
        </section>

        <section class="stream-section">
            <header class="stream-header">
                <h2>iw event -t</h2>
                <p>Kernel wireless events from iw</p>
            </header>
            <article class="meta-card log-card">
                <div class="event-log-toolbar">
                    <input
                        class="event-log-filter"
                        type="text"
                        value=${iwEventFilter}
                        placeholder="Filter events (e.g. connected, auth, disconnected)"
                        oninput=${(ev) => {
            state.iwEventFilter = ev.target.value;
            redraw();
        }}
                    />
                    <span class="event-log-count">${filteredIwEventLog.length}/${iwEventLog.length}</span>
                </div>
                <div class="event-log" role="log" aria-live="polite">
                    ${filteredIwEventLog.length
            ? filteredIwEventLog.map((entry) => html`
                                <div class="event-log-line">
                                    <span class="event-log-ts">${new Date(entry.timestampMs).toLocaleTimeString()}</span>
                                    <span class="event-log-msg">${entry.line}</span>
                                </div>
                            `)
            : html`<div class="event-log-empty">${iwEventLog.length ? "No matching iw events for this filter." : "No iw events yet."}</div>`}
                </div>
            </article>
        </section>
    </main>
  `;
}

function redraw() {
    render(html`<${App} state=${state}></${App}>`, document.body);
}

const wsUrl = new URL("/ws", window.location.href);
wsUrl.protocol = wsUrl.protocol.replace("http", "ws");

function connect() {
    state.wsState = "connecting";
    redraw();
    const ws = new WebSocket(wsUrl.href);

    ws.onopen = () => {
        state.wsState = "live";
        redraw();
    };

    ws.onmessage = (ev) => {
        let event = JSON.parse(ev.data);
        const recordedAtMs = event.recorded_at_ms;
        state.wsLastMessageMs = eventTimeMs(recordedAtMs);
        if (event.kind === "Shairport") {
            pushShairport(event.payload, recordedAtMs);
        }
        if (event.kind === "WifiStation") {
            pushWifi(event.payload, recordedAtMs);
        }
        if (event.kind === "System") {
            pushSystem(event.payload, recordedAtMs);
        }
        if (event.kind === "ShairportMetadata") {
            pushShairportMetadata(event.payload, recordedAtMs);
        }
        if (event.kind === "IwEvent") {
            pushIwEvent(event.payload, recordedAtMs);
        }
        redraw();
    };

    ws.onclose = () => {
        state.wsState = "reconnecting";
        redraw();
        setTimeout(connect, 1000);
    };
    ws.onerror = () => {
        state.wsState = "error";
        redraw();
        ws.close();
    };
}

initTheme();
redraw();
connect();