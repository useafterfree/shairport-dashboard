use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct WifiStationSample {
    pub station_mac: String,
    pub interface_name: String,
    pub inactive_time_ms: u64,
    pub rx_bytes: u64,
    pub rx_packets: u64,
    pub tx_bytes: u64,
    pub tx_packets: u64,
    pub tx_failed: u64,
    pub signal_dbm: i32,
    pub tx_bitrate_mbit_s: f64,
    pub rx_bitrate_mbit_s: f64,
    pub authorized: bool,
    pub authenticated: bool,
    pub associated: bool,
    pub wmm_wme: bool,
    pub tdls_peer: bool,
    pub dtim_period: u32,
    pub beacon_interval: u32,
    pub short_preamble: bool,
    pub short_slot_time: bool,
    pub connected_time_seconds: u64,
    pub current_time_ms: u64,
}

#[derive(Serialize, Clone)]
pub struct ShairportSample {
    pub timestamp: String,
    pub av_sync_error_ms: f64,
    pub ppm: f64,
    pub sync_window_ms: f64,
    pub missing: u32,
    pub resend: u32,
    pub fps_r: Option<f64>,
    pub fps_c: Option<f64>,
}

#[derive(Serialize, Clone)]
pub struct SystemSample {
    pub timestamp_ms: u64,
    pub cpu_temp_c: f64,
    pub cpu_usage_pct: f64,
    pub ram_usage_pct: f64,
    pub fan_speed_rpm: Option<u32>,
    pub fan_speeds_rpm: Vec<u32>,
    pub throttled_now: Option<bool>,
}

#[derive(Serialize, Clone)]
#[serde(tag = "kind", content = "payload")]
pub enum SampleEvent {
    Shairport(ShairportSample),
    WifiStation(WifiStationSample),
    System(SystemSample),
}
