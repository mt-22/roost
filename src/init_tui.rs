use std::{
    collections::HashSet,
    io::{self, Stdout},
    path::Path,
};

use color_eyre::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Tabs},
    Terminal,
};

use crate::miller::MillerColumns;
use crate::scanner::{DiscoveredItem, ItemType};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    ScanResults,
    Browse,
}

struct App {
    tab: Tab,
    scan_items: Vec<DiscoveredItem>,
    selected_indices: HashSet<usize>,
    scan_cursor: usize,
    scan_scroll: usize,
    miller: MillerColumns,
}

impl App {
    fn new(scan_items: Vec<DiscoveredItem>, root_path: &Path) -> Self {
        let mut selected_indices = HashSet::new();
        for (i, item) in scan_items.iter().enumerate() {
            if item.confidence >= 100 {
                selected_indices.insert(i);
            }
        }
        let miller = MillerColumns::new(root_path);
        Self {
            tab: Tab::ScanResults,
            scan_items,
            selected_indices,
            scan_cursor: 0,
            scan_scroll: 0,
            miller,
        }
    }

    fn selected_items(&self) -> Vec<DiscoveredItem> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();

        for &i in &self.selected_indices {
            if let Some(item) = self.scan_items.get(i)
                && seen.insert(item.path.clone())
            {
                result.push(item.clone());
            }
        }

        for path in self.miller.selected_paths() {
            if seen.insert(path.clone()) {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let item_type = if path.is_dir() {
                    ItemType::Dir
                } else {
                    ItemType::File
                };
                result.push(DiscoveredItem {
                    path: path.clone(),
                    name,
                    confidence: 0,
                    item_type,
                });
            }
        }

        result
    }

    fn scan_cursor(&mut self) {
        if self.scan_cursor > 0 {
            self.scan_cursor -= 1;
        }
    }

    fn scan_down(&mut self) {
        if !self.scan_items.is_empty() && self.scan_cursor < self.scan_items.len() - 1 {
            self.scan_cursor += 1;
        }
    }

    fn toggle_scan(&mut self) {
        if self.scan_items.is_empty() {
            return;
        }
        if !self.selected_indices.remove(&self.scan_cursor) {
            self.selected_indices.insert(self.scan_cursor);
        }
    }

    fn visible_range(&self, height: usize) -> (usize, usize) {
        let visible = if height == 0 { 1 } else { height };
        let total = self.scan_items.len();
        if total == 0 {
            return (0, 0);
        }
        let scroll = if self.scan_cursor >= visible {
            self.scan_cursor - visible + 1
        } else {
            0
        };
        let end = (scroll + visible).min(total);
        (scroll, end)
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    crossterm::execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    terminal.clear()?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

pub fn run_selection_tui(
    scan_items: Vec<DiscoveredItem>,
    root_path: &Path,
) -> Result<Vec<DiscoveredItem>> {
    let mut terminal = setup_terminal()?;
    let result = run_app(&mut terminal, scan_items, root_path);
    restore_terminal(&mut terminal)?;
    result
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    scan_items: Vec<DiscoveredItem>,
    root_path: &Path,
) -> Result<Vec<DiscoveredItem>> {
    let mut app = App::new(scan_items, root_path);

    loop {
        terminal.draw(|f| {
            let chunks = Layout::vertical([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(f.area());

            render_tab_bar(f, chunks[0], app.tab);
            render_content(f, chunks[1], &mut app);
            render_key_hints(f, chunks[2]);
        })?;

        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Tab => {
                    app.tab = match app.tab {
                        Tab::ScanResults => Tab::Browse,
                        Tab::Browse => Tab::ScanResults,
                    };
                }
                KeyCode::Enter => {
                    return Ok(app.selected_items());
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    return Ok(Vec::new());
                }
                KeyCode::Char(' ') => match app.tab {
                    Tab::ScanResults => app.toggle_scan(),
                    Tab::Browse => app.miller.toggle_select(),
                },
                KeyCode::Up | KeyCode::Char('k') => match app.tab {
                    Tab::ScanResults => app.scan_cursor(),
                    Tab::Browse => app.miller.move_up(),
                },
                KeyCode::Down | KeyCode::Char('j') => match app.tab {
                    Tab::ScanResults => app.scan_down(),
                    Tab::Browse => app.miller.move_down(),
                },
                KeyCode::Left | KeyCode::Char('h') => match app.tab {
                    Tab::ScanResults => {}
                    Tab::Browse => app.miller.navigate_up(),
                },
                KeyCode::Right | KeyCode::Char('l') => match app.tab {
                    Tab::ScanResults => {}
                    Tab::Browse => app.miller.navigate_down(),
                },
                _ => {}
            }
        }
    }
}

fn render_tab_bar(frame: &mut ratatui::Frame, area: Rect, active: Tab) {
    let titles = vec![
        Span::styled(
            " [1] Scan Results ",
            if active == Tab::ScanResults {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ),
        Span::styled(
            " [2] Browse ",
            if active == Tab::Browse {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ),
    ];
    let tabs = Tabs::new(titles);
    frame.render_widget(tabs, area);
}

fn render_content(frame: &mut ratatui::Frame, area: Rect, app: &mut App) {
    match app.tab {
        Tab::ScanResults => render_scan_list(frame, area, app),
        Tab::Browse => {
            frame.render_widget(&app.miller, area);
        }
    }
}

fn render_scan_list(frame: &mut ratatui::Frame, area: Rect, app: &mut App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Scan Results ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let visible = inner.height as usize;
    let (scroll, end) = app.visible_range(visible);
    app.scan_scroll = scroll;

    let items: Vec<ListItem> = (scroll..end)
        .map(|i| {
            let item = &app.scan_items[i];
            let selected = app.selected_indices.contains(&i);
            let cursor = i == app.scan_cursor;

            let check = if selected { "\u{2611}" } else { "\u{2610}" };
            let type_marker = match item.item_type {
                ItemType::Dir => "Dir ",
                ItemType::File => "File",
            };

            let short_path = item
                .path
                .parent()
                .and_then(|p| p.to_str())
                .unwrap_or("");

            let line = Line::from(vec![
                Span::styled(
                    format!(" {check} "),
                    if selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
                Span::styled(
                    format!("{:<20}", truncate_str(&item.name, 20)),
                    if cursor {
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    },
                ),
                Span::styled(
                    format!(" {:<3}", type_marker),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!(" {:>3}", item.confidence),
                    confidence_style(item.confidence),
                ),
                Span::styled(
                    format!(" {}", truncate_str(short_path, 40)),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);

            let style = if cursor {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            ListItem::new(line).style(style)
        })
        .collect();

    let list = List::new(items);
    let mut state = ListState::default();
    state.select(if app.scan_items.is_empty() {
        None
    } else {
        Some(app.scan_cursor.saturating_sub(scroll))
    });
    frame.render_stateful_widget(list, inner, &mut state);
}

fn confidence_style(confidence: u32) -> Style {
    if confidence >= 150 {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else if confidence >= 100 {
        Style::default().fg(Color::Green)
    } else if confidence >= 50 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn truncate_str(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else if max > 2 {
        let mut truncated: String = chars[..max - 2].iter().collect();
        truncated.push_str("..");
        truncated
    } else {
        chars[..max].iter().collect()
    }
}

fn render_key_hints(frame: &mut ratatui::Frame, area: Rect) {
    let hints = " Tab: switch \u{2502} Space: select \u{2502} Enter: confirm \u{2502} Esc: cancel ";
    let line = Line::from(Span::styled(
        hints,
        Style::default().fg(Color::DarkGray),
    ));
    frame.render_widget(line, area);
}
