use crate::app::{AppState, View};
use ratatui::{prelude::*, widgets::*};

/// Rendre l'interface complète
pub fn render_ui(frame: &mut Frame, state: &AppState) {
    let area = frame.size();

    // Layout principal : header + contenu + footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Contenu
            Constraint::Length(1), // Footer
        ])
        .split(area);

    // Rendre le header
    render_header(frame, chunks[0], state);

    // Rendre le contenu selon la vue active
    match state.current_view {
        View::FileBrowser => crate::ui::render_file_browser(frame, chunks[1], state),
        View::Queue => crate::ui::render_queue_view(frame, chunks[1], state),
        View::Active => crate::ui::render_active_view(frame, chunks[1], state),
        View::History => crate::ui::render_history_view(frame, chunks[1], state),
    }

    // Rendre le footer
    render_footer(frame, chunks[2], state);

    // Rendre le dialogue par-dessus si présent
    if state.dialog.is_some() {
        crate::ui::render_dialog(frame, area, state);
    }
}

/// Rendre le header avec les onglets
fn render_header(frame: &mut Frame, area: Rect, state: &AppState) {
    let titles = vec![
        "Nouvel encodage",
        "Queue",
        "Encodage en cours...",
        "Historique",
    ];
    let selected = match state.current_view {
        View::FileBrowser => 0,
        View::Queue => 1,
        View::Active => 2,
        View::History => 3,
    };

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("EncodeTalker"))
        .select(selected)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(tabs, area);
}

/// Rendre le footer avec les raccourcis clavier
fn render_footer(frame: &mut Frame, area: Rect, state: &AppState) {
    let text = if state.dialog.is_some() {
        " ESC: Fermer | Enter: Valider "
    } else {
        match state.current_view {
            View::FileBrowser => " Tab: Vue suivante | ↑↓: Naviguer | Enter: Ouvrir | a: Ajouter | r: Rafraîchir | q: Quitter ",
            View::Queue => " Tab: Vue suivante | ↑↓: Naviguer | c: Annuler | r: Rafraîchir | q: Quitter ",
            View::Active => " Tab: Vue suivante | ↑↓: Naviguer | c: Annuler | r: Rafraîchir | q: Quitter ",
            View::History => " Tab: Vue suivante | ↑↓: Naviguer | r: Réessayer | c: Effacer | C: Tout effacer | q: Quitter ",
        }
    };

    let footer = Paragraph::new(text)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White))
        .alignment(Alignment::Center);

    frame.render_widget(footer, area);
}
