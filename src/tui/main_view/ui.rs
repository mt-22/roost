use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::tui::confirm::render_confirm_dialog;
use crate::tui::main_view::state::{Focus, MainViewState, SearchTarget};

/// Render the complete main view.
pub fn render(state: &mut MainViewState, frame: &mut Frame) {
    let area = frame.area();

    let vertical = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Min(1),    // main content
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
    if let Some(ref help) = state.help_dialog {
        render_help_dialog(frame, help);
    }
    if let Some(ref profile) = state.profile_dialog {
        render_profile_dialog(frame, profile, state);
    }
    if let Some(ref ignore) = state.ignore_dialog {
        render_ignore_dialog(frame, ignore, state);
    }
    if let Some(ref git_log) = state.git_log_dialog {
        render_git_log_dialog(frame, git_log);
    }
    if let Some(ref undo) = state.undo_dialog {
        render_undo_dialog(frame, undo);
    }
    if let Some(ref app_link) = state.app_link_dialog {
        render_app_link_dialog(frame, app_link, state);
    }
    if let Some(ref diff) = state.diff_view {
        render_diff_view_dialog(frame, diff);
    }
}

// ------------------------------------------------------------------
// Header
// ------------------------------------------------------------------

fn render_header(state: &MainViewState, frame: &mut Frame, area: Rect) {
    let line = Line::from(vec![
        Span::styled(
            "roost",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                " · profile: {}  {} app{} managed",
                state.active_profile_name(),
                state.app_count(),
                if state.app_count() == 1 { "" } else { "s" }
            ),
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
            let source = state
                .shared
                .profiles
                .get(state.active_profile_name())
                .and_then(|p| p.app_sources.get(*name));

            if source.is_some() {
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
                Span::styled(truncate_str(name, inner.width as usize - 6), name_style),
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
        Constraint::Min(1),    // miller columns
    ])
    .split(area);

    let header_line = Line::from(vec![Span::styled(
        " Files ",
        Style::default()
            .fg(header_color)
            .add_modifier(Modifier::BOLD),
    )]);
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
        vec![Span::styled(
            msg.clone(),
            Style::default().fg(Color::Yellow),
        )]
    } else {
        let base = vec![
            Span::styled("?", key_style()),
            Span::styled(" help  ", Style::default().fg(Color::DarkGray)),
            Span::styled("j", key_style()),
            Span::styled("/", Style::default().fg(Color::DarkGray)),
            Span::styled("k", key_style()),
            Span::styled(" nav  ", Style::default().fg(Color::DarkGray)),
            Span::styled("h", key_style()),
            Span::styled("/", Style::default().fg(Color::DarkGray)),
            Span::styled("l", key_style()),
            Span::styled(" miller  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Tab", key_style()),
            Span::styled(" focus  ", Style::default().fg(Color::DarkGray)),
            Span::styled("/", key_style()),
            Span::styled(" search  ", Style::default().fg(Color::DarkGray)),
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
            Span::styled(" save ", Style::default().fg(Color::DarkGray)),
            Span::styled("S", key_style()),
            Span::styled(" sync ", Style::default().fg(Color::DarkGray)),
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

fn render_search_overlay(
    _state: &MainViewState,
    frame: &mut Frame,
    search: &crate::tui::main_view::state::SearchState,
) {
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

fn render_help_dialog(frame: &mut Frame, help: &crate::tui::main_view::dialogs::HelpState) {
    let area = frame.area();
    let width = 72u16.min(area.width.saturating_sub(4)).max(40);
    let height = 24u16.min(area.height.saturating_sub(4)).max(10);
    let popup_x = (area.width.saturating_sub(width)) / 2;
    let popup_y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, width, height);

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

    let visible = inner.height as usize;
    let total = crate::tui::main_view::dialogs::KEYBINDS.len();
    let (scroll, end) = help.scroll_for_visible(visible, total);

    let items: Vec<ListItem> = crate::tui::main_view::dialogs::KEYBINDS
        .iter()
        .enumerate()
        .skip(scroll)
        .take(end - scroll)
        .map(|(i, entry)| {
            let is_cursor = i == help.cursor;
            let line = Line::from(vec![
                Span::styled(
                    format!("{:>12}", entry.key),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("  ", Style::default()),
                Span::styled(
                    truncate_str(entry.description, inner.width as usize - 14),
                    Style::default().fg(Color::White),
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
    let cursor_in_view = help.cursor.saturating_sub(scroll);
    list_state.select(Some(cursor_in_view));
    frame.render_stateful_widget(list, inner, &mut list_state);
}

fn render_profile_dialog(
    frame: &mut Frame,
    profile: &crate::tui::main_view::dialogs::ProfileState,
    state: &MainViewState,
) {
    use crate::tui::main_view::dialogs::ProfileMode;

    let area = frame.area();
    let width = 50u16.min(area.width.saturating_sub(4)).max(30);
    let height = 20u16.min(area.height.saturating_sub(4)).max(8);
    let popup_x = (area.width.saturating_sub(width)) / 2;
    let popup_y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, width, height);

    frame.render_widget(Clear, popup_area);

    let border_color = match profile.mode {
        ProfileMode::Delete => Color::Red,
        _ => Color::Yellow,
    };

    let mode_title = match profile.mode {
        ProfileMode::Switch => "Switch",
        ProfileMode::Create => "Create",
        ProfileMode::Delete => "Delete",
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(format!(" Profile — {} ", mode_title));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    // Split inner area into content + footer hint
    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);
    let content_area = chunks[0];
    let hint_area = chunks[1];

    // Footer hint showing all modes with current one highlighted
    let switch_style = if profile.mode == ProfileMode::Switch {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let create_style = if profile.mode == ProfileMode::Create {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let delete_style = if profile.mode == ProfileMode::Delete {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let hint_line = Line::from(vec![
        Span::styled("Switch", switch_style),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled("Create", create_style),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled("Delete", delete_style),
        Span::styled("  — Tab to cycle", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(
        Paragraph::new(hint_line).alignment(ratatui::layout::Alignment::Center),
        hint_area,
    );

    match profile.mode {
        ProfileMode::Switch | ProfileMode::Delete => {
            let mut names: Vec<&String> = state.shared.profiles.keys().collect();
            names.sort();
            let visible = content_area.height as usize;
            let total = names.len();
            let (scroll, end) = profile.scroll_for_visible(visible, total);

            let items: Vec<ListItem> = names
                .iter()
                .enumerate()
                .skip(scroll)
                .take(end - scroll)
                .map(|(i, name)| {
                    let is_cursor = i == profile.cursor;
                    let is_active = **name == state.local.active_profile;
                    let marker = if is_active { "★ " } else { "  " };
                    let line = Line::from(vec![
                        Span::styled(marker, Style::default().fg(Color::Green)),
                        Span::styled(
                            truncate_str(name, content_area.width as usize - 4),
                            Style::default().fg(Color::White),
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
            let cursor_in_view = profile.cursor.saturating_sub(scroll);
            list_state.select(Some(cursor_in_view));
            frame.render_stateful_widget(list, content_area, &mut list_state);
        }
        ProfileMode::Create => {
            let chunks = Layout::vertical([
                Constraint::Length(1), // name input
                Constraint::Length(1), // copy current toggle
                Constraint::Min(1),    // space
            ])
            .split(content_area);

            let input_line = Line::from(vec![
                Span::styled("Name: ", Style::default().fg(Color::Yellow)),
                Span::styled(&profile.input, Style::default().fg(Color::White)),
                Span::styled("_", Style::default().fg(Color::Yellow)),
            ]);
            frame.render_widget(Paragraph::new(input_line), chunks[0]);

            let toggle = if profile.copy_current {
                "[x] Copy apps from current profile"
            } else {
                "[ ] Start empty"
            };
            let toggle_line = Line::from(vec![
                Span::styled(
                    "Space ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(toggle, Style::default().fg(Color::White)),
            ]);
            frame.render_widget(Paragraph::new(toggle_line), chunks[1]);

            let hint = Line::from(vec![Span::styled(
                "Tab: cycle modes  Enter: create",
                Style::default().fg(Color::DarkGray),
            )]);
            frame.render_widget(Paragraph::new(hint), chunks[2]);
        }
    }
}

fn render_ignore_dialog(
    frame: &mut Frame,
    ignore: &crate::tui::main_view::dialogs::IgnoreState,
    state: &MainViewState,
) {
    use crate::tui::main_view::dialogs::IgnoreMode;

    let area = frame.area();
    let width = 60u16.min(area.width.saturating_sub(4)).max(30);
    let height = 20u16.min(area.height.saturating_sub(4)).max(8);
    let popup_x = (area.width.saturating_sub(width)) / 2;
    let popup_y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, width, height);

    frame.render_widget(Clear, popup_area);

    let mode_title = match ignore.mode {
        IgnoreMode::Add => "Add",
        IgnoreMode::Remove => "Remove",
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(format!(" Ignore — {} ", mode_title));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    match ignore.mode {
        IgnoreMode::Add => {
            let chunks = Layout::vertical([
                Constraint::Length(1), // input
                Constraint::Min(1),    // hint
            ])
            .split(inner);

            let input_line = Line::from(vec![
                Span::styled("Pattern: ", Style::default().fg(Color::Yellow)),
                Span::styled(&ignore.input, Style::default().fg(Color::White)),
                Span::styled("_", Style::default().fg(Color::Yellow)),
            ]);
            frame.render_widget(Paragraph::new(input_line), chunks[0]);

            let hint = Line::from(vec![Span::styled(
                "Tab: cycle modes  Enter: add  Esc: close",
                Style::default().fg(Color::DarkGray),
            )]);
            frame.render_widget(Paragraph::new(hint), chunks[1]);
        }
        IgnoreMode::Remove => {
            let mut patterns: Vec<&String> = state.shared.ignored.iter().collect();
            patterns.sort();
            let visible = inner.height as usize;
            let total = patterns.len();
            let (scroll, end) = ignore.scroll_for_visible(visible, total);

            let items: Vec<ListItem> = patterns
                .iter()
                .enumerate()
                .skip(scroll)
                .take(end - scroll)
                .map(|(i, pat)| {
                    let is_cursor = i == ignore.cursor;
                    let line = Line::from(vec![Span::styled(
                        truncate_str(pat, inner.width as usize - 4),
                        Style::default().fg(Color::White),
                    )]);
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
            let cursor_in_view = ignore.cursor.saturating_sub(scroll);
            list_state.select(Some(cursor_in_view));
            frame.render_stateful_widget(list, inner, &mut list_state);
        }
    }
}

fn render_git_log_dialog(frame: &mut Frame, git_log: &crate::tui::main_view::dialogs::GitLogState) {
    let area = frame.area();
    let width = 58u16.min(area.width.saturating_sub(4)).max(40);
    let height = 20u16.min(area.height.saturating_sub(4)).max(10);
    let popup_x = (area.width.saturating_sub(width)) / 2;
    let popup_y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, width, height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Git Log ");
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let visible = inner.height as usize;
    let _total = git_log.commits.len();
    let (scroll, end) = git_log.scroll_for_visible(visible);

    let items: Vec<ListItem> = git_log
        .commits
        .iter()
        .enumerate()
        .skip(scroll)
        .take(end - scroll)
        .map(|(i, commit)| {
            let is_cursor = i == git_log.cursor;
            let short_hash = &commit.hash[..commit.hash.len().min(7)];
            let line = Line::from(vec![
                Span::styled(
                    format!("{:<7} ", short_hash),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    truncate_str(&commit.message, inner.width as usize - 10),
                    Style::default().fg(Color::White),
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
    let cursor_in_view = git_log.cursor.saturating_sub(scroll);
    list_state.select(Some(cursor_in_view));
    frame.render_stateful_widget(list, inner, &mut list_state);
}

fn render_undo_dialog(frame: &mut Frame, undo: &crate::tui::main_view::dialogs::UndoState) {
    let area = frame.area();
    let width = 50u16.min(area.width.saturating_sub(4)).max(30);
    let popup_x = (area.width.saturating_sub(width)) / 2;

    let _text_width = (width as usize).saturating_sub(4);
    let lines_needed = undo.message.lines().count().max(1);
    let height = (lines_needed as u16 + 5)
        .min(area.height.saturating_sub(4))
        .max(6);
    let popup_y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, width, height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .title(" Undo ");
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);

    let message_lines: Vec<Line> = undo
        .message
        .lines()
        .map(|line| {
            Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(Color::White),
            ))
        })
        .collect();
    let message_para = Paragraph::new(message_lines);
    frame.render_widget(message_para, chunks[0]);

    let buttons = Line::from(vec![
        Span::styled(
            "y",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" / ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "n",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    let buttons_para = Paragraph::new(buttons).alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(buttons_para, chunks[1]);
}

fn render_app_link_dialog(
    frame: &mut Frame,
    app_link: &crate::tui::main_view::dialogs::AppLinkState,
    state: &MainViewState,
) {
    use crate::tui::main_view::dialogs::{AppLinkAction, AppLinkStep};

    let area = frame.area();
    let width = 60u16.min(area.width.saturating_sub(4)).max(30);
    let height = 20u16.min(area.height.saturating_sub(4)).max(8);
    let popup_x = (area.width.saturating_sub(width)) / 2;
    let popup_y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, width, height);

    frame.render_widget(Clear, popup_area);

    let border_color = if app_link.action == AppLinkAction::Import {
        Color::Cyan
    } else {
        Color::Yellow
    };

    let action_label = if app_link.action == AppLinkAction::Import {
        "Import From"
    } else {
        "Paste Into"
    };

    let step_label = match app_link.step {
        AppLinkStep::PickProfile => "Pick Profile",
        AppLinkStep::PickApp => "Pick App",
        AppLinkStep::ConfirmCopy => "Confirm",
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(format!(" {} — {} ", action_label, step_label));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    match app_link.step {
        AppLinkStep::PickProfile => {
            let mut names: Vec<&String> = state.shared.profiles.keys().collect();
            names.sort();
            let visible = inner.height as usize;
            let total = names.len();
            let (scroll, end) = app_link.scroll_for_visible(visible, total);

            let items: Vec<ListItem> = names
                .iter()
                .enumerate()
                .skip(scroll)
                .take(end - scroll)
                .map(|(i, name)| {
                    let is_cursor = i == app_link.cursor;
                    let is_active = **name == state.local.active_profile;
                    let marker = if is_active { "★ " } else { "  " };
                    let line = Line::from(vec![
                        Span::styled(marker, Style::default().fg(Color::Green)),
                        Span::styled(
                            truncate_str(name, inner.width as usize - 4),
                            Style::default().fg(Color::White),
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
            let cursor_in_view = app_link.cursor.saturating_sub(scroll);
            list_state.select(Some(cursor_in_view));
            frame.render_stateful_widget(list, inner, &mut list_state);
        }
        AppLinkStep::PickApp => {
            if let Some(ref profile) = app_link.selected_profile {
                let mut apps: Vec<&String> = state
                    .shared
                    .profiles
                    .get(profile)
                    .map(|p| p.apps.iter().collect())
                    .unwrap_or_default();
                apps.sort();
                let visible = inner.height as usize;
                let total = apps.len();
                let (scroll, end) = app_link.scroll_for_visible(visible, total);

                let items: Vec<ListItem> = apps
                    .iter()
                    .enumerate()
                    .skip(scroll)
                    .take(end - scroll)
                    .map(|(i, app)| {
                        let is_cursor = i == app_link.cursor;
                        let line = Line::from(vec![Span::styled(
                            truncate_str(app, inner.width as usize - 4),
                            Style::default().fg(Color::White),
                        )]);
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
                let cursor_in_view = app_link.cursor.saturating_sub(scroll);
                list_state.select(Some(cursor_in_view));
                frame.render_stateful_widget(list, inner, &mut list_state);
            }
        }
        AppLinkStep::ConfirmCopy => {
            if let Some(ref profile) = app_link.selected_profile {
                if let Some(app) = state.selected_app() {
                    let confirm_text = format!("Copy '{}' to profile '{}' ?", app, profile);
                    let line = Line::from(vec![Span::styled(
                        confirm_text,
                        Style::default().fg(Color::White),
                    )]);
                    frame.render_widget(Paragraph::new(line), inner);
                }
            }
        }
    }
}

fn render_diff_view_dialog(
    frame: &mut Frame,
    diff: &crate::tui::main_view::dialogs::DiffViewState,
) {
    let area = frame.area();
    let width = 72u16.min(area.width.saturating_sub(4)).max(40);
    let height = (area.height as f32 * 0.8) as u16;
    let popup_x = (area.width.saturating_sub(width)) / 2;
    let popup_y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, width, height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Diff ");
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let visible = inner.height as usize;
    let end = (diff.scroll + visible).min(diff.lines.len());

    let lines: Vec<Line> = diff
        .lines
        .iter()
        .skip(diff.scroll)
        .take(end - diff.scroll)
        .map(|line| {
            let color = if line.starts_with('+') {
                Color::Green
            } else if line.starts_with('-') {
                Color::Red
            } else if line.starts_with("@@") {
                Color::Cyan
            } else {
                Color::White
            };
            Line::from(Span::styled(
                truncate_str(line, inner.width as usize),
                Style::default().fg(color),
            ))
        })
        .collect();

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

fn truncate_str(s: &str, max_width: usize) -> String {
    if s.chars().count() <= max_width {
        s.to_string()
    } else {
        s.chars()
            .take(max_width.saturating_sub(1))
            .collect::<String>()
            + "…"
    }
}
