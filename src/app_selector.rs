use std::{
    collections::HashSet,
    io::{self, Stdout},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use color_eyre::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Tabs},
};

use crate::miller::MillerColumns;
use crate::scanner::{DiscoveredItem, ItemType};
use crate::tui::confirm::{ConfirmAction, ConfirmDialog, render_confirm_dialog};
use crate::tui::search::FuzzyEngine;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    ScanResults,
    Browse,
}

/// Result from the selection TUI.
pub enum TuiResult {
    /// User confirmed with selections.
    Selected(Vec<DiscoveredItem>),
    /// User aborted (Esc, q, or Ctrl+C). No state should be saved.
    Aborted,
}

struct App {
    tab: Tab,
    scan_items: Vec<DiscoveredItem>,
    selected_indices: HashSet<usize>,
    scan_cursor: usize,
    scan_scroll: usize,
    miller: MillerColumns,
    search: FuzzyEngine,
    in_search: bool,
    pre_search_scan_cursor: usize,
    confirm_dialog: Option<ConfirmDialog>,
    help_visible: bool,
    help_scroll: usize,
    help_search: FuzzyEngine,
}

impl App {
    fn new(mut scan_items: Vec<DiscoveredItem>, root_path: &Path, auto_select: bool) -> Self {
        scan_items.sort_by(|a, b| b.confidence.cmp(&a.confidence));
        let mut selected_indices = HashSet::new();
        if auto_select {
            for (i, item) in scan_items.iter().enumerate() {
                if item.confidence >= 150 {
                    selected_indices.insert(i);
                }
            }
        }
        let mut miller = MillerColumns::new(root_path);
        for &i in &selected_indices {
            if let Some(item) = scan_items.get(i) {
                miller.select_path(item.path.clone());
            }
        }
        Self {
            tab: Tab::ScanResults,
            scan_items,
            selected_indices,
            scan_cursor: 0,
            scan_scroll: 0,
            miller,
            search: FuzzyEngine::new(),
            in_search: false,
            pre_search_scan_cursor: 0,
            confirm_dialog: None,
            help_visible: false,
            help_scroll: 0,
            help_search: FuzzyEngine::new(),
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

    fn selected_count(&self) -> usize {
        self.selected_items().len()
    }

    fn is_filter_active(&self) -> bool {
        !self.search.query().is_empty()
    }

    fn activate_search(&mut self) {
        self.clear_search();
        self.in_search = true;
        self.pre_search_scan_cursor = self.scan_cursor;
        self.apply_search_filter();
    }

    fn deactivate_search(&mut self) {
        self.in_search = false;
    }

    fn clear_search(&mut self) {
        self.search.clear();
        self.miller.clear_filter();
        self.scan_cursor = self.pre_search_scan_cursor;
    }

    fn search_up(&mut self) {
        self.search.move_up();
        self.sync_cursor_from_search();
    }

    fn search_down(&mut self) {
        self.search.move_down();
        self.sync_cursor_from_search();
    }

    fn sync_cursor_from_search(&mut self) {
        match self.tab {
            Tab::ScanResults => {
                if let Some(idx) = self.search.selected_index() {
                    self.scan_cursor = idx;
                }
            }
            Tab::Browse => {
                if let Some(idx) = self.search.selected_index() {
                    self.miller.sync_filtered_cursor(idx);
                }
            }
        }
    }

    fn apply_search_filter(&mut self) {
        match self.tab {
            Tab::ScanResults => {
                let names: Vec<String> = self.scan_items.iter().map(|i| i.name.clone()).collect();
                self.search.filter(&names);
                self.sync_cursor_from_search();
            }
            Tab::Browse => {
                let names: Vec<String> = self
                    .miller
                    .current_entries()
                    .iter()
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect();
                self.search.filter(&names);
                let indices: Vec<usize> = self.search.matches().iter().map(|m| m.index).collect();
                self.miller.set_filter(indices);
                self.sync_cursor_from_search();
            }
        }
    }

    fn scan_up(&mut self) {
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
        let idx = if self.is_filter_active() {
            self.search.selected_index().unwrap_or(self.scan_cursor)
        } else {
            self.scan_cursor
        };
        if self.scan_items.is_empty() {
            return;
        }
        if self.selected_indices.remove(&idx) {
            if let Some(item) = self.scan_items.get(idx) {
                self.miller.deselect_path(&item.path);
            }
        } else {
            self.selected_indices.insert(idx);
            if let Some(item) = self.scan_items.get(idx) {
                self.miller.select_path(item.path.clone());
            }
        }
    }

    fn visible_range(&self, height: usize) -> (usize, usize) {
        let visible = if height == 0 { 1 } else { height };
        let total = if self.is_filter_active() {
            self.search.match_count()
        } else {
            self.scan_items.len()
        };
        if total == 0 {
            return (0, 0);
        }
        let cursor = if self.is_filter_active() {
            self.search.cursor()
        } else {
            self.scan_cursor
        };
        let scroll = if cursor >= visible {
            cursor - visible + 1
        } else {
            0
        };
        let end = (scroll + visible).min(total);
        (scroll, end)
    }

    fn toggle_help(&mut self) {
        self.help_visible = !self.help_visible;
        if self.help_visible {
            self.help_scroll = 0;
            self.help_search.clear();
        }
    }

    fn help_bindings(&self) -> Vec<(&str, &str)> {
        vec![
            ("?", "help"),
            ("j/k", "nav"),
            ("Tab", "focus"),
            ("/", "search"),
            ("Space", "select"),
            ("Enter", "confirm"),
            ("Esc", "cancel"),
        ]
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
    should_exit: &AtomicBool,
    auto_select: bool,
) -> Result<TuiResult> {
    crate::tui::init();
    let mut terminal = setup_terminal()?;

    let result = run_app(
        &mut terminal,
        scan_items,
        root_path,
        should_exit,
        auto_select,
    );

    restore_terminal(&mut terminal)?;

    match result {
        Ok(items) => Ok(TuiResult::Selected(items)),
        Err(e) if e.to_string() == "aborted" => Ok(TuiResult::Aborted),
        Err(e) => Err(e),
    }
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    scan_items: Vec<DiscoveredItem>,
    root_path: &Path,
    should_exit: &AtomicBool,
    auto_select: bool,
) -> Result<Vec<DiscoveredItem>> {
    let mut app = App::new(scan_items, root_path, auto_select);

    loop {
        terminal.draw(|f| {
            let main_chunks = Layout::vertical([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(f.area());

            render_tab_bar(f, main_chunks[0], app.tab);
            render_content_with_panel(f, main_chunks[1], &mut app);
            render_status_bar(f, main_chunks[2], &app);

            if app.help_visible {
                render_help_overlay(f, &app);
            }

            if app.in_search && !app.help_visible {
                render_search_overlay(f, &app);
            }

            if let Some(ref dialog) = app.confirm_dialog {
                render_confirm_dialog(f, dialog);
            }
        })?;

        if should_exit.load(Ordering::Relaxed) {
            return Err(color_eyre::eyre::eyre!("aborted"));
        }

        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            // Check if a confirmation dialog is active
            if let Some(ref mut dialog) = app.confirm_dialog {
                match key.code {
                    KeyCode::Char('y') => dialog.confirm(),
                    KeyCode::Char('n') | KeyCode::Esc => dialog.cancel(),
                    _ => {}
                }
                continue;
            }

            if app.help_visible {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                        app.toggle_help();
                        continue;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if app.help_scroll > 0 {
                            app.help_scroll -= 1;
                        }
                        continue;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        app.help_scroll += 1;
                        continue;
                    }
                    KeyCode::Backspace => {
                        app.help_search.backspace();
                        if app.help_search.query().is_empty() {
                            app.help_search.clear();
                        } else {
                            let names: Vec<String> = app
                                .help_bindings()
                                .iter()
                                .map(|(k, d)| format!("{} {}", k, d))
                                .collect();
                            app.help_search.filter(&names);
                        }
                        continue;
                    }
                    KeyCode::Char(c) => {
                        app.help_search.push_char(c);
                        let names: Vec<String> = app
                            .help_bindings()
                            .iter()
                            .map(|(k, d)| format!("{} {}", k, d))
                            .collect();
                        app.help_search.filter(&names);
                        continue;
                    }
                    _ => {}
                }
                continue;
            }

            if app.in_search {
                match key.code {
                    KeyCode::Esc | KeyCode::Enter => {
                        app.deactivate_search();
                        continue;
                    }
                    KeyCode::Char(c) => {
                        app.search.push_char(c);
                        app.apply_search_filter();
                        continue;
                    }
                    KeyCode::Backspace => {
                        app.search.backspace();
                        if app.search.query().is_empty() {
                            app.clear_search();
                        } else {
                            app.apply_search_filter();
                        }
                        continue;
                    }
                    KeyCode::Up => {
                        app.search_up();
                        continue;
                    }
                    KeyCode::Down => {
                        app.search_down();
                        continue;
                    }
                    _ => {}
                }
                continue;
            }

            match key.code {
                KeyCode::Tab => {
                    app.tab = match app.tab {
                        Tab::ScanResults => Tab::Browse,
                        Tab::Browse => Tab::ScanResults,
                    };
                    app.clear_search();
                }
                KeyCode::Enter => {
                    let count = app.selected_count();
                    if count == 0 {
                        app.confirm_dialog = Some(ConfirmDialog::affirmative(
                            "Confirm",
                            "No apps selected. Exit without selecting any?",
                        ));
                    } else {
                        app.confirm_dialog = Some(ConfirmDialog::affirmative(
                            "Confirm",
                            &format!(
                                "Ingest {} selected app{} and finish?",
                                count,
                                if count == 1 { "" } else { "s" }
                            ),
                        ));
                    }
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    let count = app.selected_count();
                    if count == 0 {
                        return Err(color_eyre::eyre::eyre!("aborted"));
                    } else {
                        app.confirm_dialog = Some(ConfirmDialog::destructive(
                            "Discard",
                            &format!(
                                "Discard {} selected app{} and exit?",
                                count,
                                if count == 1 { "" } else { "s" }
                            ),
                        ));
                    }
                }
                KeyCode::Char(' ') => match app.tab {
                    Tab::ScanResults => app.toggle_scan(),
                    Tab::Browse => app.miller.toggle_select(),
                },
                KeyCode::Up | KeyCode::Char('k') => {
                    if app.is_filter_active() {
                        app.search_up();
                    } else {
                        match app.tab {
                            Tab::ScanResults => app.scan_up(),
                            Tab::Browse => app.miller.move_up(),
                        }
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if app.is_filter_active() {
                        app.search_down();
                    } else {
                        match app.tab {
                            Tab::ScanResults => app.scan_down(),
                            Tab::Browse => app.miller.move_down(),
                        }
                    }
                }
                KeyCode::Left | KeyCode::Char('h') => match app.tab {
                    Tab::ScanResults => {}
                    Tab::Browse => {
                        app.miller.navigate_up();
                        app.clear_search();
                    }
                },
                KeyCode::Right | KeyCode::Char('l') => match app.tab {
                    Tab::ScanResults => {}
                    Tab::Browse => {
                        app.miller.navigate_down();
                        app.clear_search();
                    }
                },
                KeyCode::Char('?') => {
                    app.toggle_help();
                }
                KeyCode::Char('/') => {
                    app.activate_search();
                }
                _ => {}
            }
        }

        // Handle confirmation dialog result
        if let Some(dialog) = app.confirm_dialog.take() {
            if let Some(confirmed) = dialog.confirmed {
                if confirmed {
                    match dialog.action {
                        ConfirmAction::Confirm => return Ok(app.selected_items()),
                        ConfirmAction::Discard => return Err(color_eyre::eyre::eyre!("aborted")),
                    }
                } else {
                    // Dialog was cancelled, continue
                    continue;
                }
            } else {
                // Dialog still active, put it back
                app.confirm_dialog = Some(dialog);
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

fn render_content_with_panel(frame: &mut ratatui::Frame, area: Rect, app: &mut App) {
    let chunks =
        Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)]).split(area);

    match app.tab {
        Tab::ScanResults => render_scan_list(frame, chunks[0], app),
        Tab::Browse => {
            frame.render_widget(&app.miller, chunks[0]);
        }
    }

    render_selected_panel(frame, chunks[1], app);
}

fn render_scan_list(frame: &mut ratatui::Frame, area: Rect, app: &mut App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Scan Results ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let visible = inner.height as usize;
    let (scroll, end) = app.visible_range(visible);
    app.scan_scroll = scroll;

    let indices: Vec<usize> = if app.is_filter_active() {
        app.search
            .matches()
            .iter()
            .map(|m| m.index)
            .skip(scroll)
            .take(end - scroll)
            .collect()
    } else {
        (scroll..end).collect()
    };

    let items: Vec<ListItem> = indices
        .iter()
        .enumerate()
        .map(|(vi, &i)| {
            let item = &app.scan_items[i];
            let selected = app.selected_indices.contains(&i);
            let cursor = if app.is_filter_active() {
                vi == app.search.cursor().saturating_sub(scroll)
            } else {
                i == app.scan_cursor
            };

            let check = if selected { "✓" } else { " " };
            let type_marker = match item.item_type {
                ItemType::Dir => "Dir ",
                ItemType::File => "File",
            };

            let short_path = item.path.parent().and_then(|p| p.to_str()).unwrap_or("");

            let line = Line::from(vec![
                Span::styled(
                    format!(" {check} "),
                    if selected {
                        Style::default()
                            .fg(Color::Green)
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
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(line).style(style)
        })
        .collect();

    let list = List::new(items);
    let mut state = ListState::default();
    let select_idx = if app.scan_items.is_empty() {
        None
    } else if app.is_filter_active() {
        let rel_cursor = app.search.cursor().saturating_sub(scroll);
        Some(rel_cursor)
    } else {
        Some(app.scan_cursor.saturating_sub(scroll))
    };
    state.select(select_idx);
    frame.render_stateful_widget(list, inner, &mut state);
}

fn render_selected_panel(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let count = app.selected_count();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(format!(" Managed ({}) ", count));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let items = app.selected_items();
    if items.is_empty() {
        if inner.height > 0 && inner.width > 4 {
            let hint = Span::styled(" (no apps selected) ", Style::default().fg(Color::DarkGray));
            frame.render_widget(ratatui::widgets::Paragraph::new(Line::from(hint)), inner);
        }
        return;
    }

    let visible = inner.height as usize;
    let lines: Vec<Line> = items
        .iter()
        .take(visible)
        .map(|item| {
            Line::from(vec![Span::styled(
                format!(" ● {}", truncate_str(&item.name, inner.width as usize - 3)),
                Style::default().fg(Color::White),
            )])
        })
        .collect();

    let paragraph = ratatui::widgets::Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

fn render_help_overlay(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let popup_width = 50u16.min(area.width.saturating_sub(4)).max(20);
    let popup_height = 14u16.min(area.height.saturating_sub(4)).max(6);
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Help ");
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let bindings = app.help_bindings();
    let filtered: Vec<(usize, &(&str, &str))> = if app.help_search.query().is_empty() {
        bindings.iter().enumerate().collect()
    } else {
        app.help_search
            .matches()
            .iter()
            .map(|m| (m.index, &bindings[m.index]))
            .collect()
    };

    let visible = inner.height as usize;
    let scroll = app.help_scroll.min(filtered.len().saturating_sub(visible));
    let end = (scroll + visible).min(filtered.len());

    let lines: Vec<Line> = filtered[scroll..end]
        .iter()
        .map(|(_, (key, desc))| {
            Line::from(vec![
                Span::styled(
                    format!("{:<10}", key),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(*desc, Style::default().fg(Color::White)),
            ])
        })
        .collect();

    frame.render_widget(ratatui::widgets::Paragraph::new(lines), inner);

    // Render search hint if active
    if !app.help_search.query().is_empty() {
        let query = app.help_search.query();
        let match_count = app.help_search.match_count();
        let hint = if match_count == 0 {
            "no matches".to_string()
        } else if match_count == 1 {
            "1 match".to_string()
        } else {
            format!("{} matches", match_count)
        };
        let hint_line = Line::from(vec![
            Span::styled("› ", Style::default().fg(Color::Yellow)),
            Span::styled(query, Style::default().fg(Color::White)),
            Span::styled(format!("  {}", hint), Style::default().fg(Color::DarkGray)),
        ]);
        let hint_area = Rect::new(
            inner.x,
            inner.y + inner.height.saturating_sub(1),
            inner.width,
            1,
        );
        frame.render_widget(ratatui::widgets::Paragraph::new(hint_line), hint_area);
    }
}

fn render_search_overlay(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let popup_width = 40u16.min(area.width.saturating_sub(4)).max(20);
    let popup_height = 3u16.clamp(3, area.height.saturating_sub(4));
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Search ");
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let query = app.search.query();
    let match_count = app.search.match_count();
    let hint = if query.is_empty() {
        "all items".to_string()
    } else if match_count == 0 {
        "no matches".to_string()
    } else if match_count == 1 {
        "1 match".to_string()
    } else {
        format!("{} matches", match_count)
    };

    let line = Line::from(vec![
        Span::styled("› ", Style::default().fg(Color::Yellow)),
        Span::styled(query, Style::default().fg(Color::White)),
        Span::styled("_", Style::default().fg(Color::Yellow)),
    ]);
    frame.render_widget(
        ratatui::widgets::Paragraph::new(line),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );

    if inner.height > 1 {
        let hint_line = Line::from(Span::styled(
            format!("  {}", hint),
            Style::default().fg(Color::DarkGray),
        ));
        frame.render_widget(
            ratatui::widgets::Paragraph::new(hint_line),
            Rect::new(inner.x, inner.y + 1, inner.width, 1),
        );
    }
}

fn render_status_bar(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let filter_info = if app.is_filter_active() {
        let query = app.search.query();
        let count = app.search.match_count();
        let hint = if count == 0 {
            "no matches".to_string()
        } else if count == 1 {
            "1 match".to_string()
        } else {
            format!("{} matches", count)
        };
        Some((query, hint))
    } else {
        None
    };

    let line = if app.in_search {
        if let Some((query, hint)) = filter_info {
            Line::from(vec![
                Span::styled("Fuzzy Search (/) ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("[filter: \"{}\" | {}]", query, hint),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    "  ↑/↓ nav  Enter/Esc close",
                    Style::default().fg(Color::DarkGray),
                ),
            ])
        } else {
            Line::from(vec![
                Span::styled("Fuzzy Search (/) ", Style::default().fg(Color::Yellow)),
                Span::styled("[type to filter]", Style::default().fg(Color::White)),
                Span::styled("  Esc close", Style::default().fg(Color::DarkGray)),
            ])
        }
    } else if let Some((query, hint)) = filter_info {
        Line::from(vec![
            Span::styled("Fuzzy Search (/) ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("[filter: \"{}\" | {}]", query, hint),
                Style::default().fg(Color::White),
            ),
            Span::styled("  / edit filter", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        let hints = vec![
            ("?", "help"),
            ("j/k", "nav"),
            ("Tab", "focus"),
            ("/", "search"),
            ("Space", "select"),
            ("Enter", "confirm"),
            ("Esc", "cancel"),
        ];
        let spans: Vec<Span> = hints
            .into_iter()
            .enumerate()
            .flat_map(|(i, (key, desc))| {
                let mut s = vec![
                    Span::styled(key, Style::default().fg(Color::Yellow)),
                    Span::styled(format!(" {}", desc), Style::default().fg(Color::DarkGray)),
                ];
                if i > 0 {
                    s.insert(0, Span::styled("  ", Style::default().fg(Color::DarkGray)));
                }
                s
            })
            .collect();
        Line::from(spans)
    };
    frame.render_widget(line, area);
}

fn confidence_style(confidence: u32) -> Style {
    if confidence >= 150 {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else if confidence >= 100 {
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
