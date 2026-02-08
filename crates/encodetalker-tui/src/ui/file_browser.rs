use ratatui::{
    prelude::*,
    widgets::*,
};
use crate::app::AppState;

/// Rendre le navigateur de fichiers
pub fn render_file_browser(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" 📁 {} ", state.file_browser.current_dir.display()))
        .border_style(Style::default().fg(Color::Cyan));

    // Créer les items de la liste
    let items: Vec<ListItem> = state.file_browser.entries.iter()
        .map(|entry| {
            let icon = if entry.is_dir {
                "📁"
            } else if entry.is_video {
                "🎬"
            } else {
                "📄"
            };

            let style = if entry.is_dir {
                Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)
            } else if entry.is_video {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let text = format!("{} {}", icon, entry.name);
            ListItem::new(text).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        )
        .highlight_symbol("▶ ");

    let mut list_state = ListState::default();
    list_state.select(Some(state.selected_index));

    frame.render_stateful_widget(list, area, &mut list_state);
}
