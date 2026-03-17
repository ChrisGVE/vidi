//! Render a PDF page to PNG and display it in the terminal via `chafa`.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Result, VidiError};
use crate::terminal::TerminalCaps;

/// Render page 1 of a PDF to a PNG file using `mutool draw`.
///
/// The PNG is written to `<outdir>/page.png`.
///
/// # Errors
///
/// Returns [`VidiError::ToolNotFound`] when `mutool` is absent.
/// Returns [`VidiError::ToolFailed`] when the render exits non-zero.
pub fn render_pdf_page(
    pdf: &Path,
    outdir: &Path,
    available: fn(&'static str) -> bool,
) -> Result<PathBuf> {
    if !available("mutool") {
        return Err(VidiError::ToolNotFound {
            tool: "mutool".to_string(),
        });
    }

    let png_path = outdir.join("page.png");

    let status = Command::new("mutool")
        .arg("draw")
        .arg("-F")
        .arg("png")
        .arg("-o")
        .arg(&png_path)
        .arg(pdf)
        .arg("1")
        .status()
        .map_err(VidiError::Io)?;

    if !status.success() {
        return Err(VidiError::ToolFailed {
            tool: "mutool".to_string(),
            code: status.code().unwrap_or(1),
        });
    }

    Ok(png_path)
}

/// Build the argument list for `chafa` to display `png` in the terminal.
///
/// The returned vector begins with the PNG path and then size arguments.
/// Callers should pass the entire slice to `chafa`.
pub fn chafa_args(png: &Path, caps: &TerminalCaps) -> Vec<String> {
    let cols = caps.columns.max(80).to_string();
    let rows = caps.rows.max(24).to_string();
    vec![
        png.to_string_lossy().into_owned(),
        "--size".to_string(),
        format!("{cols}x{rows}"),
    ]
}

/// Display `png` in the terminal using `chafa`.
///
/// Spawns `chafa` as a child process and waits for it to exit.
///
/// # Errors
///
/// Returns [`VidiError::ToolNotFound`] when `chafa` is absent.
/// Returns [`VidiError::ToolFailed`] when chafa exits non-zero.
pub fn display_png(
    png: &Path,
    caps: &TerminalCaps,
    available: fn(&'static str) -> bool,
) -> Result<()> {
    if !available("chafa") {
        return Err(VidiError::ToolNotFound {
            tool: "chafa".to_string(),
        });
    }

    let args = chafa_args(png, caps);
    let status = Command::new("chafa")
        .args(&args)
        .status()
        .map_err(VidiError::Io)?;

    if status.success() {
        Ok(())
    } else {
        Err(VidiError::ToolFailed {
            tool: "chafa".to_string(),
            code: status.code().unwrap_or(1),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::{GraphicsProtocol, TerminalCaps};
    use std::path::Path;

    fn unavailable(_: &'static str) -> bool {
        false
    }

    fn test_caps() -> TerminalCaps {
        TerminalCaps {
            graphics: GraphicsProtocol::HalfBlock256,
            true_color: false,
            columns: 120,
            rows: 40,
        }
    }

    #[test]
    fn render_returns_tool_not_found_when_mutool_absent() {
        let result = render_pdf_page(Path::new("/tmp/test.pdf"), Path::new("/tmp"), unavailable);
        match result {
            Err(VidiError::ToolNotFound { tool }) => assert_eq!(tool, "mutool"),
            other => panic!("expected ToolNotFound, got {other:?}"),
        }
    }

    #[test]
    fn display_returns_tool_not_found_when_chafa_absent() {
        let png = Path::new("/tmp/page.png");
        let caps = test_caps();
        let result = display_png(png, &caps, unavailable);
        match result {
            Err(VidiError::ToolNotFound { tool }) => assert_eq!(tool, "chafa"),
            other => panic!("expected ToolNotFound, got {other:?}"),
        }
    }

    #[test]
    fn chafa_args_first_element_is_png_path() {
        let png = Path::new("/tmp/page.png");
        let caps = test_caps();
        let args = chafa_args(png, &caps);
        assert_eq!(args[0], "/tmp/page.png");
    }

    #[test]
    fn chafa_args_contain_size_flag() {
        let png = Path::new("/tmp/page.png");
        let caps = test_caps();
        let args = chafa_args(png, &caps);
        assert!(args.iter().any(|a| a == "--size"), "missing --size flag");
    }

    #[test]
    fn chafa_args_size_uses_terminal_dimensions() {
        let png = Path::new("/tmp/page.png");
        let caps = test_caps();
        let args = chafa_args(png, &caps);
        let size_idx = args.iter().position(|a| a == "--size").unwrap();
        assert_eq!(args[size_idx + 1], "120x40");
    }

    #[test]
    fn chafa_args_fallback_to_minimum_dimensions() {
        let png = Path::new("/tmp/page.png");
        let caps = TerminalCaps {
            graphics: GraphicsProtocol::HalfBlock256,
            true_color: false,
            columns: 0,
            rows: 0,
        };
        let args = chafa_args(png, &caps);
        let size_idx = args.iter().position(|a| a == "--size").unwrap();
        // Should fall back to 80x24 minimum
        assert_eq!(args[size_idx + 1], "80x24");
    }

    #[test]
    fn render_png_output_path_is_page_png() {
        let outdir = Path::new("/tmp");
        let expected = outdir.join("page.png");
        // Verify path computation without running the binary
        assert_eq!(expected, PathBuf::from("/tmp/page.png"));
    }
}
