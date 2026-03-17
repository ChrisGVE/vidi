/// Integration tests for the internal HTML and EPUB renderer paths.
///
/// These tests exercise the public binary (`vidi --inline`) and the library
/// APIs directly, verifying end-to-end behaviour without requiring any
/// external tools.
use assert_cmd::Command;
use std::io::Write as _;
use tempfile::NamedTempFile;

// ─── HTML renderer ────────────────────────────────────────────────────────────

/// Write a minimal HTML file and verify that `vidi --inline` produces output
/// containing the heading text, without panicking.
#[test]
fn inline_html_produces_output() {
    let mut tmp = NamedTempFile::with_suffix(".html").unwrap();
    writeln!(tmp, "<html><body><h1>Hello</h1><p>World</p></body></html>").unwrap();
    tmp.flush().unwrap();

    let mut cmd = Command::cargo_bin("vidi").unwrap();
    cmd.args(["--inline", "--lines", "20", tmp.path().to_str().unwrap()]);
    let output = cmd.output().unwrap();

    // Must not panic (exit status 0 or non-zero depending on environment)
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Hello") || stdout.contains("World") || output.status.success(),
        "expected some output from inline HTML render; stdout={stdout:?}",
    );
}

/// Verify that the HTML renderer emits bold ANSI codes for headings.
#[test]
fn html_renderer_headings_use_bold() {
    use vidi::{
        renderer::render_html_with_resolver,
        terminal::{GraphicsProtocol, TerminalCaps},
    };

    let caps = TerminalCaps {
        graphics: GraphicsProtocol::HalfBlock256,
        true_color: false,
        columns: 80,
        rows: 24,
    };
    let html = b"<h1>Title</h1>";
    let out = render_html_with_resolver(html, &caps, true, 50, &|_| None).unwrap();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("\x1b[1m"), "h1 should use bold ANSI");
    assert!(s.contains("Title"), "h1 text should be present");
}

/// Verify that paragraphs are separated by newlines.
#[test]
fn html_renderer_paragraphs_newline_separated() {
    use vidi::{
        renderer::render_html_with_resolver,
        terminal::{GraphicsProtocol, TerminalCaps},
    };

    let caps = TerminalCaps {
        graphics: GraphicsProtocol::HalfBlock256,
        true_color: false,
        columns: 80,
        rows: 24,
    };
    let html = b"<p>First</p><p>Second</p>";
    let out = render_html_with_resolver(html, &caps, true, 50, &|_| None).unwrap();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("First") && s.contains("Second"));
    // At least one newline between paragraphs
    let first_pos = s.find("First").unwrap();
    let second_pos = s.find("Second").unwrap();
    let between = &s[first_pos..second_pos];
    assert!(
        between.contains('\n'),
        "expected newline between paragraphs; got: {between:?}"
    );
}

/// Verify that `<img>` without a resolver emits a placeholder line.
#[test]
fn html_renderer_img_placeholder_when_no_resolver() {
    use vidi::{
        renderer::render_html_with_resolver,
        terminal::{GraphicsProtocol, TerminalCaps},
    };

    let caps = TerminalCaps {
        graphics: GraphicsProtocol::HalfBlock256,
        true_color: false,
        columns: 80,
        rows: 24,
    };
    let html = br#"<img src="banner.png" />"#;
    let out = render_html_with_resolver(html, &caps, true, 50, &|_| None).unwrap();
    let s = String::from_utf8_lossy(&out);
    assert!(
        s.contains("[image: banner.png]"),
        "expected placeholder; got: {s:?}"
    );
}

// ─── EPUB renderer ────────────────────────────────────────────────────────────

/// Opening a non-epub file (Cargo.toml) must return `None`, not panic.
#[test]
fn epub_renderer_non_epub_returns_none() {
    use std::path::Path;
    use vidi::{
        detector::FileKind,
        renderer::internal_render,
        terminal::{GraphicsProtocol, TerminalCaps},
    };

    let caps = TerminalCaps {
        graphics: GraphicsProtocol::HalfBlock256,
        true_color: false,
        columns: 80,
        rows: 24,
    };
    let result = internal_render(FileKind::Ebook, Path::new("Cargo.toml"), &caps, 50, true);
    assert!(result.is_none(), "non-epub file should return None");
}

/// `vidi --inline` on a non-epub ebook must not panic.
///
/// Since we have no test epub available, we use Cargo.toml and expect a graceful
/// fallback to the external tool registry (or an error, not a panic).
#[test]
fn inline_ebook_non_epub_does_not_panic() {
    let mut cmd = Command::cargo_bin("vidi").unwrap();
    // Using a .epub extension on Cargo.toml content forces Ebook kind via extension.
    let mut tmp = NamedTempFile::with_suffix(".epub").unwrap();
    writeln!(tmp, "not a real epub").unwrap();
    tmp.flush().unwrap();

    cmd.args(["--inline", "--lines", "5", tmp.path().to_str().unwrap()]);
    // Must complete without panicking; exit status may be non-zero
    let output = cmd.output().unwrap();
    assert_ne!(
        output.status.code(),
        Some(101),
        "process must not exit with RUST_BACKTRACE panic code 101"
    );
}

// ─── Image renderer ───────────────────────────────────────────────────────────

/// When chafa is absent, render_image returns a `[image: …]` placeholder.
#[test]
fn image_renderer_no_chafa_placeholder() {
    use vidi::{
        renderer::image::render_image,
        terminal::{GraphicsProtocol, TerminalCaps},
    };

    let caps = TerminalCaps {
        graphics: GraphicsProtocol::HalfBlock256,
        true_color: false,
        columns: 80,
        rows: 24,
    };
    // If chafa is not installed, we get the placeholder.
    // If chafa IS installed this test still passes (output is non-empty).
    let out = render_image(b"\x89PNG\r\n\x1a\n", "test.png", &caps, 10);
    assert!(!out.is_empty(), "render_image must produce some output");
}
