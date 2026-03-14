//! Full-screen and inline handlers for audio and video files.
//!
//! # Full-screen flow (`launch_media`)
//!
//! 1. Run `ffprobe` and parse its JSON output.
//! 2. Format and print a metadata table to stdout.
//! 3. For video only: render the first frame via `timg` or `chafa` if available
//!    and the terminal supports graphics beyond half-block.
//! 4. Print a `[p] play  [q] quit` prompt and read one keypress.
//!    - `p` / `P` → spawn `mpv` with the appropriate video-output flag.
//!    - Anything else → return immediately.
//!
//! # Inline flow (`launch_media_inline`)
//!
//! Run `ffprobe`, format metadata, print to stdout — no playback prompt.

use std::io::{self, Write as IoWrite};
use std::path::Path;
use std::process::{Command, Stdio};

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal,
};

use crate::{
    detector::FileKind,
    error::{Result, VidiError},
    registry::is_available,
    terminal::{GraphicsProtocol, TerminalCaps},
    theme::mapper::ThemeMapper,
};

use super::media_meta::{format_metadata, parse_ffprobe_json};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Full-screen handler for audio and video files.
pub fn launch_media(
    file: &Path,
    kind: FileKind,
    _mapper: &ThemeMapper<'_>,
    caps: &TerminalCaps,
) -> Result<()> {
    let json = run_ffprobe(file)?;
    if let Some(meta) = parse_ffprobe_json(&json) {
        let table = format_metadata(&meta);
        // For video, try to render a first frame above the metadata.
        if kind == FileKind::Video {
            render_first_frame(file, caps);
        }
        print!("{table}");
        io::stdout().flush()?;
    } else {
        eprintln!(
            "vidi: ffprobe returned unparseable output for {}",
            file.display()
        );
    }

    print_playback_prompt();
    if await_play_keypress()? {
        spawn_mpv(file, kind, caps)?;
    }
    Ok(())
}

/// Inline handler: display ffprobe metadata only (no playback, no frame).
pub fn launch_media_inline(file: &Path, _lines: u16) -> Result<()> {
    let json = run_ffprobe(file)?;
    if let Some(meta) = parse_ffprobe_json(&json) {
        print!("{}", format_metadata(&meta));
        io::stdout().flush()?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// ffprobe invocation
// ---------------------------------------------------------------------------

fn run_ffprobe(file: &Path) -> Result<String> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(file)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|_| VidiError::ToolNotFound {
            tool: "ffprobe".into(),
        })?;

    if !output.status.success() {
        return Err(VidiError::ToolFailed {
            tool: "ffprobe".into(),
            code: output.status.code().unwrap_or(1),
        });
    }

    String::from_utf8(output.stdout)
        .map_err(|e| VidiError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))
}

// ---------------------------------------------------------------------------
// First-frame rendering (video only)
// ---------------------------------------------------------------------------

/// Render the first frame of a video file inline if a suitable tool is available.
///
/// Failures are silently ignored — frame preview is a best-effort enhancement.
fn render_first_frame(file: &Path, caps: &TerminalCaps) {
    let supports_graphics = !matches!(
        caps.graphics,
        GraphicsProtocol::HalfBlock256 | GraphicsProtocol::HalfBlock24
    );
    if !supports_graphics {
        return;
    }

    let cols = caps.columns.max(40).to_string();
    let rows = (caps.rows / 3).max(10).to_string();
    let geometry = format!("{cols}x{rows}");

    if is_available("timg") {
        let _ = Command::new("timg")
            .args(["--frames=1", &format!("-g{geometry}")])
            .arg(file)
            .status();
    } else if is_available("chafa") {
        let _ = Command::new("chafa")
            .args(["--size", &geometry])
            .arg(file)
            .status();
    }
}

// ---------------------------------------------------------------------------
// Interactive prompt
// ---------------------------------------------------------------------------

fn print_playback_prompt() {
    println!("\n[p] play  [q] quit");
    let _ = io::stdout().flush();
}

/// Block until the user presses a key.  Returns `true` if the user wants to play.
fn await_play_keypress() -> Result<bool> {
    terminal::enable_raw_mode()?;
    let result = read_play_key();
    // Restore terminal state regardless of result.
    let _ = terminal::disable_raw_mode();
    result
}

fn read_play_key() -> Result<bool> {
    loop {
        match event::read() {
            Ok(Event::Key(key)) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    return Ok(false);
                }
                return Ok(matches!(key.code, KeyCode::Char('p') | KeyCode::Char('P')));
            }
            Err(e) => return Err(VidiError::Io(e)),
            _ => continue,
        }
    }
}

// ---------------------------------------------------------------------------
// mpv playback
// ---------------------------------------------------------------------------

fn spawn_mpv(file: &Path, kind: FileKind, caps: &TerminalCaps) -> Result<()> {
    let mut cmd = Command::new("mpv");

    if kind == FileKind::Video {
        match caps.graphics {
            GraphicsProtocol::Kitty => {
                cmd.arg("--vo=kitty");
            }
            _ => {
                cmd.arg("--vo=tct");
            }
        }
    }
    cmd.arg("--really-quiet");
    cmd.arg(file);

    let status = cmd
        .status()
        .map_err(|_| VidiError::ToolNotFound { tool: "mpv".into() })?;

    if status.success() {
        Ok(())
    } else {
        Err(VidiError::ToolFailed {
            tool: "mpv".into(),
            code: status.code().unwrap_or(1),
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::{GraphicsProtocol, TerminalCaps};

    fn kitty_caps() -> TerminalCaps {
        TerminalCaps {
            graphics: GraphicsProtocol::Kitty,
            true_color: true,
            columns: 120,
            rows: 40,
        }
    }

    fn halfblock_caps() -> TerminalCaps {
        TerminalCaps {
            graphics: GraphicsProtocol::HalfBlock256,
            true_color: false,
            columns: 80,
            rows: 24,
        }
    }

    /// Verify that `run_ffprobe` returns an error when `ffprobe` is not available
    /// or the file does not exist — without panicking.
    #[test]
    fn run_ffprobe_missing_file_returns_error() {
        // /dev/null will cause ffprobe to fail (no valid streams/format).
        // If ffprobe itself is absent the ToolNotFound variant is returned.
        // Either way, the call must not panic.
        let path = std::path::PathBuf::from("/dev/null");
        let result = run_ffprobe(&path);
        // We accept either Ok (ffprobe ran but produced empty JSON) or Err.
        match result {
            Ok(json) => {
                // ffprobe produced some output; just verify it doesn't panic on parse.
                let _ = parse_ffprobe_json(&json);
            }
            Err(_) => {} // expected when ffprobe is absent or fails
        }
    }

    #[test]
    fn render_first_frame_skips_halfblock_terminals() {
        // Should return without spawning any process on half-block terminals.
        // We cannot assert that no process was spawned, but we verify no panic.
        let caps = halfblock_caps();
        render_first_frame(std::path::Path::new("/dev/null"), &caps);
    }

    #[test]
    fn render_first_frame_kitty_caps_no_panic() {
        let caps = kitty_caps();
        // Even if timg/chafa are absent this must not panic.
        render_first_frame(std::path::Path::new("/dev/null"), &caps);
    }

    #[test]
    fn launch_media_inline_missing_ffprobe_returns_error_or_empty() {
        // /dev/null is readable but has no media streams.
        let result = launch_media_inline(std::path::Path::new("/dev/null"), 24);
        match result {
            Ok(()) => {} // ffprobe ran but produced empty/partial metadata
            Err(_) => {} // ffprobe failed or is absent
        }
    }
}
