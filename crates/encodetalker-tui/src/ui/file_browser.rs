use crate::app::AppState;
use ratatui::{prelude::*, widgets::{Block, Borders, ListItem, List, ListState}};

/// Rendre le navigateur de fichiers
pub fn render_file_browser(frame: &mut Frame, area: Rect, state: &AppState) {
    // Compteur de sélections dans le titre
    let selection_count = state.file_browser.selected_files.len();
    let title = if selection_count > 0 {
        format!(
            " 📁 {} ({} sélectionné{}) ",
            state.file_browser.current_dir.display(),
            selection_count,
            if selection_count > 1 { "s" } else { "" }
        )
    } else {
        format!(" 📁 {} ", state.file_browser.current_dir.display())
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::Cyan));

    // Créer les items de la liste
    let items: Vec<ListItem> = state
        .file_browser
        .entries
        .iter()
        .map(|entry| {
            let icon = if entry.is_dir {
                "📁"
            } else if entry.is_video {
                "🎬"
            } else {
                "📄"
            };

            // Checkbox pour les vidéos
            let checkbox = if entry.is_video {
                if state.file_browser.is_selected(&entry.path) {
                    "☑ "
                } else {
                    "  " // 2 espaces pour alignement
                }
            } else {
                "  "
            };

            // Style pour sélections
            let style = if state.file_browser.is_selected(&entry.path) {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if entry.is_dir {
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD)
            } else if entry.is_video {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let text = format!("{}{} {}", checkbox, icon, entry.name);
            ListItem::new(text).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut list_state = ListState::default();
    list_state.select(Some(state.selected_index));

    frame.render_stateful_widget(list, area, &mut list_state);
}
