use crate::app::{App, DirEntry};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Paragraph},
    Frame,
};

/// Main draw routine. Splits the terminal into a content area and a 1-row
/// status bar, then delegates to the individual render functions.
pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(f.area());

    draw_listing(f, app, chunks[0]);
    draw_status(f, app, chunks[1]);
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
    let items: Vec<ListItem> = app
        .entries
        .iter()
        .map(|e| ListItem::new(format_entry(e)))
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
// Status bar
// ---------------------------------------------------------------------------

fn draw_status(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let path_str = app.cwd.to_string_lossy().into_owned();
    let mode_str = app.mode.to_string();

    // Left: current path.  Right: mode indicator.
    let inner_width = area.width as usize;
    let mode_len = mode_str.len();
    let path_display = if path_str.len() + mode_len + 1 > inner_width {
        // Truncate path from the left to make room.
        let keep = inner_width.saturating_sub(mode_len + 4); // 4 = "... " prefix
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
        // Directories suppress the size field.
        assert!(
            !text.contains("4.0K"),
            "directory must not display its size"
        );
    }
}
