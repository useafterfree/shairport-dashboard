use base64::{Engine as _, engine::general_purpose};
use rust_stats::models::{SampleEvent, ShairportMetadataSample};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;
use tokio::time::{Duration, sleep};

const META_FIFO_PATH: &str = "/tmp/shairport-sync-metadata";

#[derive(Default)]
struct MetadataParserState {
    current_type: String,
    current_code: String,
    track: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    artwork_base64: Option<String>,
    data_buffer: String,
    in_data_tag: bool,
}

impl MetadataParserState {
    fn parse_line(&mut self, line: &str) -> Option<ShairportMetadataSample> {
        // Handle data tag that spans multiple lines
        if self.in_data_tag {
            if line.contains("</data>") {
                // End of data tag - extract content before </data>
                if let Some(end_pos) = line.find("</data>") {
                    self.data_buffer.push_str(&line[..end_pos]);
                }
                self.in_data_tag = false;

                // Process accumulated data
                let data = self.data_buffer.trim().to_string();
                self.data_buffer.clear();
                return self.process_data(&data);
            } else {
                // Continue buffering data
                self.data_buffer.push_str(line);
                self.data_buffer.push('\n');
                return None;
            }
        }

        if let Some(type_hex) = extract_tag_content(line, "type") {
            self.current_type = decode_xml_hex(&type_hex);
        }

        if let Some(code_hex) = extract_tag_content(line, "code") {
            self.current_code = decode_xml_hex(&code_hex);
        }

        // Check if data tag is starting
        if line.contains("<data") {
            if let Some(start_pos) = line.find(">") {
                if let Some(end_pos) = line.find("</data>") {
                    // Single line data tag
                    let content_start = start_pos + 1;
                    let content = line[content_start..end_pos].trim().to_string();
                    return self.process_data(&content);
                } else {
                    // Multi-line data tag - start buffering
                    self.in_data_tag = true;
                    let content_start = start_pos + 1;
                    self.data_buffer = line[content_start..].to_string();
                    self.data_buffer.push('\n');
                    return None;
                }
            }
        }

        if line.contains("</item>") {
            self.current_type.clear();
            self.current_code.clear();
        }

        None
    }

    fn process_data(&mut self, data: &str) -> Option<ShairportMetadataSample> {
        let mut changed = false;
        match (self.current_type.as_str(), self.current_code.as_str()) {
            ("core", "minm") => {
                self.track = Some(decode_xml_b64_utf8(data));
                changed = true;
            }
            ("core", "asar") => {
                self.artist = Some(decode_xml_b64_utf8(data));
                changed = true;
            }
            ("core", "asal") => {
                self.album = Some(decode_xml_b64_utf8(data));
                changed = true;
            }
            ("ssnc", "PICT") => {
                self.artwork_base64 = Some(data.trim().to_string());
                changed = true;
            }
            _ => {}
        }

        if changed {
            Some(ShairportMetadataSample {
                timestamp_ms: now_ms(),
                track: self.track.clone(),
                artist: self.artist.clone(),
                album: self.album.clone(),
                artwork_base64: self.artwork_base64.clone(),
            })
        } else {
            None
        }
    }
}

pub async fn stream_shairport_metadata(tx: broadcast::Sender<SampleEvent>) {
    loop {
        let tx_clone = tx.clone();
        let result = tokio::task::spawn_blocking(move || parse_fifo_once(tx_clone)).await;

        match result {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                eprintln!("metadata pipe read failed: {err}");
            }
            Err(err) => {
                eprintln!("metadata parser task failed: {err}");
            }
        }

        sleep(Duration::from_secs(1)).await;
    }
}

fn parse_fifo_once(tx: broadcast::Sender<SampleEvent>) -> std::io::Result<()> {
    let file = File::open(META_FIFO_PATH)?;
    let reader = BufReader::new(file);
    let mut parser = MetadataParserState::default();

    for line in reader.lines() {
        let line = line?;
        if let Some(sample) = parser.parse_line(&line) {
            let _ = tx.send(SampleEvent::ShairportMetadata(sample));
        }
    }

    Ok(())
}

fn extract_tag_content(line: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let start = line.find(&open)?;
    let after_open = &line[start..];
    let gt = after_open.find('>')?;
    let content_start = start + gt + 1;
    let close = format!("</{tag}>");
    let content_end = line[content_start..].find(&close)? + content_start;
    Some(line[content_start..content_end].to_string())
}

fn decode_xml_hex(raw: &str) -> String {
    let trimmed = raw.trim();
    match hex::decode(trimmed) {
        Ok(bytes) => String::from_utf8(bytes).unwrap_or_default(),
        Err(_) => String::new(),
    }
}

fn decode_xml_b64_utf8(raw: &str) -> String {
    let trimmed = raw.trim();
    match general_purpose::STANDARD.decode(trimmed) {
        Ok(bytes) => String::from_utf8(bytes).unwrap_or_else(|_| trimmed.to_string()),
        Err(_) => trimmed.to_string(),
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
