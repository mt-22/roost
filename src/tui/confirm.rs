use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

/// The action that triggered the confirmation dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    Confirm,
    Discard,
}

/// Lightweight reusable yes/no confirmation dialog.
#[derive(Debug, Clone)]
pub struct ConfirmDialog {
    pub title: String,
    pub message: String,
    pub border_color: Color,
    pub action: ConfirmAction,
    pub confirmed: Option<bool>,
}

impl ConfirmDialog {
    pub fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        border_color: Color,
        action: ConfirmAction,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            border_color,
            action,
            confirmed: None,
        }
    }

    pub fn destructive(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(title, message, Color::Red, ConfirmAction::Discard)
    }

    pub fn affirmative(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(title, message, Color::Yellow, ConfirmAction::Confirm)
    }

    pub fn is_active(&self) -> bool {
        self.confirmed.is_none()
    }

    pub fn confirm(&mut self) {
        self.confirmed = Some(true);
    }

    pub fn cancel(&mut self) {
        self.confirmed = Some(false);
    }

    pub fn take_result(&mut self) -> Option<bool> {
        self.confirmed.take()
    }
}

pub fn render_confirm_dialog(frame: &mut ratatui::Frame, dialog: &ConfirmDialog) {
    let area = frame.area();
    let width = 50u16.min(area.width.saturating_sub(4)).max(30);

    let text_width = (width as usize).saturating_sub(4);
    let lines_needed = dialog
        .message
        .chars()
        .collect::<Vec<_>>()
        .chunks(text_width.max(1))
        .count()
        .max(1);
    let height = (lines_needed as u16 + 5).min(area.height.saturating_sub(4)).max(6);

    let popup_x = (area.width.saturating_sub(width)) / 2;
    let popup_y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, width, height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(dialog.border_color))
        .title(format!(" {} ", dialog.title));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let chunks = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(inner);

    let message_lines: Vec<Line> = dialog
        .message
        .lines()
        .map(|line| Line::from(Span::styled(line.to_string(), Style::default().fg(Color::White))))
        .collect();
    let message_para = Paragraph::new(message_lines).wrap(Wrap { trim: true });
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
    let buttons_para = Paragraph::new(buttons).alignment(Alignment::Center);
    frame.render_widget(buttons_para, chunks[1]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirm_dialog_lifecycle() {
        let mut dialog = ConfirmDialog::destructive("Test", "Are you sure?");
        assert!(dialog.is_active());
        assert_eq!(dialog.take_result(), None);

        dialog.confirm();
        assert!(!dialog.is_active());
        assert_eq!(dialog.take_result(), Some(true));
        assert_eq!(dialog.take_result(), None);
    }

    #[test]
    fn cancel_dialog() {
        let mut dialog = ConfirmDialog::affirmative("Test", "Proceed?");
        dialog.cancel();
        assert_eq!(dialog.take_result(), Some(false));
    }
}
