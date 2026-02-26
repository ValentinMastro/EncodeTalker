use crate::app::state::format_duration;
use crate::app::AppState;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState},
};

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
                "\u{1F4C1}"
            } else if entry.is_video {
                "🎬"
            } else {
                "\u{1F4C4}"
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

            // Calculer largeur disponible pour le nom
            let available_width = area.width.saturating_sub(
                2 +   // checkbox (2 chars)
                2 +   // icône emoji (compte pour 2)
                1 +   // espace après icône
                14 +  // colonne taille (9 chars + " Mo" + 2 séparateurs) - uniquement pour vidéos
                14 +  // colonne durée (11 chars "JJ:HH:MM:SS" + 3 séparateurs) - uniquement pour vidéos
                2, // bordures du bloc
            );

            // Formatter la taille (uniquement pour vidéos, arrondi au Mo)
            let size_str = if entry.is_video {
                entry
                    .size_bytes
                    .map(|b| format!("{:>9} Mo", b / 1_000_000))
                    .unwrap_or_else(|| "        - Mo".to_string())
            } else {
                "            ".to_string() // vide pour les non-vidéos
            };

            // Formatter la durée (uniquement pour vidéos, format JJ:HH:MM:SS toujours complet)
            let duration_str = if entry.is_video {
                entry
                    .duration_secs
                    .map(|s| format!("{:>11}", format_duration(s)))
                    .unwrap_or_else(|| "          ?".to_string())
            } else {
                "            ".to_string() // vide pour les non-vidéos
            };

            // Tronquer le nom si trop long
            let display_name = if entry.name.len() > available_width as usize {
                format!(
                    "{}…",
                    &entry.name[..available_width.saturating_sub(1) as usize]
                )
            } else {
                format!("{:<width$}", entry.name, width = available_width as usize)
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

            // Assembler la ligne
            let text = format!(
                "{}{} {} | {} | {}",
                checkbox, icon, display_name, size_str, duration_str
            );
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
