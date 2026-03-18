use crate::config::VeniConfig;
use crate::error::{Result, VeniError};
use caesar_common::terminal::TerminalCaps;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::time::SystemTime;

/// Input mode for the modal editing model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Navigation and file operations.
    Normal,
    /// Text input (rename, search, command palette).
    Insert,
    /// Multi-file selection.
    Visual,
    /// Ex-style command input.
    Command,
}

impl Default for Mode {
    fn default() -> Self {
        Self::Normal
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mode::Normal => write!(f, "NORMAL"),
            Mode::Insert => write!(f, "INSERT"),
            Mode::Visual => write!(f, "VISUAL"),
            Mode::Command => write!(f, "COMMAND"),
        }
    }
}

/// A single entry in the directory listing.
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

/// Core application state.
pub struct App {
    pub mode: Mode,
    pub cwd: PathBuf,
    pub caps: TerminalCaps,
    pub config: VeniConfig,
    pub should_quit: bool,
    pub entries: Vec<DirEntry>,
    pub selected: usize,
    pub scroll_offset: usize,
    /// Tracks whether a pending `g` was pressed (for `gg` binding).
    pending_g: bool,
}

impl App {
    pub fn new(path: PathBuf, caps: TerminalCaps, config: VeniConfig) -> Self {
        Self {
            mode: Mode::Normal,
            cwd: path,
            caps,
            config,
            should_quit: false,
            entries: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            pending_g: false,
        }
    }

    /// Read the current working directory and populate `entries`.
    ///
    /// Sort order: directories first, then files; alphabetical within each
    /// group (case-insensitive).  Unreadable entries are silently skipped.
    /// Dotfiles are included only when `config.show_hidden` is true.
    pub fn load_dir(&mut self) -> Result<()> {
        let read_dir = std::fs::read_dir(&self.cwd).map_err(|source| VeniError::ReadDir {
            path: self.cwd.clone(),
            source,
        })?;

        let mut entries: Vec<DirEntry> = Vec::new();
        for entry_result in read_dir {
            let entry = match entry_result {
                Ok(e) => e,
                Err(_) => continue, // skip unreadable entries gracefully
            };

            let name = entry.file_name().to_string_lossy().into_owned();

            // Respect show_hidden setting.
            if !self.config.show_hidden && name.starts_with('.') {
                continue;
            }

            let meta = entry.metadata().ok();
            let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let modified = meta.and_then(|m| m.modified().ok());

            entries.push(DirEntry {
                name,
                path: entry.path(),
                is_dir,
                size,
                modified,
            });
        }

        // Sort: directories first, then files; alphabetical within each group.
        entries.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        self.entries = entries;
        // Reset cursor when changing directory.
        self.selected = 0;
        self.scroll_offset = 0;
        Ok(())
    }

    /// Dispatch a key event to the active mode handler.
    pub fn handle_key(&mut self, key: KeyEvent) {
        // Ctrl-c always quits.
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            self.pending_g = false;
            return;
        }
        match self.mode {
            Mode::Normal => self.handle_key_normal(key),
            Mode::Insert | Mode::Visual | Mode::Command => {
                // Escape returns to Normal from any other mode.
                if key.code == KeyCode::Esc {
                    self.mode = Mode::Normal;
                }
                self.pending_g = false;
            }
        }
    }

    fn handle_key_normal(&mut self, key: KeyEvent) {
        // Handle pending `g` — second `g` completes `gg`.
        if self.pending_g {
            self.pending_g = false;
            if key.code == KeyCode::Char('g') {
                self.go_top();
                return;
            }
            // Any other key cancels the pending `g`; fall through to process it.
        }

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Char('l') | KeyCode::Enter | KeyCode::Right => self.enter_dir(),
            KeyCode::Char('h') | KeyCode::Backspace | KeyCode::Left => self.go_parent(),
            KeyCode::Char('g') => self.pending_g = true,
            KeyCode::Char('G') => self.go_bottom(),
            _ => {}
        }
    }

    // ------------------------------------------------------------------
    // Navigation primitives
    // ------------------------------------------------------------------

    fn move_down(&mut self) {
        if !self.entries.is_empty() && self.selected < self.entries.len() - 1 {
            self.selected += 1;
        }
    }

    fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    fn go_top(&mut self) {
        self.selected = 0;
        self.scroll_offset = 0;
    }

    fn go_bottom(&mut self) {
        if !self.entries.is_empty() {
            self.selected = self.entries.len() - 1;
        }
    }

    fn enter_dir(&mut self) {
        if let Some(entry) = self.entries.get(self.selected) {
            if entry.is_dir {
                let new_path = entry.path.clone();
                self.cwd = new_path;
                let _ = self.load_dir();
            }
        }
    }

    fn go_parent(&mut self) {
        if let Some(parent) = self.cwd.parent().map(|p| p.to_path_buf()) {
            self.cwd = parent;
            let _ = self.load_dir();
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_app(dir: &TempDir) -> App {
        App::new(
            dir.path().to_path_buf(),
            TerminalCaps::default(),
            VeniConfig::default(),
        )
    }

    // ------------------------------------------------------------------
    // Mode tests
    // ------------------------------------------------------------------

    #[test]
    fn default_mode_is_normal() {
        assert_eq!(Mode::default(), Mode::Normal);
    }

    #[test]
    fn mode_display() {
        assert_eq!(Mode::Normal.to_string(), "NORMAL");
        assert_eq!(Mode::Insert.to_string(), "INSERT");
        assert_eq!(Mode::Visual.to_string(), "VISUAL");
        assert_eq!(Mode::Command.to_string(), "COMMAND");
    }

    #[test]
    fn app_starts_in_normal_mode() {
        let tmp = TempDir::new().unwrap();
        let app = make_app(&tmp);
        assert_eq!(app.mode, Mode::Normal);
        assert!(!app.should_quit);
        assert!(app.entries.is_empty());
    }

    // ------------------------------------------------------------------
    // load_dir tests
    // ------------------------------------------------------------------

    #[test]
    fn load_dir_lists_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("file.txt"), b"hello").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        assert_eq!(app.entries.len(), 1);
        assert_eq!(app.entries[0].name, "file.txt");
    }

    #[test]
    fn load_dir_sorts_dirs_before_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("aaa.txt"), b"").unwrap();
        fs::create_dir(tmp.path().join("bbb_dir")).unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        assert!(app.entries[0].is_dir, "directory must come first");
        assert!(!app.entries[1].is_dir);
    }

    #[test]
    fn load_dir_sorts_alphabetically_within_group() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("zebra.txt"), b"").unwrap();
        fs::write(tmp.path().join("apple.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        assert_eq!(app.entries[0].name, "apple.txt");
        assert_eq!(app.entries[1].name, "zebra.txt");
    }

    #[test]
    fn load_dir_hides_dotfiles_by_default() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".hidden"), b"").unwrap();
        fs::write(tmp.path().join("visible.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        assert_eq!(app.entries.len(), 1);
        assert_eq!(app.entries[0].name, "visible.txt");
    }

    #[test]
    fn load_dir_shows_dotfiles_when_config_enabled() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".hidden"), b"").unwrap();
        fs::write(tmp.path().join("visible.txt"), b"").unwrap();
        let mut cfg = VeniConfig::default();
        cfg.show_hidden = true;
        let mut app = App::new(tmp.path().to_path_buf(), TerminalCaps::default(), cfg);
        app.load_dir().unwrap();
        assert_eq!(app.entries.len(), 2);
    }

    #[test]
    fn load_dir_resets_cursor() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), b"").unwrap();
        fs::write(tmp.path().join("b.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        app.selected = 1;
        app.load_dir().unwrap();
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn load_dir_empty_directory() {
        let tmp = TempDir::new().unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        assert!(app.entries.is_empty());
    }

    // ------------------------------------------------------------------
    // handle_key / navigation tests
    // ------------------------------------------------------------------

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    #[test]
    fn q_sets_should_quit() {
        let tmp = TempDir::new().unwrap();
        let mut app = make_app(&tmp);
        app.handle_key(key(KeyCode::Char('q')));
        assert!(app.should_quit);
    }

    #[test]
    fn ctrl_c_quits() {
        let tmp = TempDir::new().unwrap();
        let mut app = make_app(&tmp);
        app.handle_key(ctrl_key(KeyCode::Char('c')));
        assert!(app.should_quit);
    }

    #[test]
    fn j_moves_down() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), b"").unwrap();
        fs::write(tmp.path().join("b.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn k_moves_up() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), b"").unwrap();
        fs::write(tmp.path().join("b.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        app.selected = 1;
        app.handle_key(key(KeyCode::Char('k')));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn j_at_bottom_does_not_overflow() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.selected, 0); // only one entry; stays at 0
    }

    #[test]
    fn k_at_top_does_not_underflow() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        app.handle_key(key(KeyCode::Char('k')));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn capital_g_goes_to_bottom() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), b"").unwrap();
        fs::write(tmp.path().join("b.txt"), b"").unwrap();
        fs::write(tmp.path().join("c.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        app.handle_key(key(KeyCode::Char('G')));
        assert_eq!(app.selected, 2);
    }

    #[test]
    fn gg_goes_to_top() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), b"").unwrap();
        fs::write(tmp.path().join("b.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        app.selected = 1;
        app.handle_key(key(KeyCode::Char('g')));
        app.handle_key(key(KeyCode::Char('g')));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn single_g_does_not_move() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), b"").unwrap();
        fs::write(tmp.path().join("b.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        app.selected = 1;
        app.handle_key(key(KeyCode::Char('g')));
        // Only one `g` pressed — cursor must not change yet.
        assert_eq!(app.selected, 1);
        assert!(app.pending_g);
    }

    #[test]
    fn l_enters_subdirectory() {
        let tmp = TempDir::new().unwrap();
        let subdir = tmp.path().join("subdir");
        fs::create_dir(&subdir).unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        assert_eq!(app.entries[0].name, "subdir");
        let expected = app.entries[0].path.clone();
        app.handle_key(key(KeyCode::Char('l')));
        assert_eq!(app.cwd, expected);
    }

    #[test]
    fn h_goes_to_parent() {
        let tmp = TempDir::new().unwrap();
        let subdir = tmp.path().join("sub");
        fs::create_dir(&subdir).unwrap();
        let mut app = App::new(
            subdir.clone(),
            TerminalCaps::default(),
            VeniConfig::default(),
        );
        app.load_dir().unwrap();
        let parent = tmp.path().to_path_buf();
        app.handle_key(key(KeyCode::Char('h')));
        // After going up, cwd should be the parent (canonicalized form).
        assert_eq!(
            app.cwd.canonicalize().unwrap_or(app.cwd.clone()),
            parent.canonicalize().unwrap_or(parent)
        );
    }

    #[test]
    fn arrow_keys_work_like_hjkl() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), b"").unwrap();
        fs::write(tmp.path().join("b.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.selected, 1);
        app.handle_key(key(KeyCode::Up));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn escape_returns_to_normal_from_insert() {
        let tmp = TempDir::new().unwrap();
        let mut app = make_app(&tmp);
        app.mode = Mode::Insert;
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn g_then_non_g_cancels_pending() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), b"").unwrap();
        fs::write(tmp.path().join("b.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        app.selected = 1;
        app.handle_key(key(KeyCode::Char('g')));
        assert!(app.pending_g);
        // Press 'j' after 'g' — pending_g clears, 'j' also fires (moves down)
        // but we're already at the end so selected stays 1.
        app.handle_key(key(KeyCode::Char('j')));
        assert!(!app.pending_g);
    }
}
