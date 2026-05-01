use rust_stats::WifiStationParser;
use rust_stats::models::SampleEvent;
use tokio::process::Command;
use tokio::sync::broadcast;
use tokio::time::{Duration, MissedTickBehavior, interval};

pub async fn stream_wlan_station_dump(tx: broadcast::Sender<SampleEvent>) {
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
