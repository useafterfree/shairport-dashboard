pub mod models;
pub use models::WifiStationSample;

#[derive(Default)]
struct WifiStationSampleBuilder {
    station_mac: Option<String>,
    interface_name: Option<String>,
    power_save_enabled: Option<bool>,
    inactive_time_ms: Option<u64>,
    rx_bytes: Option<u64>,
    rx_packets: Option<u64>,
    tx_bytes: Option<u64>,
    tx_packets: Option<u64>,
    tx_failed: Option<u64>,
    signal_dbm: Option<i32>,
    tx_bitrate_mbit_s: Option<f64>,
    rx_bitrate_mbit_s: Option<f64>,
    authorized: Option<bool>,
    authenticated: Option<bool>,
    associated: Option<bool>,
    wmm_wme: Option<bool>,
    tdls_peer: Option<bool>,
    dtim_period: Option<u32>,
    beacon_interval: Option<u32>,
    short_preamble: Option<bool>,
    short_slot_time: Option<bool>,
    connected_time_seconds: Option<u64>,
    current_time_ms: Option<u64>,
}

impl WifiStationSampleBuilder {
    fn build(self) -> Option<WifiStationSample> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Some(WifiStationSample {
            station_mac: self.station_mac?,
            interface_name: self.interface_name?,
            power_save_enabled: self.power_save_enabled,
            inactive_time_ms: self.inactive_time_ms.unwrap_or(0),
            rx_bytes: self.rx_bytes.unwrap_or(0),
            rx_packets: self.rx_packets.unwrap_or(0),
            tx_bytes: self.tx_bytes.unwrap_or(0),
            tx_packets: self.tx_packets.unwrap_or(0),
            tx_failed: self.tx_failed.unwrap_or(0),
            signal_dbm: self.signal_dbm.unwrap_or(0),
            tx_bitrate_mbit_s: self.tx_bitrate_mbit_s.unwrap_or(0.0),
            rx_bitrate_mbit_s: self.rx_bitrate_mbit_s.unwrap_or(0.0),
            authorized: self.authorized.unwrap_or(false),
            authenticated: self.authenticated.unwrap_or(false),
            associated: self.associated.unwrap_or(false),
            wmm_wme: self.wmm_wme.unwrap_or(false),
            tdls_peer: self.tdls_peer.unwrap_or(false),
            dtim_period: self.dtim_period.unwrap_or(0),
            beacon_interval: self.beacon_interval.unwrap_or(0),
            short_preamble: self.short_preamble.unwrap_or(false),
            short_slot_time: self.short_slot_time.unwrap_or(false),
            connected_time_seconds: self.connected_time_seconds.unwrap_or(0),
            current_time_ms: self.current_time_ms.unwrap_or(now_ms),
        })
    }
}

#[derive(Default)]
pub struct WifiStationParser {
    builder: Option<WifiStationSampleBuilder>,
}

impl WifiStationParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn parse_line(&mut self, raw_line: &str) -> Option<WifiStationSample> {
        let clean = strip_ansi_and_cr(raw_line);
        let line = clean.trim();

        if line.is_empty() || line.starts_with("Every ") {
            return None;
        }

        if let Some((station_mac, interface_name)) = parse_station_header(line) {
            self.builder = Some(WifiStationSampleBuilder {
                station_mac: Some(station_mac),
                interface_name: Some(interface_name),
                ..Default::default()
            });
            return None;
        }

        let active = self.builder.as_mut()?;
        let (key, value) = line.split_once(':')?;
        let key = key.trim();
        let value = value.trim();

        match key {
            "power save" => active.power_save_enabled = parse_bool_flag(value),
            "inactive time" => active.inactive_time_ms = parse_first_token(value),
            "rx bytes" => active.rx_bytes = parse_first_token(value),
            "rx packets" => active.rx_packets = parse_first_token(value),
            "tx bytes" => active.tx_bytes = parse_first_token(value),
            "tx packets" => active.tx_packets = parse_first_token(value),
            "tx failed" => active.tx_failed = parse_first_token(value),
            "signal" => active.signal_dbm = parse_first_token(value),
            "tx bitrate" => active.tx_bitrate_mbit_s = parse_first_token(value),
            "rx bitrate" => active.rx_bitrate_mbit_s = parse_first_token(value),
            "authorized" => active.authorized = parse_bool_yes_no(value),
            "authenticated" => active.authenticated = parse_bool_yes_no(value),
            "associated" => active.associated = parse_bool_yes_no(value),
            "WMM/WME" => active.wmm_wme = parse_bool_yes_no(value),
            "TDLS peer" => active.tdls_peer = parse_bool_yes_no(value),
            "DTIM period" => active.dtim_period = parse_first_token(value),
            "beacon interval" => active.beacon_interval = parse_first_token(value),
            "short preamble" => active.short_preamble = parse_bool_yes_no(value),
            "short slot time" => active.short_slot_time = parse_bool_yes_no(value),
            "connected time" => active.connected_time_seconds = parse_first_token(value),
            "current time" => {
                active.current_time_ms = parse_first_token(value);
                let finished = std::mem::take(&mut self.builder)?;
                return finished.build();
            }
            _ => {}
        }

        None
    }

    pub fn finish(&mut self) -> Option<WifiStationSample> {
        let finished = std::mem::take(&mut self.builder)?;
        finished.build()
    }
}

fn strip_ansi_and_cr(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            0x1B => {
                i += 1;
                if i < bytes.len() && bytes[i] == b'[' {
                    i += 1;
                    while i < bytes.len() && !(0x40..=0x7E).contains(&bytes[i]) {
                        i += 1;
                    }
                    if i < bytes.len() {
                        i += 1;
                    }
                }
            }
            b'\r' => i += 1,
            b => {
                out.push(b as char);
                i += 1;
            }
        }
    }

    out
}

fn parse_bool_yes_no(value: &str) -> Option<bool> {
    match value {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

fn parse_bool_flag(value: &str) -> Option<bool> {
    match value {
        "yes" | "on" => Some(true),
        "no" | "off" => Some(false),
        _ => None,
    }
}

fn parse_first_token<T: std::str::FromStr>(value: &str) -> Option<T> {
    value.split_whitespace().next()?.parse().ok()
}

fn parse_station_header(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("Station ")?;
    let (mac, tail) = rest.split_once(' ')?;
    let iface = tail
        .trim()
        .strip_prefix("(on ")?
        .strip_suffix(')')?
        .to_string();

    Some((mac.to_string(), iface))
}
