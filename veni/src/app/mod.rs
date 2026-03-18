use crate::config::VeniConfig;
use crate::error::{Result, VeniError};
use crate::input::{resolve, KeyAction};
use caesar_common::terminal::TerminalCaps;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashSet;
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
    /// Incremental filename search.
    Search,
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
            Mode::Search => write!(f, "SEARCH"),
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
    /// Pending first key for multi-key sequences (e.g. `gg`, `dd`).
    pub pending_key: Option<char>,
    /// Index where Visual mode selection started.
    pub visual_anchor: Option<usize>,
    /// Explicitly toggled entries (V-mode line selections).
    pub selection: HashSet<usize>,
    /// Buffer for Command mode input (`:` commands).
    pub command_input: String,
    /// Buffer for Search mode input (`/` search).
    pub search_query: String,
    /// Indices into `entries` that match the current search query.
    pub search_matches: Vec<usize>,
    /// Position within `search_matches` currently highlighted.
    pub search_match_idx: usize,
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
            pending_key: None,
            visual_anchor: None,
            selection: HashSet::new(),
            command_input: String::new(),
            search_query: String::new(),
            search_matches: Vec::new(),
            search_match_idx: 0,
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
            self.pending_key = None;
            return;
        }
        match self.mode {
            Mode::Normal => self.handle_key_normal(key),
            Mode::Visual => self.handle_key_visual(key),
            Mode::Command => self.handle_key_command(key),
            Mode::Search => self.handle_key_search(key),
            Mode::Insert => {
                if key.code == KeyCode::Esc {
                    self.mode = Mode::Normal;
                }
                self.pending_key = None;
            }
        }
    }

    // ------------------------------------------------------------------
    // Normal mode
    // ------------------------------------------------------------------

    fn handle_key_normal(&mut self, key: KeyEvent) {
        // Arrow keys handled directly without going through char resolver.
        match key.code {
            KeyCode::Down => {
                self.pending_key = None;
                self.move_down();
                return;
            }
            KeyCode::Up => {
                self.pending_key = None;
                self.move_up();
                return;
            }
            KeyCode::Right | KeyCode::Enter => {
                self.pending_key = None;
                self.enter_dir();
                return;
            }
            KeyCode::Left | KeyCode::Backspace => {
                self.pending_key = None;
                self.go_parent();
                return;
            }
            _ => {}
        }

        if let KeyCode::Char(ch) = key.code {
            if let Some(action) = resolve(ch, &mut self.pending_key) {
                self.execute_action(action);
            }
        }
    }

    fn execute_action(&mut self, action: KeyAction) {
        match action {
            KeyAction::MoveDown => self.move_down(),
            KeyAction::MoveUp => self.move_up(),
            KeyAction::EnterDir => self.enter_dir(),
            KeyAction::ParentDir => self.go_parent(),
            KeyAction::GoTop => self.go_top(),
            KeyAction::GoBottom => self.go_bottom(),
            KeyAction::Quit => self.should_quit = true,
            KeyAction::EnterVisual => {
                self.visual_anchor = Some(self.selected);
                self.mode = Mode::Visual;
            }
            KeyAction::ToggleVisualLine => {
                if self.selection.contains(&self.selected) {
                    self.selection.remove(&self.selected);
                } else {
                    self.selection.insert(self.selected);
                }
            }
            KeyAction::EnterCommand => {
                self.command_input.clear();
                self.mode = Mode::Command;
            }
            KeyAction::SearchForward => {
                self.search_query.clear();
                self.search_matches.clear();
                self.search_match_idx = 0;
                self.mode = Mode::Search;
            }
            KeyAction::SearchNext => self.search_next(),
            KeyAction::SearchPrev => self.search_prev(),
            // Yank, Delete, Paste, Undo, Rename: stubs for future tasks.
            KeyAction::Yank
            | KeyAction::Delete
            | KeyAction::Paste
            | KeyAction::Undo
            | KeyAction::Rename
            | KeyAction::ToggleHidden => {}
        }
    }

    // ------------------------------------------------------------------
    // Visual mode
    // ------------------------------------------------------------------

    fn handle_key_visual(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.visual_anchor = None;
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Char('V') => {
                // Toggle current entry in explicit selection set and exit visual.
                if self.selection.contains(&self.selected) {
                    self.selection.remove(&self.selected);
                } else {
                    self.selection.insert(self.selected);
                }
                self.mode = Mode::Normal;
                self.visual_anchor = None;
            }
            _ => {}
        }
    }

    /// Returns the range of indices covered by the current Visual selection.
    /// Returns an empty range when not in Visual mode or no anchor is set.
    pub fn visual_range(&self) -> std::ops::RangeInclusive<usize> {
        match self.visual_anchor {
            Some(anchor) => {
                let lo = anchor.min(self.selected);
                let hi = anchor.max(self.selected);
                lo..=hi
            }
            None => 0..=0, // degenerate; callers should check mode
        }
    }

    // ------------------------------------------------------------------
    // Command mode
    // ------------------------------------------------------------------

    fn handle_key_command(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.command_input.clear();
                self.mode = Mode::Normal;
            }
            KeyCode::Enter => {
                let cmd = self.command_input.trim().to_string();
                self.command_input.clear();
                self.mode = Mode::Normal;
                self.execute_command(&cmd);
            }
            KeyCode::Backspace => {
                self.command_input.pop();
            }
            KeyCode::Char(ch) => {
                self.command_input.push(ch);
            }
            _ => {}
        }
    }

    fn execute_command(&mut self, cmd: &str) {
        match cmd {
            "q" => self.should_quit = true,
            "set hidden" => {
                self.config.show_hidden = true;
                let _ = self.load_dir();
            }
            "set nohidden" => {
                self.config.show_hidden = false;
                let _ = self.load_dir();
            }
            other if other.starts_with("cd ") => {
                let path_str = other.trim_start_matches("cd ").trim();
                let new_path = if path_str.starts_with('/') {
                    PathBuf::from(path_str)
                } else {
                    self.cwd.join(path_str)
                };
                if new_path.is_dir() {
                    self.cwd = new_path;
                    let _ = self.load_dir();
                }
            }
            _ => {} // unknown command — silently ignore
        }
    }

    // ------------------------------------------------------------------
    // Search mode
    // ------------------------------------------------------------------

    fn handle_key_search(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.search_query.clear();
                self.search_matches.clear();
                self.search_match_idx = 0;
                self.mode = Mode::Normal;
            }
            KeyCode::Enter => {
                // Confirm search: move to first match if any, return to Normal.
                if !self.search_matches.is_empty() {
                    self.selected = self.search_matches[0];
                    self.search_match_idx = 0;
                }
                self.mode = Mode::Normal;
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.update_search_matches();
            }
            KeyCode::Char(ch) => {
                self.search_query.push(ch);
                self.update_search_matches();
                // Jump cursor to first match immediately.
                if !self.search_matches.is_empty() {
                    self.selected = self.search_matches[0];
                    self.search_match_idx = 0;
                }
            }
            _ => {}
        }
    }

    fn update_search_matches(&mut self) {
        if self.search_query.is_empty() {
            self.search_matches.clear();
            self.search_match_idx = 0;
            return;
        }
        let query = self.search_query.to_lowercase();
        self.search_matches = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.name.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();
        self.search_match_idx = 0;
    }

    fn search_next(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        self.search_match_idx = (self.search_match_idx + 1) % self.search_matches.len();
        self.selected = self.search_matches[self.search_match_idx];
    }

    fn search_prev(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        if self.search_match_idx == 0 {
            self.search_match_idx = self.search_matches.len() - 1;
        } else {
            self.search_match_idx -= 1;
        }
        self.selected = self.search_matches[self.search_match_idx];
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
        assert_eq!(Mode::Search.to_string(), "SEARCH");
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
        assert_eq!(app.pending_key, Some('g'));
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
        assert_eq!(app.pending_key, Some('g'));
        // Press 'j' after 'g' — pending_g clears, 'j' fires (MoveDown).
        // We're at position 1 (last), so moves nowhere but pending clears.
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.pending_key, None);
    }

    // ------------------------------------------------------------------
    // Visual mode tests
    // ------------------------------------------------------------------

    #[test]
    fn v_enters_visual_mode() {
        let tmp = TempDir::new().unwrap();
        let mut app = make_app(&tmp);
        app.handle_key(key(KeyCode::Char('v')));
        assert_eq!(app.mode, Mode::Visual);
        assert_eq!(app.visual_anchor, Some(0));
    }

    #[test]
    fn esc_exits_visual_mode() {
        let tmp = TempDir::new().unwrap();
        let mut app = make_app(&tmp);
        app.mode = Mode::Visual;
        app.visual_anchor = Some(0);
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.visual_anchor, None);
    }

    #[test]
    fn visual_j_extends_selection_down() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), b"").unwrap();
        fs::write(tmp.path().join("b.txt"), b"").unwrap();
        fs::write(tmp.path().join("c.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        app.mode = Mode::Visual;
        app.visual_anchor = Some(0);
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.selected, 1);
        let range = app.visual_range();
        assert_eq!(*range.start(), 0);
        assert_eq!(*range.end(), 1);
    }

    #[test]
    fn visual_range_upward() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), b"").unwrap();
        fs::write(tmp.path().join("b.txt"), b"").unwrap();
        fs::write(tmp.path().join("c.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        app.selected = 2;
        app.mode = Mode::Visual;
        app.visual_anchor = Some(2);
        app.handle_key(key(KeyCode::Char('k')));
        let range = app.visual_range();
        assert_eq!(*range.start(), 1);
        assert_eq!(*range.end(), 2);
    }

    #[test]
    fn capital_v_toggles_selection() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        // V in Normal mode toggles current entry.
        app.handle_key(key(KeyCode::Char('V')));
        assert!(app.selection.contains(&0));
        app.handle_key(key(KeyCode::Char('V')));
        assert!(!app.selection.contains(&0));
    }

    // ------------------------------------------------------------------
    // Command mode tests
    // ------------------------------------------------------------------

    #[test]
    fn colon_enters_command_mode() {
        let tmp = TempDir::new().unwrap();
        let mut app = make_app(&tmp);
        app.handle_key(key(KeyCode::Char(':')));
        assert_eq!(app.mode, Mode::Command);
        assert!(app.command_input.is_empty());
    }

    #[test]
    fn command_mode_types_chars() {
        let tmp = TempDir::new().unwrap();
        let mut app = make_app(&tmp);
        app.mode = Mode::Command;
        app.handle_key(key(KeyCode::Char('q')));
        assert_eq!(app.command_input, "q");
    }

    #[test]
    fn command_mode_backspace_deletes() {
        let tmp = TempDir::new().unwrap();
        let mut app = make_app(&tmp);
        app.mode = Mode::Command;
        app.command_input = "cd".to_string();
        app.handle_key(key(KeyCode::Backspace));
        assert_eq!(app.command_input, "c");
    }

    #[test]
    fn command_mode_esc_cancels() {
        let tmp = TempDir::new().unwrap();
        let mut app = make_app(&tmp);
        app.mode = Mode::Command;
        app.command_input = "q".to_string();
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.command_input.is_empty());
    }

    #[test]
    fn command_q_quits() {
        let tmp = TempDir::new().unwrap();
        let mut app = make_app(&tmp);
        app.mode = Mode::Command;
        app.command_input = "q".to_string();
        app.handle_key(key(KeyCode::Enter));
        assert!(app.should_quit);
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn command_set_hidden_shows_dotfiles() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".hidden"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        assert_eq!(app.entries.len(), 0);
        app.mode = Mode::Command;
        app.command_input = "set hidden".to_string();
        app.handle_key(key(KeyCode::Enter));
        assert!(app.config.show_hidden);
        assert_eq!(app.entries.len(), 1);
    }

    #[test]
    fn command_set_nohidden_hides_dotfiles() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".hidden"), b"").unwrap();
        fs::write(tmp.path().join("visible.txt"), b"").unwrap();
        let mut cfg = VeniConfig::default();
        cfg.show_hidden = true;
        let mut app = App::new(tmp.path().to_path_buf(), TerminalCaps::default(), cfg);
        app.load_dir().unwrap();
        assert_eq!(app.entries.len(), 2);
        app.mode = Mode::Command;
        app.command_input = "set nohidden".to_string();
        app.handle_key(key(KeyCode::Enter));
        assert!(!app.config.show_hidden);
        assert_eq!(app.entries.len(), 1);
    }

    #[test]
    fn command_cd_changes_directory() {
        let tmp = TempDir::new().unwrap();
        let subdir = tmp.path().join("sub");
        fs::create_dir(&subdir).unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        app.mode = Mode::Command;
        let cd_cmd = format!("cd {}", subdir.to_string_lossy());
        app.command_input = cd_cmd;
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.cwd, subdir);
    }

    #[test]
    fn command_unknown_is_ignored() {
        let tmp = TempDir::new().unwrap();
        let mut app = make_app(&tmp);
        app.mode = Mode::Command;
        app.command_input = "foobar".to_string();
        app.handle_key(key(KeyCode::Enter));
        assert!(!app.should_quit);
        assert_eq!(app.mode, Mode::Normal);
    }

    // ------------------------------------------------------------------
    // Search mode tests
    // ------------------------------------------------------------------

    #[test]
    fn slash_enters_search_mode() {
        let tmp = TempDir::new().unwrap();
        let mut app = make_app(&tmp);
        app.handle_key(key(KeyCode::Char('/')));
        assert_eq!(app.mode, Mode::Search);
        assert!(app.search_query.is_empty());
    }

    #[test]
    fn search_typing_filters_matches() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("alpha.txt"), b"").unwrap();
        fs::write(tmp.path().join("beta.txt"), b"").unwrap();
        fs::write(tmp.path().join("alphabet.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        app.mode = Mode::Search;
        app.handle_key(key(KeyCode::Char('a')));
        app.handle_key(key(KeyCode::Char('l')));
        // "al" matches alpha.txt (idx 0) and alphabet.txt (idx 1) — not beta.
        assert_eq!(app.search_matches.len(), 2);
        // Cursor should jump to first match.
        assert_eq!(app.selected, app.search_matches[0]);
    }

    #[test]
    fn search_enter_confirms_and_returns_normal() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("alpha.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        app.mode = Mode::Search;
        app.search_query = "alpha".to_string();
        app.update_search_matches();
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn search_esc_cancels() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("alpha.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        app.mode = Mode::Search;
        app.search_query = "al".to_string();
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.search_query.is_empty());
        assert!(app.search_matches.is_empty());
    }

    #[test]
    fn search_n_goes_to_next_match() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("alpha.txt"), b"").unwrap();
        fs::write(tmp.path().join("beta.txt"), b"").unwrap();
        fs::write(tmp.path().join("gamma_a.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        // Set up a search with matches at indices 0 and 2.
        app.search_query = "a".to_string();
        app.update_search_matches();
        // Confirm all three contain 'a'.
        assert_eq!(app.search_matches.len(), 3);
        app.selected = app.search_matches[0];
        app.search_match_idx = 0;
        // 'n' moves to next.
        app.handle_key(key(KeyCode::Char('n')));
        assert_eq!(app.selected, app.search_matches[1]);
    }

    #[test]
    fn search_capital_n_goes_to_prev_match() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("alpha.txt"), b"").unwrap();
        fs::write(tmp.path().join("beta.txt"), b"").unwrap();
        fs::write(tmp.path().join("gamma_a.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        app.search_query = "a".to_string();
        app.update_search_matches();
        app.selected = app.search_matches[1];
        app.search_match_idx = 1;
        app.handle_key(key(KeyCode::Char('N')));
        assert_eq!(app.selected, app.search_matches[0]);
    }

    #[test]
    fn search_backspace_removes_char_and_updates() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("alpha.txt"), b"").unwrap();
        fs::write(tmp.path().join("beta.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        app.mode = Mode::Search;
        app.handle_key(key(KeyCode::Char('a')));
        let matches_after_a = app.search_matches.len();
        app.handle_key(key(KeyCode::Backspace));
        assert!(app.search_query.is_empty());
        assert!(app.search_matches.is_empty());
        let _ = matches_after_a; // just ensuring count changed
    }

    #[test]
    fn search_case_insensitive() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("Alpha.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        app.search_query = "alpha".to_string();
        app.update_search_matches();
        assert_eq!(app.search_matches.len(), 1);
    }

    #[test]
    fn search_n_wraps_around() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a1.txt"), b"").unwrap();
        fs::write(tmp.path().join("a2.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        app.search_query = "a".to_string();
        app.update_search_matches();
        app.search_match_idx = app.search_matches.len() - 1;
        app.selected = *app.search_matches.last().unwrap();
        app.handle_key(key(KeyCode::Char('n')));
        assert_eq!(app.search_match_idx, 0);
    }

    #[test]
    fn search_capital_n_wraps_around() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a1.txt"), b"").unwrap();
        fs::write(tmp.path().join("a2.txt"), b"").unwrap();
        let mut app = make_app(&tmp);
        app.load_dir().unwrap();
        app.search_query = "a".to_string();
        app.update_search_matches();
        app.search_match_idx = 0;
        app.selected = app.search_matches[0];
        app.handle_key(key(KeyCode::Char('N')));
        assert_eq!(app.search_match_idx, app.search_matches.len() - 1);
    }
}
