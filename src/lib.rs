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

#[derive(Default)]
struct WifiStationSampleBuilder {
    station_mac: Option<String>,
    interface_name: Option<String>,
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
        Some(WifiStationSample {
            station_mac: self.station_mac?,
            interface_name: self.interface_name?,
            inactive_time_ms: self.inactive_time_ms?,
            rx_bytes: self.rx_bytes?,
            rx_packets: self.rx_packets?,
            tx_bytes: self.tx_bytes?,
            tx_packets: self.tx_packets?,
            tx_failed: self.tx_failed?,
            signal_dbm: self.signal_dbm?,
            tx_bitrate_mbit_s: self.tx_bitrate_mbit_s?,
            rx_bitrate_mbit_s: self.rx_bitrate_mbit_s?,
            authorized: self.authorized?,
            authenticated: self.authenticated?,
            associated: self.associated?,
            wmm_wme: self.wmm_wme?,
            tdls_peer: self.tdls_peer?,
            dtim_period: self.dtim_period?,
            beacon_interval: self.beacon_interval?,
            short_preamble: self.short_preamble?,
            short_slot_time: self.short_slot_time?,
            connected_time_seconds: self.connected_time_seconds?,
            current_time_ms: self.current_time_ms?,
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
