use shairport_dashboard::models::{IwEventSample, SampleEvent};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::broadcast;
use tokio::time::{Duration, sleep};

pub async fn stream_iw_events(tx: broadcast::Sender<SampleEvent>) {
    loop {
        let mut child = match Command::new("iw")
            .args(["event", "-t"])
            .stdout(std::process::Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(err) => {
                eprintln!("failed to start 'iw event -t': {err}");
                sleep(Duration::from_secs(2)).await;
                continue;
            }
        };

        let Some(stdout) = child.stdout.take() else {
            eprintln!("'iw event -t' had no stdout");
            sleep(Duration::from_secs(2)).await;
            continue;
        };

        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let sample = IwEventSample {
                timestamp_ms: now_ms(),
                line: trimmed.to_string(),
            };
            let _ = tx.send(SampleEvent::IwEvent(sample));
        }

        if let Err(err) = child.wait().await {
            eprintln!("'iw event -t' exited with wait error: {err}");
        }

        sleep(Duration::from_secs(1)).await;
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
