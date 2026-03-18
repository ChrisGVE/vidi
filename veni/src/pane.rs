use crate::app::DirEntry;
use crate::error::{Result, VeniError};
use std::path::PathBuf;

/// Action dispatched to a pane for navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationAction {
    Down,
    Up,
    Top,
    Bottom,
    Enter,
    Parent,
}

/// State for one file-manager pane.
#[derive(Debug, Clone)]
pub struct Pane {
    pub cwd: PathBuf,
    pub entries: Vec<DirEntry>,
    pub selected: usize,
    pub scroll_offset: usize,
}

impl Pane {
    pub fn new(path: PathBuf) -> Self {
        Self {
            cwd: path,
            entries: Vec::new(),
            selected: 0,
            scroll_offset: 0,
        }
    }

    /// Read `cwd` and populate `entries`.
    ///
    /// Sort order: directories first, then files; alphabetical within each
    /// group (case-insensitive).  Dotfiles are included only when
    /// `show_hidden` is true.
    pub fn load_dir(&mut self, show_hidden: bool) -> Result<()> {
        let read_dir = std::fs::read_dir(&self.cwd).map_err(|source| VeniError::ReadDir {
            path: self.cwd.clone(),
            source,
        })?;

        let mut entries: Vec<DirEntry> = Vec::new();
        for entry_result in read_dir {
            let entry = match entry_result {
                Ok(e) => e,
                Err(_) => continue,
            };

            let name = entry.file_name().to_string_lossy().into_owned();

            if !show_hidden && name.starts_with('.') {
                continue;
            }

            let full_path = entry.path();
            // Use symlink_metadata so symlinks are not followed for type detection.
            let symlink_meta = std::fs::symlink_metadata(&full_path).ok();
            let is_symlink = symlink_meta
                .as_ref()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false);
            let symlink_target = if is_symlink {
                std::fs::read_link(&full_path).ok()
            } else {
                None
            };

            // For size/modified, follow the symlink (use metadata, not symlink_metadata).
            let meta = entry.metadata().ok();
            let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let modified = meta.and_then(|m| m.modified().ok());

            entries.push(DirEntry {
                name,
                path: full_path,
                is_dir,
                is_symlink,
                symlink_target,
                size,
                modified,
            });
        }

        entries.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        self.entries = entries;
        self.selected = 0;
        self.scroll_offset = 0;
        Ok(())
    }

    /// Apply a navigation action to this pane.
    pub fn handle_navigation(&mut self, action: NavigationAction, show_hidden: bool) {
        match action {
            NavigationAction::Down => self.move_down(),
            NavigationAction::Up => self.move_up(),
            NavigationAction::Top => self.go_top(),
            NavigationAction::Bottom => self.go_bottom(),
            NavigationAction::Enter => self.enter_dir(show_hidden),
            NavigationAction::Parent => self.go_parent(show_hidden),
        }
    }

    /// Currently highlighted entry, if any.
    pub fn current_entry(&self) -> Option<&DirEntry> {
        self.entries.get(self.selected)
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

    fn enter_dir(&mut self, show_hidden: bool) {
        if let Some(entry) = self.entries.get(self.selected) {
            // Follow symlink targets that point to a directory.
            let target = if entry.is_symlink {
                entry.symlink_target.clone().filter(|t| t.is_dir())
            } else if entry.is_dir {
                Some(entry.path.clone())
            } else {
                None
            };
            if let Some(new_path) = target {
                self.cwd = new_path;
                let _ = self.load_dir(show_hidden);
            }
        }
    }

    /// Adjust `scroll_offset` so that `selected` is within the visible window
    /// of `visible_height` rows.
    pub fn ensure_visible(&mut self, visible_height: usize) {
        if visible_height == 0 {
            return;
        }
        // Scroll down if cursor is below the visible window.
        if self.selected >= self.scroll_offset + visible_height {
            self.scroll_offset = self.selected + 1 - visible_height;
        }
        // Scroll up if cursor is above the visible window.
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        }
    }

    fn go_parent(&mut self, show_hidden: bool) {
        if let Some(parent) = self.cwd.parent().map(|p| p.to_path_buf()) {
            self.cwd = parent;
            let _ = self.load_dir(show_hidden);
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

    fn make_pane(dir: &TempDir) -> Pane {
        Pane::new(dir.path().to_path_buf())
    }

    #[test]
    fn load_dir_lists_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("file.txt"), b"hello").unwrap();
        let mut pane = make_pane(&tmp);
        pane.load_dir(false).unwrap();
        assert_eq!(pane.entries.len(), 1);
        assert_eq!(pane.entries[0].name, "file.txt");
    }

    #[test]
    fn load_dir_sorts_dirs_before_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("aaa.txt"), b"").unwrap();
        fs::create_dir(tmp.path().join("bbb_dir")).unwrap();
        let mut pane = make_pane(&tmp);
        pane.load_dir(false).unwrap();
        assert!(pane.entries[0].is_dir, "directory must come first");
        assert!(!pane.entries[1].is_dir);
    }

    #[test]
    fn load_dir_hides_dotfiles_by_default() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".hidden"), b"").unwrap();
        fs::write(tmp.path().join("visible.txt"), b"").unwrap();
        let mut pane = make_pane(&tmp);
        pane.load_dir(false).unwrap();
        assert_eq!(pane.entries.len(), 1);
        assert_eq!(pane.entries[0].name, "visible.txt");
    }

    #[test]
    fn load_dir_shows_dotfiles_when_enabled() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".hidden"), b"").unwrap();
        fs::write(tmp.path().join("visible.txt"), b"").unwrap();
        let mut pane = make_pane(&tmp);
        pane.load_dir(true).unwrap();
        assert_eq!(pane.entries.len(), 2);
    }

    #[test]
    fn load_dir_resets_cursor() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), b"").unwrap();
        fs::write(tmp.path().join("b.txt"), b"").unwrap();
        let mut pane = make_pane(&tmp);
        pane.load_dir(false).unwrap();
        pane.selected = 1;
        pane.load_dir(false).unwrap();
        assert_eq!(pane.selected, 0);
    }

    #[test]
    fn navigate_down_and_up() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), b"").unwrap();
        fs::write(tmp.path().join("b.txt"), b"").unwrap();
        let mut pane = make_pane(&tmp);
        pane.load_dir(false).unwrap();
        pane.handle_navigation(NavigationAction::Down, false);
        assert_eq!(pane.selected, 1);
        pane.handle_navigation(NavigationAction::Up, false);
        assert_eq!(pane.selected, 0);
    }

    #[test]
    fn navigate_bottom_and_top() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), b"").unwrap();
        fs::write(tmp.path().join("b.txt"), b"").unwrap();
        fs::write(tmp.path().join("c.txt"), b"").unwrap();
        let mut pane = make_pane(&tmp);
        pane.load_dir(false).unwrap();
        pane.handle_navigation(NavigationAction::Bottom, false);
        assert_eq!(pane.selected, 2);
        pane.handle_navigation(NavigationAction::Top, false);
        assert_eq!(pane.selected, 0);
        assert_eq!(pane.scroll_offset, 0);
    }

    #[test]
    fn navigate_down_at_bottom_does_not_overflow() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), b"").unwrap();
        let mut pane = make_pane(&tmp);
        pane.load_dir(false).unwrap();
        pane.handle_navigation(NavigationAction::Down, false);
        assert_eq!(pane.selected, 0);
    }

    #[test]
    fn navigate_up_at_top_does_not_underflow() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), b"").unwrap();
        let mut pane = make_pane(&tmp);
        pane.load_dir(false).unwrap();
        pane.handle_navigation(NavigationAction::Up, false);
        assert_eq!(pane.selected, 0);
    }

    #[test]
    fn navigate_enter_changes_cwd() {
        let tmp = TempDir::new().unwrap();
        let subdir = tmp.path().join("subdir");
        fs::create_dir(&subdir).unwrap();
        let mut pane = make_pane(&tmp);
        pane.load_dir(false).unwrap();
        assert_eq!(pane.entries[0].name, "subdir");
        let expected = pane.entries[0].path.clone();
        pane.handle_navigation(NavigationAction::Enter, false);
        assert_eq!(pane.cwd, expected);
    }

    #[test]
    fn navigate_parent_goes_up() {
        let tmp = TempDir::new().unwrap();
        let subdir = tmp.path().join("sub");
        fs::create_dir(&subdir).unwrap();
        let mut pane = Pane::new(subdir.clone());
        pane.load_dir(false).unwrap();
        let parent = tmp.path().to_path_buf();
        pane.handle_navigation(NavigationAction::Parent, false);
        assert_eq!(
            pane.cwd.canonicalize().unwrap_or(pane.cwd.clone()),
            parent.canonicalize().unwrap_or(parent)
        );
    }

    #[test]
    fn current_entry_returns_selected() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), b"").unwrap();
        let mut pane = make_pane(&tmp);
        pane.load_dir(false).unwrap();
        assert!(pane.current_entry().is_some());
        assert_eq!(pane.current_entry().unwrap().name, "a.txt");
    }

    #[test]
    fn current_entry_empty_pane() {
        let tmp = TempDir::new().unwrap();
        let mut pane = make_pane(&tmp);
        pane.load_dir(false).unwrap();
        assert!(pane.current_entry().is_none());
    }

    // ------------------------------------------------------------------
    // Symlink tests
    // ------------------------------------------------------------------

    #[cfg(unix)]
    #[test]
    fn load_dir_detects_symlink_to_file() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("real.txt");
        fs::write(&target, b"hello").unwrap();
        std::os::unix::fs::symlink(&target, tmp.path().join("link.txt")).unwrap();
        let mut pane = make_pane(&tmp);
        pane.load_dir(false).unwrap();
        let link = pane.entries.iter().find(|e| e.name == "link.txt").unwrap();
        assert!(link.is_symlink, "link.txt must be detected as a symlink");
        assert_eq!(
            link.symlink_target.as_deref(),
            Some(target.as_path()),
            "symlink target must be the real file path"
        );
    }

    #[cfg(unix)]
    #[test]
    fn load_dir_detects_symlink_to_dir() {
        let tmp = TempDir::new().unwrap();
        let target_dir = tmp.path().join("real_dir");
        fs::create_dir(&target_dir).unwrap();
        std::os::unix::fs::symlink(&target_dir, tmp.path().join("link_dir")).unwrap();
        let mut pane = make_pane(&tmp);
        pane.load_dir(false).unwrap();
        let link = pane.entries.iter().find(|e| e.name == "link_dir").unwrap();
        assert!(link.is_symlink, "link_dir must be detected as a symlink");
        assert_eq!(
            link.symlink_target.as_deref(),
            Some(target_dir.as_path()),
            "symlink target must be the real directory path"
        );
    }

    #[cfg(unix)]
    #[test]
    fn navigate_enter_follows_symlink_to_dir() {
        let tmp = TempDir::new().unwrap();
        let real_dir = tmp.path().join("real_dir");
        fs::create_dir(&real_dir).unwrap();
        fs::write(real_dir.join("inside.txt"), b"").unwrap();
        std::os::unix::fs::symlink(&real_dir, tmp.path().join("link_dir")).unwrap();
        let mut pane = make_pane(&tmp);
        pane.load_dir(false).unwrap();
        // Find and select the symlink entry.
        let idx = pane
            .entries
            .iter()
            .position(|e| e.name == "link_dir")
            .unwrap();
        pane.selected = idx;
        pane.handle_navigation(NavigationAction::Enter, false);
        assert_eq!(
            pane.cwd, real_dir,
            "entering a dir symlink must navigate to the target"
        );
        assert_eq!(pane.entries.len(), 1);
        assert_eq!(pane.entries[0].name, "inside.txt");
    }

    // ------------------------------------------------------------------
    // Virtual scrolling / ensure_visible tests
    // ------------------------------------------------------------------

    #[test]
    fn ensure_visible_scrolls_down_when_cursor_below_window() {
        let tmp = TempDir::new().unwrap();
        for i in 0..10 {
            fs::write(tmp.path().join(format!("{:02}.txt", i)), b"").unwrap();
        }
        let mut pane = make_pane(&tmp);
        pane.load_dir(false).unwrap();
        // Move cursor past the visible window of 5 rows.
        pane.selected = 7;
        pane.scroll_offset = 0;
        pane.ensure_visible(5);
        assert!(
            pane.scroll_offset > 0,
            "scroll_offset must advance when cursor is below window"
        );
        assert!(
            pane.selected >= pane.scroll_offset,
            "selected must be >= scroll_offset"
        );
        assert!(
            pane.selected < pane.scroll_offset + 5,
            "selected must be within the visible window"
        );
    }

    #[test]
    fn ensure_visible_scrolls_up_when_cursor_above_window() {
        let tmp = TempDir::new().unwrap();
        for i in 0..10 {
            fs::write(tmp.path().join(format!("{:02}.txt", i)), b"").unwrap();
        }
        let mut pane = make_pane(&tmp);
        pane.load_dir(false).unwrap();
        pane.selected = 2;
        pane.scroll_offset = 5; // cursor is above the window
        pane.ensure_visible(5);
        assert_eq!(
            pane.scroll_offset, 2,
            "scroll_offset must shrink to bring cursor into view"
        );
    }

    #[test]
    fn ensure_visible_noop_when_cursor_in_window() {
        let tmp = TempDir::new().unwrap();
        for i in 0..10 {
            fs::write(tmp.path().join(format!("{:02}.txt", i)), b"").unwrap();
        }
        let mut pane = make_pane(&tmp);
        pane.load_dir(false).unwrap();
        pane.selected = 2;
        pane.scroll_offset = 0;
        pane.ensure_visible(5);
        assert_eq!(
            pane.scroll_offset, 0,
            "scroll_offset must not change when cursor is already visible"
        );
    }
}
