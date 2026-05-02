use regex::Regex;
use shairport_dashboard::models::{SampleEvent, ShairportSample};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::broadcast;

pub async fn stream_shairport_logs(tx: broadcast::Sender<SampleEvent>) {
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
