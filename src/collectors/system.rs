use shairport_dashboard::models::{SampleEvent, SystemSample};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::process::Command;
use tokio::sync::broadcast;
use tokio::time::{Duration, MissedTickBehavior, interval};

pub async fn stream_system_stats(tx: broadcast::Sender<SampleEvent>) {
    let mut ticker = interval(Duration::from_secs(1));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut prev_cpu: Option<(u64, u64)> = None;

    loop {
        ticker.tick().await;

        let cpu_temp_c = read_cpu_temp_c().await.unwrap_or(0.0);
        let ram_usage_pct = read_ram_usage_pct().await.unwrap_or(0.0);
        let fan_speeds_rpm = read_fan_speeds_rpm();
        let fan_speed_rpm = fan_speeds_rpm.iter().copied().max();
        let throttled_now = read_throttled_now().await;

        let cpu_usage_pct = if let Some((idle, total)) = read_cpu_totals().await {
            let usage = if let Some((prev_idle, prev_total)) = prev_cpu {
                let idle_delta = idle.saturating_sub(prev_idle);
                let total_delta = total.saturating_sub(prev_total);
                if total_delta == 0 {
                    0.0
                } else {
                    (1.0 - idle_delta as f64 / total_delta as f64) * 100.0
                }
            } else {
                0.0
            };

            prev_cpu = Some((idle, total));
            usage
        } else {
            0.0
        };

        let sample = SystemSample {
            timestamp_ms: now_ms(),
            cpu_temp_c,
            cpu_usage_pct,
            ram_usage_pct,
            fan_speed_rpm,
            fan_speeds_rpm,
            throttled_now,
        };

        let _ = tx.send(SampleEvent::System(sample));
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

async fn read_cpu_temp_c() -> Option<f64> {
    let raw = tokio::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
        .await
        .ok()?;
    let millic = raw.trim().parse::<f64>().ok()?;
    Some(millic / 1000.0)
}

async fn read_cpu_totals() -> Option<(u64, u64)> {
    let stat = tokio::fs::read_to_string("/proc/stat").await.ok()?;
    let line = stat.lines().next()?;
    let mut parts = line.split_whitespace();
    if parts.next()? != "cpu" {
        return None;
    }

    let values: Vec<u64> = parts.filter_map(|v| v.parse::<u64>().ok()).collect();
    if values.len() < 4 {
        return None;
    }

    let idle = values[3] + values.get(4).copied().unwrap_or(0);
    let total = values.iter().sum();
    Some((idle, total))
}

fn parse_meminfo_kb(meminfo: &str, key: &str) -> Option<u64> {
    let line = meminfo.lines().find(|line| line.starts_with(key))?;
    line.split_whitespace().nth(1)?.parse::<u64>().ok()
}

async fn read_ram_usage_pct() -> Option<f64> {
    let meminfo = tokio::fs::read_to_string("/proc/meminfo").await.ok()?;
    let total = parse_meminfo_kb(&meminfo, "MemTotal:")?;
    let available = parse_meminfo_kb(&meminfo, "MemAvailable:")?;
    if total == 0 {
        return None;
    }

    let used = total.saturating_sub(available) as f64;
    Some((used / total as f64) * 100.0)
}

fn collect_fan_rpms_from(dir: &Path, out: &mut Vec<u32>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        if !name.starts_with("fan") || !name.ends_with("_input") {
            continue;
        }

        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(rpm) = content.trim().parse::<u32>() {
                out.push(rpm);
            }
        }
    }
}

fn read_fan_speeds_rpm() -> Vec<u32> {
    let mut rpms = Vec::new();

    if let Ok(hwmons) = std::fs::read_dir("/sys/class/hwmon") {
        for hwmon in hwmons.filter_map(Result::ok) {
            collect_fan_rpms_from(&hwmon.path(), &mut rpms);
        }
    }

    rpms.sort_unstable();
    rpms
}

async fn read_throttled_now() -> Option<bool> {
    let output = Command::new("vcgencmd")
        .arg("get_throttled")
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8(output.stdout).ok()?;
    let hex = text.trim().split("0x").nth(1)?;
    let flags = u32::from_str_radix(hex, 16).ok()?;

    // Bit 2 indicates currently throttled.
    Some((flags & (1 << 2)) != 0)
}
