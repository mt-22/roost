use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::tui::confirm::render_confirm_dialog;
use crate::tui::main_view::state::{Focus, MainViewState, SearchTarget};

/// Render the complete main view.
pub fn render(state: &mut MainViewState, frame: &mut Frame) {
    let area = frame.area();

    let vertical = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Min(1),   // main content
        Constraint::Length(1), // status bar
    ])
    .split(area);

    render_header(state, frame, vertical[0]);
    render_main(state, frame, vertical[1]);
    render_status_bar(state, frame, vertical[2]);

    // Overlays (drawn last so they appear on top)
    if let Some(ref search) = state.search {
        render_search_overlay(state, frame, search);
    }
    if let Some(ref dialog) = state.confirm_dialog {
        render_confirm_dialog(frame, dialog);
    }
}

// ------------------------------------------------------------------
// Header
// ------------------------------------------------------------------

fn render_header(state: &MainViewState, frame: &mut Frame, area: Rect) {
    let line = Line::from(vec![
        Span::styled("roost", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!(" · profile: {}  {} app{} managed", state.active_profile_name(), state.app_count(), if state.app_count() == 1 { "" } else { "s" }),
            Style::default().fg(Color::White),
        ),
    ]);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

// ------------------------------------------------------------------
// Main content: left apps panel + right miller panel
// ------------------------------------------------------------------

fn render_main(state: &mut MainViewState, frame: &mut Frame, area: Rect) {
    let horizontal =
        Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)]).split(area);

    render_apps_panel(state, frame, horizontal[0]);
    render_files_panel(state, frame, horizontal[1]);
}

fn render_apps_panel(state: &mut MainViewState, frame: &mut Frame, area: Rect) {
    let border_color = if state.focus == Focus::AppsPanel {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(" Apps ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let visible = inner.height as usize;
    let (scroll, end) = state.scroll_for_visible(visible);

    let apps = state.apps_in_active_profile();
    let items: Vec<ListItem> = apps
        .iter()
        .enumerate()
        .skip(scroll)
        .take(end - scroll)
        .map(|(i, name)| {
            let is_cursor = i == state.app_cursor;
            let has_primary = state.has_primary_config(name);
            let source = state
                .shared
                .profiles
                .get(state.active_profile_name())
                .and_then(|p| p.app_sources.get(*name));

            let marker = if has_primary {
                "★ "
            } else if source.is_some() {
                "← "
            } else {
                "  "
            };

            let cursor_prefix = if is_cursor { "» " } else { "  " };

            let name_style = if is_cursor {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let line = Line::from(vec![
                Span::styled(cursor_prefix, Style::default().fg(Color::Yellow)),
                Span::styled(marker, Style::default().fg(Color::Cyan)),
                Span::styled(
                    truncate_str(name, inner.width as usize - 6),
                    name_style,
                ),
            ]);

            let style = if is_cursor {
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
    let mut list_state = ListState::default();
    let cursor_in_view = state.app_cursor.saturating_sub(scroll);
    if !apps.is_empty() {
        list_state.select(Some(cursor_in_view));
    }
    frame.render_stateful_widget(list, inner, &mut list_state);
}

fn render_files_panel(state: &mut MainViewState, frame: &mut Frame, area: Rect) {
    let header_color = if state.focus == Focus::FilesPanel {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let vertical = Layout::vertical([
        Constraint::Length(1), // " Files " header
        Constraint::Min(1),   // miller columns
    ])
    .split(area);

    let header_line = Line::from(vec![
        Span::styled(" Files ", Style::default().fg(header_color).add_modifier(Modifier::BOLD)),
    ]);
    frame.render_widget(Paragraph::new(header_line), vertical[0]);

    if vertical[1].width == 0 || vertical[1].height == 0 {
        return;
    }

    frame.render_widget(&state.miller, vertical[1]);
}

// ------------------------------------------------------------------
// Status bar
// ------------------------------------------------------------------

fn render_status_bar(state: &MainViewState, frame: &mut Frame, area: Rect) {
    let parts = if let Some(ref msg) = state.status_message {
        vec![Span::styled(msg.clone(), Style::default().fg(Color::Yellow))]
    } else {
        let base = vec![
            Span::styled("j", key_style()),
            Span::styled("/", Style::default().fg(Color::DarkGray)),
            Span::styled("k", key_style()),
            Span::styled(" nav  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Tab", key_style()),
            Span::styled(" focus  ", Style::default().fg(Color::DarkGray)),
            Span::styled("h", key_style()),
            Span::styled("/", Style::default().fg(Color::DarkGray)),
            Span::styled("l", key_style()),
            Span::styled(" miller  ", Style::default().fg(Color::DarkGray)),
            Span::styled("/", key_style()),
            Span::styled(" search  ", Style::default().fg(Color::DarkGray)),
            Span::styled("?", key_style()),
            Span::styled(" help  ", Style::default().fg(Color::DarkGray)),
            Span::styled("q", key_style()),
            Span::styled(" quit", Style::default().fg(Color::DarkGray)),
        ];

        let focused = match state.focus {
            Focus::AppsPanel => vec![
                Span::styled("  ·  ", Style::default().fg(Color::DarkGray)),
                Span::styled("o", key_style()),
                Span::styled(" open  ", Style::default().fg(Color::DarkGray)),
                Span::styled("x", key_style()),
                Span::styled(" remove  ", Style::default().fg(Color::DarkGray)),
                Span::styled("f", key_style()),
                Span::styled(" link-from  ", Style::default().fg(Color::DarkGray)),
                Span::styled("m", key_style()),
                Span::styled(" paste", Style::default().fg(Color::DarkGray)),
            ],
            Focus::FilesPanel => vec![
                Span::styled("  ·  ", Style::default().fg(Color::DarkGray)),
                Span::styled("e", key_style()),
                Span::styled("/", Style::default().fg(Color::DarkGray)),
                Span::styled("Enter", key_style()),
                Span::styled(" edit  ", Style::default().fg(Color::DarkGray)),
                Span::styled("p", key_style()),
                Span::styled(" set-primary", Style::default().fg(Color::DarkGray)),
            ],
        };

        let actions = vec![
            Span::styled("  ·  ", Style::default().fg(Color::DarkGray)),
            Span::styled("s", key_style()),
            Span::styled(" sync  ", Style::default().fg(Color::DarkGray)),
            Span::styled("a", key_style()),
            Span::styled(" add  ", Style::default().fg(Color::DarkGray)),
            Span::styled("i", key_style()),
            Span::styled(" ignore  ", Style::default().fg(Color::DarkGray)),
            Span::styled("P", key_style()),
            Span::styled(" profile  ", Style::default().fg(Color::DarkGray)),
            Span::styled("g", key_style()),
            Span::styled(" log  ", Style::default().fg(Color::DarkGray)),
            Span::styled("d", key_style()),
            Span::styled(" diff  ", Style::default().fg(Color::DarkGray)),
            Span::styled("u", key_style()),
            Span::styled(" undo", Style::default().fg(Color::DarkGray)),
        ];

        let mut all = base;
        all.extend(focused);
        all.extend(actions);
        all
    };

    let line = Line::from(parts);
    frame.render_widget(Paragraph::new(line), area);
}

fn key_style() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

// ------------------------------------------------------------------
// Search overlay
// ------------------------------------------------------------------

fn render_search_overlay(_state: &MainViewState, frame: &mut Frame, search: &crate::tui::main_view::state::SearchState) {
    let area = frame.area();
    let popup_width = 40u16.min(area.width.saturating_sub(4)).max(20);
    let popup_height = 3u16.min(area.height.saturating_sub(4)).max(3);
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(format!(
            " Search {} ",
            match search.target {
                SearchTarget::Apps => "Apps",
                SearchTarget::Files => "Files",
            }
        ));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let match_count = search.engine.match_count();
    let hint = if search.query.is_empty() {
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
        Span::styled(&search.query, Style::default().fg(Color::White)),
        Span::styled("_", Style::default().fg(Color::Yellow)),
    ]);
    frame.render_widget(
        Paragraph::new(line),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );

    if inner.height > 1 {
        let hint_line = Line::from(Span::styled(
            format!("  {}", hint),
            Style::default().fg(Color::DarkGray),
        ));
        frame.render_widget(
            Paragraph::new(hint_line),
            Rect::new(inner.x, inner.y + 1, inner.width, 1),
        );
    }
}

// ------------------------------------------------------------------
// Helpers
// ------------------------------------------------------------------

fn truncate_str(s: &str, max_width: usize) -> String {
    if s.chars().count() <= max_width {
        s.to_string()
    } else {
        s.chars().take(max_width.saturating_sub(1)).collect::<String>() + "…"
    }
}
