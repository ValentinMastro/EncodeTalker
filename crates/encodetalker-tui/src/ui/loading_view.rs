use crate::app::state::LoadingState;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame,
};

/// Rendre la vue de chargement (compilation des dépendances)
pub fn render_loading_view(frame: &mut Frame, area: Rect, state: &LoadingState) {
    if let Some(error) = &state.error {
        render_error_state(frame, area, error);
        return;
    }

    // Si total_deps = 0, on est en train de vérifier
    if state.total_deps == 0 {
        render_checking_deps(frame, area);
        return;
    }

    // Layout principal
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3), // Titre
            Constraint::Length(3), // Barre de progression
            Constraint::Length(8), // Liste des dépendances
            Constraint::Length(3), // Étape actuelle
            Constraint::Min(0),    // Espace
            Constraint::Length(3), // Aide
        ])
        .split(area);

    // Titre
    let title = Paragraph::new("Compilation des dépendances")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, chunks[0]);

    // Barre de progression
    let progress = state.progress_percent();
    let gauge = Gauge::default()
        .block(Block::default().title("Progression").borders(Borders::ALL))
        .gauge_style(
            Style::default()
                .fg(Color::Cyan)
                .bg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .percent(progress);
    frame.render_widget(gauge, chunks[1]);

    // Liste des dépendances
    render_deps_list(frame, chunks[2], state);

    // Étape actuelle
    let current_step_text = if state.completed_deps == state.total_deps {
        "✅ Prêt !".to_string()
    } else if let Some(step) = state.step_text() {
        step
    } else {
        "En attente...".to_string()
    };

    let current_step = Paragraph::new(current_step_text)
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .title("Étape actuelle")
                .borders(Borders::ALL),
        );
    frame.render_widget(current_step, chunks[3]);

    // Aide
    let help = Paragraph::new("q: Quitter | Première compilation: 30-60 minutes")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(help, chunks[5]);
}

/// Afficher la liste des dépendances avec leur statut
fn render_deps_list(frame: &mut Frame, area: Rect, state: &LoadingState) {
    let deps = [
        ("FFmpeg", "15-20 min"),
        ("SVT-AV1-PSY", "10-15 min"),
        ("libaom", "15-20 min"),
    ];

    let items: Vec<ListItem> = deps
        .iter()
        .enumerate()
        .map(|(idx, (name, duration))| {
            let (icon, style) = if idx < state.completed_deps {
                // Terminé
                ("✅", Style::default().fg(Color::Green))
            } else if idx == state.completed_deps {
                // En cours
                ("⏳", Style::default().fg(Color::Yellow))
            } else {
                // En attente
                ("⏸", Style::default().fg(Color::DarkGray))
            };

            let line = Line::from(vec![
                Span::raw("  "),
                Span::styled(icon, style),
                Span::raw(" "),
                Span::styled(format!("{} ({})", name, duration), style),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(Block::default().title("Dépendances").borders(Borders::ALL));

    frame.render_widget(list, area);
}

/// Afficher l'état de vérification initial
fn render_checking_deps(frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Length(3),
            Constraint::Percentage(40),
        ])
        .split(area);

    let message = Paragraph::new("Vérification des dépendances...")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

    frame.render_widget(message, chunks[1]);
}

/// Afficher l'état d'erreur
fn render_error_state(frame: &mut Frame, area: Rect, error: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3), // Titre
            Constraint::Min(5),    // Message d'erreur
            Constraint::Length(6), // Instructions
            Constraint::Min(0),    // Espace
            Constraint::Length(3), // Aide
        ])
        .split(area);

    // Titre
    let title = Paragraph::new("❌ Erreur de compilation")
        .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, chunks[0]);

    // Message d'erreur
    let error_msg = Paragraph::new(error)
        .style(Style::default().fg(Color::Red))
        .alignment(Alignment::Left)
        .block(Block::default().title("Détails").borders(Borders::ALL))
        .wrap(ratatui::widgets::Wrap { trim: true });
    frame.render_widget(error_msg, chunks[1]);

    // Instructions
    let instructions = [
        "Dépendances système requises:",
        "",
        "sudo pacman -S base-devel cmake git nasm",
        "",
        "Puis relancez le daemon.",
    ];

    let instructions_text = instructions.join("\n");
    let instructions_widget = Paragraph::new(instructions_text)
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Left)
        .block(Block::default().title("Solution").borders(Borders::ALL));
    frame.render_widget(instructions_widget, chunks[2]);

    // Aide
    let help = Paragraph::new("q: Quitter")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(help, chunks[4]);
}
