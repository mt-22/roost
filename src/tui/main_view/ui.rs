use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use super::dialogs::app_link::{AppLinkMode, LinkFromStep};
use super::dialogs::help::HelpFocus;
use super::state::{ConfirmKind, FileEntry, IgnoreMode, MainViewTui, PanelFocus, ProfileMode};
use crate::tui::state::MillerEntry;

pub fn render(state: &mut MainViewTui, frame: &mut Frame) {
    let outer = Layout::vertical([
        Constraint::Length(2), // header
        Constraint::Min(0),    // body
        Constraint::Length(1), // status bar or flash message
    ])
    .split(frame.area());

    render_header(state, frame, outer[0]);
    render_body(state, frame, outer[1]);
    if state.status_message.is_some() {
        render_flash_message(state, frame, outer[2]);
    } else {
        render_status_bar(state, frame, outer[2]);
    }

    if state.search.is_some() {
        render_search_overlay(state, frame);
    } else if state.confirm_dialog.is_some() {
        render_confirm_dialog(state, frame);
    } else if state.input_dialog.is_some() {
        render_input_dialog(state, frame);
    } else if state.profile_dialog.is_some() {
        render_profile_dialog(state, frame);
    } else if state.undo_confirm.is_some() {
        render_undo_confirm_dialog(state, frame);
    } else if state.git_log_dialog.is_some() {
        render_git_log_dialog(state, frame);
    } else if state.app_link_dialog.is_some() {
        render_app_link_dialog(state, frame);
    } else if state.help_dialog.is_some() {
        render_help_dialog(state, frame);
    }
}

fn render_header(state: &MainViewTui, frame: &mut Frame, area: Rect) {
    let profile_span = Span::styled(
        format!(" roost · profile: {}", state.active_profile),
        Style::default().add_modifier(Modifier::BOLD),
    );
    let count_span = Span::styled(
        format!("  {} apps managed", state.app_count()),
        Style::default().fg(Color::DarkGray),
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![profile_span, count_span])),
        area,
    );
}

fn render_body(state: &mut MainViewTui, frame: &mut Frame, area: Rect) {
    let chunks = Layout::horizontal([Constraint::Length(24), Constraint::Min(0)]).split(area);

    render_app_panel(state, frame, chunks[0]);
    render_miller_columns(state, frame, chunks[1]);
}

fn render_app_panel(state: &mut MainViewTui, frame: &mut Frame, area: Rect) {
    let active_prof = state.config.profiles.get(&state.active_profile);
    let items: Vec<ListItem> = state
        .app_names
        .iter()
        .map(|name| {
            let has_primary = state
                .config
                .apps
                .get(name)
                .map_or(false, |app| app.primary_config.is_some());
            let marker = if has_primary { "★ " } else { "  " };
            let source = active_prof.and_then(|p| p.app_sources.get(name));
            if let Some(src) = source {
                ListItem::new(Line::from(vec![
                    Span::raw(format!("{}{}", marker, name)),
                    Span::styled(
                        format!(" \u{2190}{}", src),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]))
            } else {
                ListItem::new(format!("{}{}", marker, name))
            }
        })
        .collect();

    let focused = state.focus == PanelFocus::Apps;
    let border_color = if focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Apps")
                .border_style(Style::default().fg(border_color)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("» ");

    frame.render_stateful_widget(list, area, &mut state.app_list_state);
}

fn render_miller_columns(state: &mut MainViewTui, frame: &mut Frame, area: Rect) {
    let cols = Layout::horizontal([
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
    ])
    .split(area);

    let focused = state.focus == PanelFocus::Files;
    let border_color = if focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let border_style = Style::default().fg(border_color);

    let miller = &state.miller;

    if let Some(parent_entries) = miller.parent_listing() {
        let parent_cursor = miller.parent_cursor().unwrap_or(0);
        render_file_column(
            parent_entries,
            parent_cursor,
            "Parent",
            border_style,
            frame,
            cols[0],
        );
    } else {
        frame.render_widget(
            Block::default()
                .borders(Borders::ALL)
                .title("Parent")
                .border_style(border_style),
            cols[0],
        );
    }

    let current_entries = miller.current_listing();
    let current_cursor = miller.current_cursor();
    let current_title = miller
        .current_dir()
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| miller.current_dir().display().to_string());
    render_file_column(
        current_entries,
        current_cursor,
        &current_title,
        border_style,
        frame,
        cols[1],
    );

    match miller.current_entry() {
        Some(entry) if entry.path().is_dir() => {
            let preview: Vec<FileEntry> = match state.selected_app() {
                Some(app) => {
                    let dir = entry.path();
                    build_preview_entries(dir, &state.config.ignored, app.primary_config.as_deref())
                }
                None => Vec::new(),
            };
            if !preview.is_empty() {
                render_file_column(&preview, 0, "Preview", border_style, frame, cols[2]);
            } else {
                frame.render_widget(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Preview")
                        .border_style(border_style),
                    cols[2],
                );
            }
        }
        Some(entry) if !entry.path().is_dir() => {
            let content = std::fs::read_to_string(entry.path())
                .unwrap_or_else(|e| format!("(could not read file: {})", e));
            let lines: Vec<Line> = content
                .lines()
                .take((cols[2].height as usize).saturating_sub(2))
                .map(|line| Line::from(format!(" {}", line)))
                .collect();
            let file_name = entry
                .path()
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            frame.render_widget(
                Paragraph::new(lines).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!("Preview: {}", file_name))
                        .border_style(border_style),
                ),
                cols[2],
            );
        }
        _ => {
            frame.render_widget(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Preview")
                    .border_style(border_style),
                cols[2],
            );
        }
    }
}

fn render_file_column(
    entries: &[FileEntry],
    cursor: usize,
    title: &str,
    border_style: Style,
    frame: &mut Frame,
    area: Rect,
) {
    let items: Vec<ListItem> = entries
        .iter()
        .map(|entry| {
            let marker = if entry.is_primary { "★ " } else { "  " };
            let suffix = if entry.path.is_dir() { "/" } else { "" };
            let style = if entry.is_primary {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };
            ListItem::new(format!("{}{}{}", marker, entry.name, suffix)).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title.to_string())
                .border_style(border_style),
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

fn build_preview_entries(
    dir: &std::path::Path,
    ignored: &std::collections::HashSet<String>,
    primary_config: Option<&std::path::Path>,
) -> Vec<FileEntry> {
    super::state::build_tracked_entries(dir, ignored, primary_config)
}

fn render_confirm_dialog(state: &MainViewTui, frame: &mut Frame) {
    let Some(ref confirm) = state.confirm_dialog else {
        return;
    };

    let is_remove = matches!(confirm.kind, ConfirmKind::RemoveApp { .. });
    let app_name = confirm.app_name().to_string();

    let (title, lines, border_color) = if is_remove {
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!(
                    " Remove '{}' from profile '{}'?",
                    app_name, state.active_profile
                ),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                " Files will be restored to their original location.",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled(" [y] ", Style::default().fg(Color::Red)),
                Span::raw("remove   "),
                Span::styled(" [n] ", Style::default().fg(Color::DarkGray)),
                Span::raw("cancel"),
            ]),
            Line::from(""),
        ];
        (" Remove App ", lines, Color::Red)
    } else {
        let file_name = match &confirm.kind {
            ConfirmKind::SetPrimary { file_path, .. } => file_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            ConfirmKind::RemoveApp { .. } => unreachable!(),
        };
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!(" Set '{}' as primary config for '{}'?", file_name, app_name),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled(" [y] ", Style::default().fg(Color::Green)),
                Span::raw("yes   "),
                Span::styled(" [n] ", Style::default().fg(Color::DarkGray)),
                Span::raw("cancel"),
            ]),
            Line::from(""),
        ];
        (" Primary Config ", lines, Color::Yellow)
    };

    let dialog_width = 54u16;
    let dialog_height = (2 + lines.len()) as u16;
    let area = crate::tui::search::centered_rect(dialog_width, dialog_height, frame.area());

    frame.render_widget(ratatui::widgets::Clear, area);
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(Style::default().fg(border_color)),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_search_overlay(state: &mut MainViewTui, frame: &mut Frame) {
    if let Some(ref search) = state.search {
        crate::tui::search::render_search_overlay(search, frame);
    }
}

fn render_input_dialog(state: &MainViewTui, frame: &mut Frame) {
    let Some(ref input) = state.input_dialog else {
        return;
    };

    let filtered = state.filtered_ignores();
    let max_visible = 14usize;

    let is_remove = input.mode == IgnoreMode::Remove;

    let mut lines: Vec<Line> = Vec::new();

    match input.mode {
        IgnoreMode::Add => {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("Pattern", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(": "),
                Span::styled(&input.query, Style::default().fg(Color::Cyan)),
                Span::styled("_", Style::default().fg(Color::DarkGray)),
            ]));
            lines.push(Line::from(Span::styled(
                format!(
                    "  {} patterns  Tab=switch to remove",
                    state.config.ignored.len()
                ),
                Style::default().fg(Color::DarkGray),
            )));
        }
        IgnoreMode::Remove => {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("Filter", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(": "),
                Span::styled(&input.query, Style::default().fg(Color::DarkGray)),
            ]));
            lines.push(Line::from(Span::styled(
                format!(
                    "  {}/{} patterns  j/k=nav  Enter=remove  Tab=add mode",
                    filtered.len(),
                    state.config.ignored.len()
                ),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    let cursor = input.cursor;
    for (i, pattern) in filtered.iter().take(max_visible).enumerate() {
        let is_highlighted = is_remove && i == cursor;
        let (prefix, style) = if is_highlighted {
            ("  > ", Style::default().fg(Color::Red))
        } else {
            ("    ", Style::default().fg(Color::DarkGray))
        };
        lines.push(Line::from(Span::styled(
            format!("{}{}", prefix, pattern),
            style,
        )));
    }

    // 2 border + lines
    let dialog_height = 2 + lines.len() as u16;
    let dialog_width = 60u16;

    let title = match input.mode {
        IgnoreMode::Add => " Add Ignore ",
        IgnoreMode::Remove => " Remove Ignore ",
    };

    let area = crate::tui::search::centered_rect(dialog_width, dialog_height, frame.area());

    frame.render_widget(ratatui::widgets::Clear, area);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        area,
    );
}

fn render_profile_dialog(state: &MainViewTui, frame: &mut Frame) {
    let Some(ref pd) = state.profile_dialog else {
        return;
    };

    let profiles = state.profile_names_sorted();
    let max_visible = 10usize;

    let mut lines: Vec<Line> = Vec::new();

    if let Some(ref target) = pd.delete_target {
        let app_count = state
            .config
            .profiles
            .get(target)
            .map(|p| p.apps.len())
            .unwrap_or(0);
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!(
                " Permanently delete '{}'? {} app(s) will be restored.",
                target, app_count
            ),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(" [y] ", Style::default().fg(Color::Red)),
            Span::raw("delete   "),
            Span::styled(" [n] ", Style::default().fg(Color::DarkGray)),
            Span::raw("cancel"),
        ]));
        lines.push(Line::from(""));

        let dialog_width = 50u16;
        let dialog_height = 2 + lines.len() as u16;
        let area = crate::tui::search::centered_rect(dialog_width, dialog_height, frame.area());

        frame.render_widget(ratatui::widgets::Clear, area);
        frame.render_widget(
            Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Delete Profile ")
                    .border_style(Style::default().fg(Color::Red)),
            ),
            area,
        );
        return;
    }

    match pd.mode {
        ProfileMode::Switch => {
            lines.push(Line::from(Span::styled(
                "  j/k=nav  Enter=switch  Tab=create mode",
                Style::default().fg(Color::DarkGray),
            )));
            for (i, name) in profiles.iter().take(max_visible).enumerate() {
                let is_active = *name == state.active_profile;
                let is_cursor = i == pd.cursor;
                let prefix = if is_cursor { "  > " } else { "    " };
                let suffix = if is_active { " (active)" } else { "" };
                let style = if is_cursor {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else if is_active {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                lines.push(Line::from(Span::styled(
                    format!("{}{}{}", prefix, name, suffix),
                    style,
                )));
            }
        }
        ProfileMode::Create => {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("Name", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(": "),
                Span::styled(&pd.new_name, Style::default().fg(Color::Cyan)),
                Span::styled("_", Style::default().fg(Color::DarkGray)),
            ]));
            let (current_style, empty_style) = if pd.create_from_current {
                (
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                    Style::default().fg(Color::DarkGray),
                )
            } else {
                (
                    Style::default().fg(Color::DarkGray),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            };
            lines.push(Line::from(vec![
                Span::raw("  Start from: "),
                Span::styled("[current]", current_style),
                Span::raw("  "),
                Span::styled("[empty]", empty_style),
                Span::styled("  Space=toggle", Style::default().fg(Color::DarkGray)),
            ]));
            lines.push(Line::from(Span::styled(
                "  Enter=create  Tab=delete mode",
                Style::default().fg(Color::DarkGray),
            )));
        }
        ProfileMode::Delete => {
            lines.push(Line::from(Span::styled(
                "  j/k=nav  Enter=delete  Tab=switch mode",
                Style::default().fg(Color::DarkGray),
            )));
            for (i, name) in profiles.iter().take(max_visible).enumerate() {
                let is_active = *name == state.active_profile;
                let is_cursor = i == pd.cursor;
                let prefix = if is_cursor { "  > " } else { "    " };
                let suffix = if is_active { " (active)" } else { "" };
                let style = if is_active {
                    Style::default().fg(Color::DarkGray)
                } else if is_cursor {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                lines.push(Line::from(Span::styled(
                    format!("{}{}{}", prefix, name, suffix),
                    style,
                )));
            }
        }
    }

    let dialog_height = 2 + lines.len() as u16;
    let dialog_width = 50u16;

    let title = match pd.mode {
        ProfileMode::Switch => " Switch Profile ",
        ProfileMode::Create => " New Profile ",
        ProfileMode::Delete => " Delete Profile ",
    };

    let border_color = match pd.mode {
        ProfileMode::Delete => Color::Red,
        _ => Color::Yellow,
    };

    let area = crate::tui::search::centered_rect(dialog_width, dialog_height, frame.area());

    frame.render_widget(ratatui::widgets::Clear, area);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(border_color)),
        ),
        area,
    );
}

fn render_flash_message(state: &MainViewTui, frame: &mut Frame, area: Rect) {
    let Some(ref msg) = state.status_message else {
        return;
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {}", msg),
            Style::default().fg(Color::Cyan),
        ))),
        area,
    );
}

fn render_status_bar(state: &MainViewTui, frame: &mut Frame, area: Rect) {
    let key = |k: &str| Span::styled(format!(" {} ", k), Style::default().fg(Color::Yellow));
    let label = |l: &str| Span::raw(format!("{}  ", l));
    let dim = |t: &'static str| Span::styled(t, Style::default().fg(Color::DarkGray));

    let context_hint = if state.focus == PanelFocus::Files {
        "[files] e=edit  p=primary  h/l=cols"
    } else {
        "[apps]  o=open  a=add  x=remove  f=link-from  m=paste-into"
    };

    let spans = vec![
        key("j/k"),
        label("nav"),
        key("Tab"),
        label("focus"),
        key("/"),
        label("search"),
        key("s"),
        label("sync"),
        key("P"),
        label("profiles"),
        key("?"),
        label("help"),
        key("q"),
        label("quit"),
        dim("  ·  "),
        dim(context_hint),
    ];

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_app_link_dialog(state: &MainViewTui, frame: &mut Frame) {
    let Some(ref dlg) = state.app_link_dialog else {
        return;
    };

    let max_visible = 10usize;
    let mut lines: Vec<Line> = Vec::new();

    let (title, border_color) = match dlg.mode {
        AppLinkMode::LinkFrom { .. } => (" Import via Symlink ", Color::Cyan),
        AppLinkMode::PasteInto { .. } => (" Paste Into Profile ", Color::Yellow),
    };

    match &dlg.mode {
        AppLinkMode::LinkFrom { step } => match step {
            LinkFromStep::PickProfile => {
                let profiles = state.app_link_eligible_profiles();
                lines.push(Line::from(Span::styled(
                    "  Select source profile:",
                    Style::default().fg(Color::DarkGray),
                )));
                if profiles.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "  (no eligible profiles)",
                        Style::default().fg(Color::DarkGray),
                    )));
                } else {
                    for (i, name) in profiles.iter().take(max_visible).enumerate() {
                        let is_cursor = i == dlg.cursor;
                        let prefix = if is_cursor { "  > " } else { "    " };
                        let style = if is_cursor {
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        };
                        lines.push(Line::from(Span::styled(
                            format!("{}{}", prefix, name),
                            style,
                        )));
                    }
                }
            }
            LinkFromStep::PickApp { source_profile } => {
                let apps = state.app_link_eligible_apps(source_profile);
                lines.push(Line::from(Span::styled(
                    format!("  From profile '{}':", source_profile),
                    Style::default().fg(Color::DarkGray),
                )));
                if apps.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "  (no importable apps)",
                        Style::default().fg(Color::DarkGray),
                    )));
                } else {
                    for (i, name) in apps.iter().take(max_visible).enumerate() {
                        let is_cursor = i == dlg.cursor;
                        let prefix = if is_cursor { "  > " } else { "    " };
                        let style = if is_cursor {
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        };
                        lines.push(Line::from(Span::styled(
                            format!("{}{}", prefix, name),
                            style,
                        )));
                    }
                }
            }
        },
        AppLinkMode::PasteInto { app_name } => {
            let profiles = state.app_link_eligible_profiles();
            lines.push(Line::from(Span::styled(
                format!("  Copy '{}' to:", app_name),
                Style::default().fg(Color::DarkGray),
            )));
            if profiles.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  (no eligible profiles)",
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                for (i, name) in profiles.iter().take(max_visible).enumerate() {
                    let is_cursor = i == dlg.cursor;
                    let prefix = if is_cursor { "  > " } else { "    " };
                    let style = if is_cursor {
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    lines.push(Line::from(Span::styled(
                        format!("{}{}", prefix, name),
                        style,
                    )));
                }
            }
        }
    }

    lines.push(Line::from(Span::styled(
        "  j/k=nav  Enter=select  Esc=cancel",
        Style::default().fg(Color::DarkGray),
    )));

    let dialog_width = 60u16;
    let dialog_height = (2 + lines.len()) as u16;
    let area = crate::tui::search::centered_rect(dialog_width, dialog_height, frame.area());

    frame.render_widget(ratatui::widgets::Clear, area);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(border_color)),
        ),
        area,
    );
}

fn render_help_dialog(state: &MainViewTui, frame: &mut Frame) {
    let Some(ref dlg) = state.help_dialog else {
        return;
    };

    let in_search = dlg.focus == HelpFocus::Search;
    let matches = dlg.matches();
    let max_visible = 16usize;

    // scroll offset
    let start = dlg.scroll.min(matches.len().saturating_sub(1));
    let end = (start + max_visible).min(matches.len());
    let visible = &matches[start..end];

    let mut lines: Vec<Line> = Vec::new();

    // search input row — active when in_search, dimmed when scrolling list
    let search_label_style = if in_search {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let query_style = if in_search {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let cursor_span = if in_search {
        Span::styled("█", Style::default().fg(Color::Cyan))
    } else {
        Span::raw("")
    };
    let tab_hint = if in_search {
        "  Tab=scroll list"
    } else {
        "  Tab=edit search"
    };
    lines.push(Line::from(vec![
        Span::styled("  Search: ", search_label_style),
        Span::styled(&dlg.query, query_style),
        cursor_span,
        Span::styled(tab_hint, Style::default().fg(Color::DarkGray)),
    ]));

    // column header
    lines.push(Line::from(Span::styled(
        format!("  {:<12}  {:<18}  {}", "Key", "Action", "Description"),
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::UNDERLINED),
    )));

    for &idx in visible {
        let kb = &super::dialogs::help::KEYBINDS[idx];
        let ctx = kb.context.unwrap_or("");
        let line_text = format!("  {:<12}  {:<18}  {}", kb.key, kb.action, kb.description);
        let style = if in_search {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        };
        let mut spans = vec![Span::styled(line_text, style)];
        if !ctx.is_empty() {
            spans.push(Span::styled(
                format!("  {}", ctx),
                Style::default().fg(Color::DarkGray),
            ));
        }
        lines.push(Line::from(spans));
    }

    if matches.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (no matches)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    // scroll footer — only shown when in list mode
    let total = matches.len();
    if !in_search && total > 0 {
        let footer = if total > max_visible {
            format!("  j/k to scroll  ·  {}/{}", start + 1, total)
        } else {
            "  j/k to scroll".to_string()
        };
        lines.push(Line::from(Span::styled(
            footer,
            Style::default().fg(Color::Cyan),
        )));
    }

    let dialog_height = (2 + lines.len()) as u16;
    let dialog_width = 72u16;
    let border_color = if in_search {
        Color::Yellow
    } else {
        Color::Cyan
    };

    let area = crate::tui::search::centered_rect(dialog_width, dialog_height, frame.area());

    frame.render_widget(ratatui::widgets::Clear, area);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Keybind Reference  ·  Esc or ? to close ")
                .border_style(Style::default().fg(border_color)),
        ),
        area,
    );
}

fn render_git_log_dialog(state: &MainViewTui, frame: &mut Frame) {
    let Some(ref dlg) = state.git_log_dialog else {
        return;
    };

    if let Some(ref hash) = dlg.confirm_rollback {
        let short = &hash[..7.min(hash.len())];
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!(" Rollback to {}?", short),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                " This is destructive — working tree will be reset.",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled(" [y] ", Style::default().fg(Color::Red)),
                Span::raw("rollback   "),
                Span::styled(" [n] ", Style::default().fg(Color::DarkGray)),
                Span::raw("cancel"),
            ]),
            Line::from(""),
        ];

        let dialog_width = 50u16;
        let dialog_height = 2 + lines.len() as u16;
        let area = crate::tui::search::centered_rect(dialog_width, dialog_height, frame.area());

        frame.render_widget(ratatui::widgets::Clear, area);
        frame.render_widget(
            Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Rollback ")
                    .border_style(Style::default().fg(Color::Red)),
            ),
            area,
        );
        return;
    }

    let max_visible = 14usize;
    let max_msg = 30usize;

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        " j/k=nav  Enter=diff  r=rollback  Esc=close",
        Style::default().fg(Color::DarkGray),
    )));

    for (i, entry) in dlg.entries.iter().take(max_visible).enumerate() {
        let is_cursor = i == dlg.cursor;
        let prefix = if is_cursor { " > " } else { "   " };
        let msg = if entry.message.len() > max_msg {
            format!("{}...", &entry.message[..max_msg - 3])
        } else {
            entry.message.clone()
        };
        let style = if is_cursor {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        lines.push(Line::from(Span::styled(
            format!(
                "{}{}  {:>12}  {}",
                prefix, entry.short_hash, entry.date, msg
            ),
            style,
        )));
    }

    let dialog_height = 2 + lines.len() as u16;
    let dialog_width = 58u16;

    let area = crate::tui::search::centered_rect(dialog_width, dialog_height, frame.area());

    frame.render_widget(ratatui::widgets::Clear, area);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Git History ")
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        area,
    );
}

fn render_undo_confirm_dialog(state: &MainViewTui, frame: &mut Frame) {
    let Some(ref uc) = state.undo_confirm else {
        return;
    };

    let max_msg = 40usize;
    let msg = if uc.message.len() > max_msg {
        format!("{}...", &uc.message[..max_msg - 3])
    } else {
        uc.message.clone()
    };

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            " Undo last commit?",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!(" \"{}\" ({})", msg, uc.date),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            " Warning: this permanently discards changes.",
            Style::default().fg(Color::Red),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(" [y] ", Style::default().fg(Color::Red)),
            Span::raw("undo   "),
            Span::styled(" [n] ", Style::default().fg(Color::DarkGray)),
            Span::raw("cancel"),
        ]),
        Line::from(""),
    ];

    let dialog_width = 50u16;
    let dialog_height = 2 + lines.len() as u16;
    let area = crate::tui::search::centered_rect(dialog_width, dialog_height, frame.area());

    frame.render_widget(ratatui::widgets::Clear, area);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Undo (Destructive) ")
                .border_style(Style::default().fg(Color::Red)),
        ),
        area,
    );
}
