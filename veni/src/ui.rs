use crate::app::{App, DirEntry, Mode};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Paragraph},
    Frame,
};

/// Main draw routine.  Splits the terminal into three rows:
///   1. Breadcrumb header (1 line)
///   2. Directory listing (fills remaining space)
///   3. Status bar / command line (1 line)
pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(f.area());

    draw_breadcrumb(f, app, chunks[0]);
    draw_listing(f, app, chunks[1]);
    draw_status(f, app, chunks[2]);
}

// ---------------------------------------------------------------------------
// Breadcrumb header
// ---------------------------------------------------------------------------

fn draw_breadcrumb(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let crumb = build_breadcrumb(&app.cwd, area.width as usize);
    let para = Paragraph::new(crumb).style(Style::default().fg(Color::Cyan));
    f.render_widget(para, area);
}

/// Build a breadcrumb string for the given path, truncating from the left
/// with `...` if the full string would exceed `max_width` columns.
fn build_breadcrumb(path: &std::path::Path, max_width: usize) -> String {
    // Collect path components; skip the root separator itself.
    let parts: Vec<String> = path
        .components()
        .filter_map(|c| {
            use std::path::Component;
            match c {
                Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
                Component::RootDir => Some(String::new()), // represents "/"
                _ => None,
            }
        })
        .collect();

    // Join with " > ", treating the leading empty string as the root "/".
    let full = if parts.first().map(|s| s.is_empty()).unwrap_or(false) {
        let rest = &parts[1..];
        if rest.is_empty() {
            "/".to_string()
        } else {
            format!("/ > {}", rest.join(" > "))
        }
    } else {
        parts.join(" > ")
    };

    if full.len() <= max_width {
        full
    } else {
        // Truncate from the left.
        let keep = max_width.saturating_sub(3); // room for "..."
        let start = full.len().saturating_sub(keep);
        // Advance to next '>' boundary so we don't cut mid-component.
        let trimmed = &full[start..];
        format!("...{}", trimmed)
    }
}

// ---------------------------------------------------------------------------
// Directory listing
// ---------------------------------------------------------------------------

fn format_entry(entry: &DirEntry) -> Line<'static> {
    let name = if entry.is_dir {
        format!("{}/", entry.name)
    } else {
        entry.name.clone()
    };

    let size_str = if entry.is_dir {
        String::new()
    } else {
        format_size(entry.size)
    };

    let style = if entry.is_dir {
        Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    if size_str.is_empty() {
        Line::from(Span::styled(name, style))
    } else {
        Line::from(vec![
            Span::styled(format!("{:<40}", name), style),
            Span::raw(size_str),
        ])
    }
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1_024;
    const MB: u64 = 1_024 * KB;
    const GB: u64 = 1_024 * MB;
    if bytes >= GB {
        format!("{:.1}G", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}M", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}K", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

fn draw_listing(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    // Determine which indices are "selected" in visual / explicit selection.
    let visual_range = if app.mode == Mode::Visual {
        let r = app.visual_range();
        Some(r)
    } else {
        None
    };

    let items: Vec<ListItem> = app
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let line = format_entry(e);
            let in_visual = visual_range
                .as_ref()
                .map(|r| r.contains(&i))
                .unwrap_or(false);
            let in_selection = app.selection.contains(&i);
            let is_search_match = app.search_matches.contains(&i);

            if in_visual || in_selection {
                ListItem::new(line).style(Style::default().bg(Color::DarkGray).fg(Color::Yellow))
            } else if is_search_match {
                ListItem::new(line).style(Style::default().bg(Color::DarkGray).fg(Color::Green))
            } else {
                ListItem::new(line)
            }
        })
        .collect();

    let list = List::new(items)
        .block(Block::default())
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    let mut state = ListState::default();
    state.select(Some(app.selected));
    f.render_stateful_widget(list, area, &mut state);
}

// ---------------------------------------------------------------------------
// Status bar / command line
// ---------------------------------------------------------------------------

fn draw_status(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    match app.mode {
        Mode::Command => {
            // Show `:` prompt followed by the command buffer.
            let prompt = format!(":{}", app.command_input);
            let para = Paragraph::new(prompt).style(
                Style::default()
                    .fg(Color::White)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );
            f.render_widget(para, area);
        }
        Mode::Search => {
            // Show `/` prompt followed by the search query.
            let prompt = format!("/{}", app.search_query);
            let para = Paragraph::new(prompt).style(
                Style::default()
                    .fg(Color::White)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );
            f.render_widget(para, area);
        }
        _ => draw_normal_status(f, app, area),
    }
}

fn draw_normal_status(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let path_str = app.cwd.to_string_lossy().into_owned();
    let mode_str = app.mode.to_string();

    let inner_width = area.width as usize;
    let mode_len = mode_str.len();
    let path_display = if path_str.len() + mode_len + 1 > inner_width {
        let keep = inner_width.saturating_sub(mode_len + 4);
        let start = path_str.len().saturating_sub(keep);
        format!("...{}", &path_str[start..])
    } else {
        path_str.clone()
    };

    let padding = inner_width.saturating_sub(path_display.len() + mode_len);
    let status_line = format!("{}{}{}", path_display, " ".repeat(padding), mode_str);

    let status = Paragraph::new(status_line).style(
        Style::default()
            .fg(Color::Black)
            .bg(Color::Blue)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(status, area);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_size_bytes() {
        assert_eq!(format_size(0), "0B");
        assert_eq!(format_size(512), "512B");
    }

    #[test]
    fn format_size_kilobytes() {
        assert_eq!(format_size(1_024), "1.0K");
        assert_eq!(format_size(2_048), "2.0K");
    }

    #[test]
    fn format_size_megabytes() {
        assert_eq!(format_size(1_048_576), "1.0M");
    }

    #[test]
    fn format_size_gigabytes() {
        assert_eq!(format_size(1_073_741_824), "1.0G");
    }

    #[test]
    fn format_entry_dir_appends_slash() {
        let entry = DirEntry {
            name: "docs".to_string(),
            path: "/tmp/docs".into(),
            is_dir: true,
            size: 0,
            modified: None,
        };
        let line = format_entry(&entry);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("docs/"), "directory entry must end with /");
    }

    #[test]
    fn format_entry_file_no_slash() {
        let entry = DirEntry {
            name: "readme.txt".to_string(),
            path: "/tmp/readme.txt".into(),
            is_dir: false,
            size: 1_024,
            modified: None,
        };
        let line = format_entry(&entry);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            !text.contains("readme.txt/"),
            "file entry must not end with /"
        );
        assert!(text.contains("1.0K"), "file entry must show size");
    }

    #[test]
    fn format_entry_dir_has_no_size() {
        let entry = DirEntry {
            name: "bin".to_string(),
            path: "/usr/bin".into(),
            is_dir: true,
            size: 4096,
            modified: None,
        };
        let line = format_entry(&entry);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            !text.contains("4.0K"),
            "directory must not display its size"
        );
    }

    // ------------------------------------------------------------------
    // Breadcrumb tests
    // ------------------------------------------------------------------

    #[test]
    fn breadcrumb_root() {
        let path = std::path::Path::new("/");
        assert_eq!(build_breadcrumb(path, 80), "/");
    }

    #[test]
    fn breadcrumb_simple_path() {
        let path = std::path::Path::new("/Users/chris");
        assert_eq!(build_breadcrumb(path, 80), "/ > Users > chris");
    }

    #[test]
    fn breadcrumb_deeper_path() {
        let path = std::path::Path::new("/Users/chris/dev/tools");
        assert_eq!(
            build_breadcrumb(path, 80),
            "/ > Users > chris > dev > tools"
        );
    }

    #[test]
    fn breadcrumb_truncates_from_left_when_too_long() {
        let path = std::path::Path::new("/Users/chris/dev/tools/caesar");
        // Width of 20 forces truncation.
        let crumb = build_breadcrumb(path, 20);
        assert!(
            crumb.starts_with("..."),
            "truncated breadcrumb must start with ..."
        );
        assert!(crumb.len() <= 20 + 3, "should not greatly exceed max_width");
    }

    #[test]
    fn breadcrumb_fits_exactly_is_not_truncated() {
        let path = std::path::Path::new("/a/b");
        let full = build_breadcrumb(path, 80);
        let width = full.len();
        // Re-build with exactly that width — should be the same string.
        assert_eq!(build_breadcrumb(path, width), full);
    }
}
