use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};
use std::{
    collections::HashSet,
    fs::{self, DirEntry},
    path::{Path, PathBuf},
};

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
}

impl MillerColumns {
    pub fn new(root: &Path) -> Self {
        let column = MillerColumn::load(root.to_path_buf()).unwrap_or_else(|_| MillerColumn::empty(root.to_path_buf()));
        Self {
            columns: vec![column],
            root: root.to_path_buf(),
            selected: HashSet::new(),
        }
    }

    pub fn navigate_down(&mut self) {
        let path = {
            let current = &self.columns[self.columns.len() - 1];
            match current.entries.get(current.cursor) {
                Some(e) if e.file_type().map(|t| t.is_dir()).unwrap_or(false) => Some(e.path()),
                _ => None,
            }
        };
        if let Some(path) = path
            && let Ok(col) = MillerColumn::load(path)
        {
            self.columns.push(col);
        }
    }

    pub fn navigate_up(&mut self) {
        if self.columns.len() > 1 {
            self.columns.pop();
        }
    }

    pub fn toggle_select(&mut self) {
        let path = {
            let current = &self.columns[self.columns.len() - 1];
            current.entries.get(current.cursor).map(|e| e.path())
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
        let current = &self.columns[self.columns.len() - 1];
        current.entries.get(current.cursor).map(|e| e.path())
    }
}

impl Widget for &MillerColumns {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::horizontal([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(area);

        let current_idx = self.columns.len() - 1;

        if current_idx > 0 {
            let parent = &self.columns[current_idx - 1];
            let current_path = &self.columns[current_idx].path;
            let hl_idx = parent
                .entries
                .iter()
                .position(|e| e.path() == *current_path)
                .unwrap_or(0);
            render_entries(
                buf,
                chunks[0],
                &parent.entries,
                hl_idx,
                None,
                Some(current_path.as_path()),
                &self.selected,
            );
        } else {
            render_empty(buf, chunks[0]);
        }

        let current = &self.columns[current_idx];
        render_entries(
            buf,
            chunks[1],
            &current.entries,
            current.cursor,
            Some(current.cursor),
            None,
            &self.selected,
        );

        if let Some(entry) = current.entries.get(current.cursor) {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                match MillerColumn::load(entry.path()) {
                    Ok(preview) => render_entries(
                        buf,
                        chunks[2],
                        &preview.entries,
                        0,
                        None,
                        None,
                        &self.selected,
                    ),
                    Err(_) => render_empty(buf, chunks[2]),
                }
            } else {
                render_file_indicator(buf, chunks[2]);
            }
        } else {
            render_empty(buf, chunks[2]);
        }
    }
}

fn render_entries(
    buf: &mut Buffer,
    area: Rect,
    entries: &[DirEntry],
    focus: usize,
    active_cursor: Option<usize>,
    highlight_path: Option<&Path>,
    selected: &HashSet<PathBuf>,
) {
    let block = Block::default().borders(Borders::ALL);
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

        let marker = if is_sel { "\u{2611} " } else { "\u{2610} " };
        let suffix = if is_dir { "/" } else { "" };
        let display = format!("{marker}{name}{suffix}");

        let max_len = inner.width as usize;
        let truncated: String = display.chars().take(max_len).collect();

        let style = if is_cur {
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else if is_hl {
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
        } else if is_sel {
            Style::default()
                .fg(Color::Cyan)
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
    Block::default()
        .borders(Borders::ALL)
        .render(area, buf);
}

fn render_file_indicator(buf: &mut Buffer, area: Rect) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    block.render(area, buf);
    if inner.width > 6 && inner.height > 0 {
        buf.set_string(
            inner.x,
            inner.y,
            "(file)",
            Style::default().fg(Color::DarkGray),
        );
    }
}
