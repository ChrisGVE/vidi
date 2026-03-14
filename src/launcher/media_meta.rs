//! ffprobe JSON parsing and metadata formatting for media files.

use serde::Deserialize;

// ---------------------------------------------------------------------------
// ffprobe JSON types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
pub struct FfprobeOutput {
    #[serde(default)]
    pub streams: Vec<FfprobeStream>,
    #[serde(default)]
    pub format: FfprobeFormat,
}

#[derive(Debug, Deserialize, Default)]
pub struct FfprobeFormat {
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub format_long_name: String,
    #[serde(default)]
    pub duration: String,
    #[serde(default)]
    pub bit_rate: String,
    #[serde(default)]
    pub size: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct FfprobeStream {
    #[serde(default)]
    pub codec_type: String,
    #[serde(default)]
    pub codec_long_name: String,
    #[serde(default)]
    pub codec_name: String,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub sample_rate: String,
    #[serde(default)]
    pub channels: Option<u32>,
    #[serde(default)]
    pub channel_layout: String,
    #[serde(default)]
    pub index: u32,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse the JSON output produced by `ffprobe -print_format json`.
pub fn parse_ffprobe_json(json: &str) -> Option<FfprobeOutput> {
    serde_json::from_str(json).ok()
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

/// Format an `FfprobeOutput` into a human-readable metadata table.
///
/// Returns the formatted string ready to print to stdout.
pub fn format_metadata(meta: &FfprobeOutput) -> String {
    let mut out = String::with_capacity(512);

    // Container / format block
    out.push_str("── Container ──────────────────────────────\n");
    let filename = basename(&meta.format.filename);
    if !filename.is_empty() {
        out.push_str(&format!("  File     : {filename}\n"));
    }
    if !meta.format.format_long_name.is_empty() {
        out.push_str(&format!("  Format   : {}\n", meta.format.format_long_name));
    }
    let dur = format_duration(&meta.format.duration);
    if !dur.is_empty() {
        out.push_str(&format!("  Duration : {dur}\n"));
    }
    if !meta.format.bit_rate.is_empty() {
        let kbps = bitrate_kbps(&meta.format.bit_rate);
        out.push_str(&format!("  Bitrate  : {kbps} kbps\n"));
    }
    if !meta.format.size.is_empty() {
        let human = human_size(&meta.format.size);
        out.push_str(&format!("  Size     : {human}\n"));
    }

    // Per-stream blocks
    for stream in &meta.streams {
        format_stream(&mut out, stream);
    }

    out
}

fn format_stream(out: &mut String, s: &FfprobeStream) {
    let label = match s.codec_type.as_str() {
        "video" => "Video",
        "audio" => "Audio",
        _ => "Stream",
    };
    out.push_str(&format!(
        "── {label} stream #{} ─────────────────────────\n",
        s.index
    ));
    let codec = if s.codec_long_name.is_empty() {
        s.codec_name.clone()
    } else {
        s.codec_long_name.clone()
    };
    if !codec.is_empty() {
        out.push_str(&format!("  Codec    : {codec}\n"));
    }
    if let (Some(w), Some(h)) = (s.width, s.height) {
        out.push_str(&format!("  Resolution: {w}x{h}\n"));
    }
    if !s.sample_rate.is_empty() {
        out.push_str(&format!("  Sample rate: {} Hz\n", s.sample_rate));
    }
    if let Some(ch) = s.channels {
        if s.channel_layout.is_empty() {
            out.push_str(&format!("  Channels : {ch}\n"));
        } else {
            out.push_str(&format!("  Channels : {ch} ({})\n", s.channel_layout));
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn basename(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn format_duration(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let secs: f64 = raw.parse().unwrap_or(0.0);
    let total = secs as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

fn bitrate_kbps(raw: &str) -> String {
    if let Ok(bps) = raw.parse::<u64>() {
        (bps / 1000).to_string()
    } else {
        raw.to_string()
    }
}

fn human_size(raw: &str) -> String {
    if let Ok(bytes) = raw.parse::<u64>() {
        if bytes >= 1_000_000_000 {
            format!("{:.1} GB", bytes as f64 / 1_000_000_000.0)
        } else if bytes >= 1_000_000 {
            format!("{:.1} MB", bytes as f64 / 1_000_000.0)
        } else if bytes >= 1_000 {
            format!("{:.1} KB", bytes as f64 / 1_000.0)
        } else {
            format!("{bytes} B")
        }
    } else {
        raw.to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_JSON: &str = r#"{
        "streams": [
            {
                "index": 0,
                "codec_type": "video",
                "codec_name": "h264",
                "codec_long_name": "H.264 / AVC / MPEG-4 AVC / MPEG-4 part 10",
                "width": 1920,
                "height": 1080
            },
            {
                "index": 1,
                "codec_type": "audio",
                "codec_name": "aac",
                "codec_long_name": "AAC (Advanced Audio Coding)",
                "sample_rate": "44100",
                "channels": 2,
                "channel_layout": "stereo"
            }
        ],
        "format": {
            "filename": "/tmp/sample.mp4",
            "format_long_name": "QuickTime / MOV",
            "duration": "125.5",
            "bit_rate": "2500000",
            "size": "39218750"
        }
    }"#;

    #[test]
    fn parse_sample_json_succeeds() {
        let meta = parse_ffprobe_json(SAMPLE_JSON);
        assert!(meta.is_some(), "should parse valid ffprobe JSON");
    }

    #[test]
    fn format_metadata_contains_filename() {
        let meta = parse_ffprobe_json(SAMPLE_JSON).unwrap();
        let out = format_metadata(&meta);
        assert!(out.contains("sample.mp4"), "output should contain filename");
    }

    #[test]
    fn format_metadata_contains_resolution() {
        let meta = parse_ffprobe_json(SAMPLE_JSON).unwrap();
        let out = format_metadata(&meta);
        assert!(
            out.contains("1920x1080"),
            "output should contain resolution"
        );
    }

    #[test]
    fn format_duration_seconds_under_hour() {
        assert_eq!(format_duration("125.5"), "2:05");
    }

    #[test]
    fn format_duration_over_one_hour() {
        assert_eq!(format_duration("3900.0"), "1:05:00");
    }

    #[test]
    fn format_duration_empty_string_returns_empty() {
        assert_eq!(format_duration(""), "");
    }

    #[test]
    fn bitrate_kbps_converts_correctly() {
        assert_eq!(bitrate_kbps("2500000"), "2500");
    }

    #[test]
    fn human_size_megabytes() {
        let s = human_size("39218750");
        assert!(s.contains("MB"), "expected MB, got: {s}");
    }

    #[test]
    fn parse_invalid_json_returns_none() {
        let result = parse_ffprobe_json("not json");
        assert!(result.is_none());
    }

    #[test]
    fn format_metadata_contains_bitrate_kbps() {
        let meta = parse_ffprobe_json(SAMPLE_JSON).unwrap();
        let out = format_metadata(&meta);
        assert!(
            out.contains("2500"),
            "output should contain bitrate in kbps"
        );
    }

    #[test]
    fn format_metadata_audio_stream_shows_channels() {
        let meta = parse_ffprobe_json(SAMPLE_JSON).unwrap();
        let out = format_metadata(&meta);
        assert!(
            out.contains("stereo"),
            "output should contain channel layout"
        );
    }
}
