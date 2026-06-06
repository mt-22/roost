use std::{
    collections::HashSet,
    fs::{self, DirEntry},
    path::{Path, PathBuf},
};

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};

use crate::tui::rooster::render_rooster_braille;

pub struct MillerColumn {
    pub path: PathBuf,
    pub entries: Vec<DirEntry>,
    pub cursor: usize,
    pub scroll: usize,
}

impl MillerColumn {
    fn load(path: PathBuf) -> std::io::Result<Self> {
        let mut entries: Vec<DirEntry> = fs::read_dir(&path)?.filter_map(|e| e.ok()).collect();
        entries.sort_by(|a, b| {
            let a_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let b_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
            b_dir
                .cmp(&a_dir)
                .then_with(|| a.file_name().cmp(&b.file_name()))
        });
        Ok(Self {
            path,
            entries,
            cursor: 0,
            scroll: 0,
        })
    }

    fn empty(path: PathBuf) -> Self {
        Self {
            path,
            entries: Vec::new(),
            cursor: 0,
            scroll: 0,
        }
    }
}

pub struct MillerColumns {
    columns: Vec<MillerColumn>,
    root: PathBuf,
    selected: HashSet<PathBuf>,
    /// When set, only these indices are shown in the current (last) column.
    filtered_indices: Option<Vec<usize>>,
    filtered_cursor: usize,
    /// Path to the primary config file for the currently selected app.
    /// If set, the matching entry is highlighted with a `★` marker.
    primary_config: Option<PathBuf>,
    pub rooster_is_pecking: bool,
}

impl MillerColumns {
    pub fn new(root: &Path) -> Self {
        let column = MillerColumn::load(root.to_path_buf())
            .unwrap_or_else(|_| MillerColumn::empty(root.to_path_buf()));
        Self {
            columns: vec![column],
            root: root.to_path_buf(),
            selected: HashSet::new(),
            filtered_indices: None,
            filtered_cursor: 0,
            primary_config: None,
            rooster_is_pecking: false,
        }
    }

    /// Re-initialize the browser at a new root, preserving selected paths.
    pub fn set_root(&mut self, root: &Path) {
        self.root = root.to_path_buf();
        let column = MillerColumn::load(self.root.clone())
            .unwrap_or_else(|_| MillerColumn::empty(self.root.clone()));
        self.columns = vec![column];
        self.filtered_indices = None;
        self.filtered_cursor = 0;
        self.primary_config = None;
    }

    /// Set the primary config path for the currently selected app.
    /// This is used to highlight the primary config file in the miller columns.
    pub fn set_primary_config(&mut self, path: Option<PathBuf>) {
        self.primary_config = path;
    }

    /// Get the primary config path.
    pub fn primary_config(&self) -> Option<&PathBuf> {
        self.primary_config.as_ref()
    }

    pub fn set_filter(&mut self, indices: Vec<usize>) {
        self.filtered_indices = Some(indices);
        self.filtered_cursor = 0;
    }

    pub fn clear_filter(&mut self) {
        self.filtered_indices = None;
        self.filtered_cursor = 0;
    }

    pub fn is_filtered(&self) -> bool {
        self.filtered_indices.is_some()
    }

    pub fn filtered_cursor(&self) -> usize {
        self.filtered_cursor
    }

    pub fn move_filtered_up(&mut self) {
        if self.filtered_cursor > 0 {
            self.filtered_cursor -= 1;
        }
    }

    pub fn move_filtered_down(&mut self) {
        if let Some(ref indices) = self.filtered_indices {
            if !indices.is_empty() && self.filtered_cursor < indices.len() - 1 {
                self.filtered_cursor += 1;
            }
        }
    }

    /// Set the filtered cursor to the position of `original_index` within the
    /// filtered indices. This is used when navigating via an external search
    /// engine that operates on original indices.
    pub fn sync_filtered_cursor(&mut self, original_index: usize) {
        if let Some(ref indices) = self.filtered_indices {
            if let Some(pos) = indices.iter().position(|&i| i == original_index) {
                self.filtered_cursor = pos;
            }
        }
    }

    fn real_cursor(&self) -> usize {
        match self.filtered_indices {
            Some(ref indices) => indices.get(self.filtered_cursor).copied().unwrap_or(0),
            None => self.columns[self.columns.len() - 1].cursor,
        }
    }

    pub fn navigate_down(&mut self) {
        let cursor = self.real_cursor();
        let path = {
            let current = &self.columns[self.columns.len() - 1];
            match current.entries.get(cursor) {
                Some(e) if e.file_type().map(|t| t.is_dir()).unwrap_or(false) => Some(e.path()),
                _ => None,
            }
        };
        if let Some(path) = path
            && let Ok(col) = MillerColumn::load(path)
        {
            self.columns.push(col);
            self.filtered_indices = None;
            self.filtered_cursor = 0;
        }
    }

    pub fn navigate_up(&mut self) {
        if self.columns.len() > 1 {
            self.columns.pop();
            self.filtered_indices = None;
            self.filtered_cursor = 0;
        }
    }

    pub fn select_path(&mut self, path: PathBuf) {
        self.selected.insert(path);
    }

    pub fn deselect_path(&mut self, path: &Path) {
        self.selected.remove(path);
    }

    pub fn toggle_select(&mut self) {
        let cursor = self.real_cursor();
        let path = {
            let current = &self.columns[self.columns.len() - 1];
            current.entries.get(cursor).map(|e| e.path())
        };
        if let Some(path) = path
            && !self.selected.remove(&path)
        {
            self.selected.insert(path);
        }
    }

    pub fn move_up(&mut self) {
        let last = self.columns.len() - 1;
        let col = &mut self.columns[last];
        if col.cursor > 0 {
            col.cursor -= 1;
        }
    }

    pub fn move_down(&mut self) {
        let last = self.columns.len() - 1;
        let col = &mut self.columns[last];
        if !col.entries.is_empty() && col.cursor < col.entries.len() - 1 {
            col.cursor += 1;
        }
    }

    pub fn selected_paths(&self) -> &HashSet<PathBuf> {
        &self.selected
    }

    pub fn current_path(&self) -> &Path {
        &self.columns[self.columns.len() - 1].path
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn current_cursor_path(&self) -> Option<PathBuf> {
        let cursor = self.real_cursor();
        let current = &self.columns[self.columns.len() - 1];
        current.entries.get(cursor).map(|e| e.path())
    }

    pub fn current_entries(&self) -> &[DirEntry] {
        let current = &self.columns[self.columns.len() - 1];
        &current.entries
    }

    /// True when the browser is at the root directory (only one column).
    pub fn is_at_root(&self) -> bool {
        self.columns.len() == 1
    }

    /// True if the currently selected entry is a directory.
    pub fn current_cursor_is_dir(&self) -> bool {
        let cursor = self.real_cursor();
        let current = &self.columns[self.columns.len() - 1];
        current
            .entries
            .get(cursor)
            .and_then(|e| e.file_type().ok())
            .map(|t| t.is_dir())
            .unwrap_or(false)
    }

    pub fn current_cursor(&self) -> usize {
        let current = &self.columns[self.columns.len() - 1];
        current.cursor
    }

    pub fn set_current_cursor(&mut self, cursor: usize) {
        let last = self.columns.len() - 1;
        let col = &mut self.columns[last];
        if !col.entries.is_empty() {
            col.cursor = cursor.min(col.entries.len() - 1);
        } else {
            col.cursor = 0;
        }
    }
}

impl Widget for &MillerColumns {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let three_column = area.width >= NARROW_WIDTH;
        let stacked = area.width < VERY_NARROW_WIDTH;

        let chunks = if stacked {
            Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area)
        } else if three_column {
            Layout::horizontal([
                Constraint::Percentage(33),
                Constraint::Percentage(34),
                Constraint::Percentage(33),
            ])
            .split(area)
        } else {
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area)
        };

        let current_idx = self.columns.len() - 1;

        // Parent column (only in 3-column wide mode)
        if three_column {
            if current_idx > 0 {
                let parent = &self.columns[current_idx - 1];
                let current_path = &self.columns[current_idx].path;
                let hl_idx = parent
                    .entries
                    .iter()
                    .position(|e| e.path() == *current_path)
                    .unwrap_or(0);
                let parent_refs: Vec<&DirEntry> = parent.entries.iter().collect();
                render_entries(
                    buf,
                    chunks[0],
                    &parent_refs,
                    hl_idx,
                    None,
                    Some(current_path.as_path()),
                    &self.selected,
                    false,
                    None,
                    self.primary_config.as_deref(),
                );
            } else {
                render_empty(buf, chunks[0]);
            }
        }

        // Current (active) column
        let current = &self.columns[current_idx];
        let current_refs: Vec<&DirEntry> = match self.filtered_indices {
            Some(ref indices) => indices
                .iter()
                .filter_map(|&i| current.entries.get(i))
                .collect(),
            None => current.entries.iter().collect(),
        };
        let (current_cursor, active_cursor) = match self.filtered_indices {
            Some(_) => (self.filtered_cursor, Some(self.filtered_cursor)),
            None => (current.cursor, Some(current.cursor)),
        };

        // Build current column title: "Current: [dir_name]" truncated to ~20 chars
        let current_dir_name = current
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let truncated_name = if current_dir_name.chars().count() > 20 {
            current_dir_name.chars().take(20).collect::<String>() + "…"
        } else {
            current_dir_name.to_string()
        };
        let current_title = format!(" Current: {} ", truncated_name);

        let current_chunk = if three_column { chunks[1] } else { chunks[0] };
        render_entries(
            buf,
            current_chunk,
            &current_refs,
            current_cursor,
            active_cursor,
            None,
            &self.selected,
            true,
            Some(&current_title),
            self.primary_config.as_deref(),
        );

        let preview_chunk = if three_column { chunks[2] } else { chunks[1] };
        let real_cursor = self.real_cursor();
        if let Some(entry) = current.entries.get(real_cursor) {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                match MillerColumn::load(entry.path()) {
                    Ok(preview) => {
                        let preview_refs: Vec<&DirEntry> = preview.entries.iter().collect();
                        render_entries(
                            buf,
                            preview_chunk,
                            &preview_refs,
                            0,
                            None,
                            None,
                            &self.selected,
                            false,
                            None,
                            self.primary_config.as_deref(),
                        )
                    }
                    Err(_) => render_empty(buf, preview_chunk),
                }
            } else {
                render_file_preview(buf, preview_chunk, &entry.path());
            }
        } else {
            render_empty(buf, preview_chunk);
        }

        // Rooster at top-right of the rightmost/ topmost column
        let rooster_col = if stacked { chunks[0] } else { preview_chunk };
        let rw = 10u16.min(rooster_col.width);
        let rh = 3u16.min(rooster_col.height);
        if rw > 0 && rh > 0 {
            let rooster_area = Rect::new(
                rooster_col.x + rooster_col.width.saturating_sub(rw),
                rooster_col.y.saturating_sub(2),
                rw,
                rh,
            );
            render_rooster_braille(buf, rooster_area, self.rooster_is_pecking);
        }
    }
}

/// Threshold below which the miller columns drop the parent column and show
/// only two columns (current + preview) side by side.
const NARROW_WIDTH: u16 = 100;

/// Threshold below which the miller columns stack vertically instead of
/// side by side: current on top, preview below.
const VERY_NARROW_WIDTH: u16 = 55;

pub(crate) fn render_entries(
    buf: &mut Buffer,
    area: Rect,
    entries: &[&DirEntry],
    focus: usize,
    active_cursor: Option<usize>,
    highlight_path: Option<&Path>,
    selected: &HashSet<PathBuf>,
    is_active_column: bool,
    title: Option<&str>,
    primary_config: Option<&Path>,
) {
    let border_color = if is_active_column {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let title = title.unwrap_or_else(|| {
        if is_active_column {
            " Current "
        } else if highlight_path.is_some() {
            " Parent "
        } else {
            " Preview "
        }
    });

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(title);
    let inner = block.inner(area);
    block.render(area, buf);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let visible = inner.height as usize;
    let scroll = if focus >= visible {
        focus - visible + 1
    } else {
        0
    };

    for (vi, idx) in (scroll..entries.len()).enumerate() {
        if vi >= visible {
            break;
        }
        let entry = &entries[idx];
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let path = entry.path();
        let is_sel = selected.contains(&path);
        let is_hl = highlight_path.is_some_and(|hp| hp == path);
        let is_cur = active_cursor == Some(idx);
        let is_primary = primary_config.is_some_and(|pc| pc == path);

        let suffix = if is_dir { "/" } else { "" };

        // Prefix: cursor indicator, selected indicator, or primary config marker
        let prefix = if is_cur {
            "» "
        } else if is_primary {
            "★ "
        } else if is_sel {
            "✓ "
        } else {
            "  "
        };

        let display = format!("{prefix}{name}{suffix}");

        let max_len = inner.width as usize;
        let truncated: String = display.chars().take(max_len).collect();

        let style = if is_cur {
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else if is_hl {
            Style::default().bg(Color::DarkGray).fg(Color::White)
        } else if is_primary {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else if is_sel {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else if is_dir {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        buf.set_string(inner.x, inner.y + vi as u16, &truncated, style);
    }
}

fn render_empty(buf: &mut Buffer, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    block.render(area, buf);
}

fn render_file_preview(buf: &mut Buffer, area: Rect, path: &Path) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Preview ");
    let inner = block.inner(area);
    block.render(area, buf);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let content = match std::fs::read(path) {
        Ok(bytes) => {
            if bytes.contains(&0) || String::from_utf8(bytes.clone()).is_err() {
                None
            } else {
                String::from_utf8(bytes).ok()
            }
        }
        Err(_) => None,
    };

    match content {
        Some(text) => {
            let lines: Vec<&str> = text.lines().collect();
            let visible = inner.height as usize;
            for (i, line) in lines.iter().take(visible).enumerate() {
                let truncated: String = line.chars().take(inner.width as usize).collect();
                buf.set_string(
                    inner.x,
                    inner.y + i as u16,
                    &truncated,
                    Style::default().fg(Color::White),
                );
            }
        }
        None => {
            if inner.height > 0 && inner.width > 6 {
                buf.set_string(
                    inner.x,
                    inner.y,
                    "(binary file)",
                    Style::default().fg(Color::DarkGray),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn load_reads_directory() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("subdir")).unwrap();
        fs::write(dir.path().join("alpha.txt"), b"a").unwrap();
        fs::write(dir.path().join("beta.txt"), b"b").unwrap();

        let col = MillerColumn::load(dir.path().to_path_buf()).unwrap();
        assert!(!col.entries.is_empty());
        assert_eq!(col.cursor, 0);
        assert_eq!(col.scroll, 0);

        let names: Vec<String> = col
            .entries
            .iter()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["subdir", "alpha.txt", "beta.txt"]);
    }

    #[test]
    fn load_empty_directory() {
        let dir = tempfile::tempdir().unwrap();
        let col = MillerColumn::load(dir.path().to_path_buf()).unwrap();
        assert!(col.entries.is_empty());
        assert_eq!(col.cursor, 0);
        assert_eq!(col.scroll, 0);
    }

    #[test]
    fn move_up_clamps_at_zero() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"a").unwrap();
        fs::write(dir.path().join("b.txt"), b"b").unwrap();
        let mut mc = MillerColumns::new(dir.path());
        mc.move_up();
        assert_eq!(mc.current_cursor_path().unwrap(), dir.path().join("a.txt"));
    }

    #[test]
    fn move_down_advances_and_clamps() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..6u8 {
            fs::write(dir.path().join(format!("file{i}.txt")), [i]).unwrap();
        }
        let mut mc = MillerColumns::new(dir.path());

        mc.move_down();
        assert_eq!(
            mc.current_cursor_path().unwrap(),
            dir.path().join("file1.txt")
        );
        mc.move_down();
        assert_eq!(
            mc.current_cursor_path().unwrap(),
            dir.path().join("file2.txt")
        );
        mc.move_down();
        mc.move_down();
        mc.move_down();
        mc.move_down();
        mc.move_down();
        assert_eq!(
            mc.current_cursor_path().unwrap(),
            dir.path().join("file5.txt")
        );
    }

    #[test]
    fn toggle_select_adds_and_removes() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("item.txt"), b"x").unwrap();
        let mut mc = MillerColumns::new(dir.path());

        mc.toggle_select();
        assert!(mc.selected_paths().contains(&dir.path().join("item.txt")));

        mc.toggle_select();
        assert!(!mc.selected_paths().contains(&dir.path().join("item.txt")));
    }

    #[test]
    fn navigate_down_pushes_column_for_dir() {
        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("inner");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(subdir.join("deep.txt"), b"d").unwrap();
        fs::write(dir.path().join("top.txt"), b"t").unwrap();

        let mut mc = MillerColumns::new(dir.path());
        assert_eq!(mc.current_path(), dir.path());

        mc.navigate_down();
        assert_eq!(mc.current_path(), subdir);
    }

    #[test]
    fn navigate_down_ignores_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("plain.txt"), b"p").unwrap();

        let mut mc = MillerColumns::new(dir.path());
        assert_eq!(mc.current_path(), dir.path());
        mc.navigate_down();
        assert_eq!(mc.current_path(), dir.path());
    }

    #[test]
    fn navigate_up_pops_but_not_below_one() {
        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("sub");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(dir.path().join("a.txt"), b"a").unwrap();

        let mut mc = MillerColumns::new(dir.path());
        mc.navigate_down();
        assert_eq!(mc.current_path(), subdir);

        mc.navigate_up();
        assert_eq!(mc.current_path(), dir.path());

        mc.navigate_up();
        assert_eq!(mc.current_path(), dir.path());
    }

    #[test]
    fn current_path_returns_last_column_path() {
        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("child");
        fs::create_dir_all(&subdir).unwrap();

        let mut mc = MillerColumns::new(dir.path());
        assert_eq!(mc.current_path(), dir.path());
        mc.navigate_down();
        assert_eq!(mc.current_path(), subdir);
    }

    #[test]
    fn current_cursor_path_returns_entry_path() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("aaa.txt"), b"a").unwrap();
        fs::write(dir.path().join("bbb.txt"), b"b").unwrap();

        let mut mc = MillerColumns::new(dir.path());
        assert_eq!(
            mc.current_cursor_path().unwrap(),
            dir.path().join("aaa.txt")
        );
        mc.move_down();
        assert_eq!(
            mc.current_cursor_path().unwrap(),
            dir.path().join("bbb.txt")
        );
    }

    #[test]
    fn preview_renders_text_content() {
        let dir = tempfile::tempdir().unwrap();
        let content = "line1\nline2\nline3";
        fs::write(dir.path().join("file.txt"), content).unwrap();

        let mut mc = MillerColumns::new(dir.path());
        mc.move_down();
        assert_eq!(
            mc.current_cursor_path().unwrap(),
            dir.path().join("file.txt")
        );
        assert!(!mc.current_cursor_is_dir());
    }

    #[test]
    fn preview_binary_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("binary.bin"), vec![0u8, 1, 2, 255, 0]).unwrap();

        let mut mc = MillerColumns::new(dir.path());
        mc.move_down();
        assert!(!mc.current_cursor_is_dir());
        let path = mc.current_cursor_path().unwrap();
        let bytes = fs::read(&path).unwrap();
        assert!(bytes.contains(&0));
    }
}
