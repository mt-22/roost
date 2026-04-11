use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs},
    Frame,
};

use super::state::{OnboardingTui, Tab};
use crate::scanner::{self, SourceEntry};

pub fn render(state: &mut OnboardingTui, frame: &mut Frame) {
    let outer = Layout::vertical([
        Constraint::Length(3), // header + tabs
        Constraint::Min(0),    // body
        Constraint::Length(1), // status bar
    ])
    .split(frame.area());

    render_header(state, frame, outer[0]);
    render_body(state, frame, outer[1]);
    render_status_bar(state, frame, outer[2]);

    if state.search.is_some() {
        render_search_overlay(state, frame);
    }
}

fn render_header(state: &OnboardingTui, frame: &mut Frame, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // title
        Constraint::Length(2), // tabs
    ])
    .split(area);

    frame.render_widget(
        Paragraph::new(" roost setup").style(Style::default().add_modifier(Modifier::BOLD)),
        chunks[0],
    );

    let labels: Vec<Line> = state.tab_labels().into_iter().map(Line::from).collect();
    let tabs = Tabs::new(labels)
        .select(state.active_tab_index())
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .divider("│");

    frame.render_widget(tabs, chunks[1]);
}

fn render_body(state: &mut OnboardingTui, frame: &mut Frame, area: Rect) {
    let chunks =
        Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)]).split(area);

    match &state.active_tab {
        Tab::Source(i) => {
            let i = *i;
            render_source_tab(state, i, frame, chunks[0]);
        }
        Tab::Browse => {
            render_browse_tab(state, frame, chunks[0]);
        }
    }

    render_selection_panel(state, frame, chunks[1]);
}

fn render_source_tab(state: &mut OnboardingTui, tab_idx: usize, frame: &mut Frame, area: Rect) {
    let tab = &state.tabs[tab_idx];

    let items: Vec<ListItem> = tab
        .entries
        .iter()
        .map(|entry| {
            let marker = if state.is_selected(&entry.path) {
                "✓ "
            } else {
                "  "
            };
            let suffix = if entry.path.is_dir() { "/" } else { "" };
            ListItem::new(format!("{}{}{}", marker, entry.name, suffix))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(tab.label.clone()),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("» ");

    let tab = &mut state.tabs[tab_idx];
    frame.render_stateful_widget(list, area, &mut tab.list_state);
}

fn render_browse_tab(state: &mut OnboardingTui, frame: &mut Frame, area: Rect) {
    let cols = Layout::horizontal([
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
    ])
    .split(area);

    let miller = &state.miller;

    // Parent column
    if let Some(parent_entries) = miller.parent_listing() {
        let parent_cursor = miller.parent_cursor().unwrap_or(0);
        render_miller_column(
            parent_entries,
            parent_cursor,
            "Parent",
            state,
            frame,
            cols[0],
        );
    } else {
        frame.render_widget(
            Block::default().borders(Borders::ALL).title("Parent"),
            cols[0],
        );
    }

    // Current column
    let current_entries = miller.current_listing();
    let current_cursor = miller.current_cursor();
    let current_title = miller
        .current_dir()
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| miller.current_dir().display().to_string());
    render_miller_column(
        current_entries,
        current_cursor,
        &current_title,
        state,
        frame,
        cols[1],
    );

    // Preview column
    let preview: Vec<SourceEntry> = match miller.current_entry() {
        Some(entry) if entry.path.is_dir() => {
            scanner::scan_source(&entry.path, &state.context.ignored, false).unwrap_or_default()
        }
        _ => Vec::new(),
    };
    if !preview.is_empty() {
        render_miller_column(&preview, 0, "Preview", state, frame, cols[2]);
    } else {
        let info = match miller.current_entry() {
            Some(entry) if !entry.path.is_dir() => format!("  {}", entry.name),
            _ => String::new(),
        };
        frame.render_widget(
            Paragraph::new(info).block(Block::default().borders(Borders::ALL).title("Preview")),
            cols[2],
        );
    }
}

fn render_miller_column(
    entries: &[SourceEntry],
    cursor: usize,
    title: &str,
    state: &OnboardingTui,
    frame: &mut Frame,
    area: Rect,
) {
    let items: Vec<ListItem> = entries
        .iter()
        .map(|entry| {
            let marker = if state.is_selected(&entry.path) {
                "✓ "
            } else {
                "  "
            };
            let suffix = if entry.path.is_dir() { "/" } else { "" };
            ListItem::new(format!("{}{}{}", marker, entry.name, suffix))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title.to_string()),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("» ");

    let mut list_state = ListState::default();
    if !entries.is_empty() {
        list_state.select(Some(cursor));
    }
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_selection_panel(state: &OnboardingTui, frame: &mut Frame, area: Rect) {
    let items: Vec<ListItem> = state
        .selected
        .iter()
        .map(|entry| {
            let suffix = if entry.path.is_dir() { "/" } else { "" };
            ListItem::new(format!("● {}{}", entry.name, suffix))
        })
        .collect();

    let title = format!("Managed ({})", state.selected.len());

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(title));

    frame.render_widget(list, area);
}

fn render_search_overlay(state: &OnboardingTui, frame: &mut Frame) {
    if let Some(ref search) = state.search {
        super::search::render_search_overlay(search, frame);
    }
}

fn render_status_bar(state: &OnboardingTui, frame: &mut Frame, area: Rect) {
    let key = |k: &str| Span::styled(format!(" {} ", k), Style::default().fg(Color::Yellow));
    let label = |l: &str| Span::raw(format!("{}  ", l));

    let mut spans = vec![
        key("j/k"),
        label("navigate"),
        key("␣"),
        label("select"),
        key("Tab"),
        label("next tab"),
        key("/"),
        label("search"),
    ];

    if state.active_tab == Tab::Browse {
        spans.push(key("h/l"));
        spans.push(label("in/out"));
    }

    spans.push(key("w"));
    spans.push(label("done"));
    spans.push(key("q"));
    spans.push(label("quit"));

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
