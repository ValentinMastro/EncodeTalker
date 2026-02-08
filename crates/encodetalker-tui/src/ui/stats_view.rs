use ratatui::{
    prelude::*,
    widgets::*,
};
use crate::app::AppState;
use encodetalker_common::JobStatus;

/// Rendre la vue des jobs actifs
pub fn render_active_view(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" ⚙️  Active Jobs ({}) ", state.active_jobs.len()))
        .border_style(Style::default().fg(Color::Green));

    if state.active_jobs.is_empty() {
        let text = Paragraph::new("Aucun job en cours d'encodage")
            .block(block)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(text, area);
        return;
    }

    // Layout pour afficher plusieurs jobs
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            state.active_jobs.iter()
                .map(|_| Constraint::Length(8))
                .collect::<Vec<_>>()
        )
        .split(area);

    for (i, job) in state.active_jobs.iter().enumerate() {
        if i < chunks.len() {
            render_active_job(frame, chunks[i], job, i == state.selected_index);
        }
    }
}

/// Rendre un job actif individuel
fn render_active_job(frame: &mut Frame, area: Rect, job: &encodetalker_common::EncodingJob, selected: bool) {
    let filename = job.input_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let border_style = if selected {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Green)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", filename))
        .border_style(border_style);

    if let Some(stats) = &job.stats {
        let progress = stats.progress_percent;
        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::NONE))
            .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray))
            .percent(progress as u16)
            .label(format!("{:.1}%", progress));

        let eta_text = if let Some(eta) = stats.eta {
            let secs = eta.as_secs();
            let hours = secs / 3600;
            let mins = (secs % 3600) / 60;
            let secs = secs % 60;
            format!("ETA: {:02}:{:02}:{:02}", hours, mins, secs)
        } else {
            "ETA: --:--:--".to_string()
        };

        let info_text = format!(
            "Frame: {} | FPS: {:.1} | Bitrate: {:.1} kbps | {}",
            stats.frame, stats.fps, stats.bitrate, eta_text
        );

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let info_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(3),
            ])
            .split(inner);

        let info = Paragraph::new(info_text)
            .style(Style::default().fg(Color::White));
        frame.render_widget(info, info_chunks[1]);

        frame.render_widget(gauge, info_chunks[2]);
    } else {
        let text = Paragraph::new("Démarrage...")
            .block(block)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(text, area);
    }
}

/// Rendre la vue de l'historique
pub fn render_history_view(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" 📜 History ({} jobs) ", state.history_jobs.len()))
        .border_style(Style::default().fg(Color::Magenta));

    if state.history_jobs.is_empty() {
        let text = Paragraph::new("Aucun job dans l'historique")
            .block(block)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(text, area);
        return;
    }

    // Créer les items de la liste
    let items: Vec<ListItem> = state.history_jobs.iter()
        .map(|job| {
            let filename = job.input_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            let (status_icon, status_color) = match job.status {
                JobStatus::Completed => ("✓", Color::Green),
                JobStatus::Failed => ("✗", Color::Red),
                JobStatus::Cancelled => ("⊘", Color::Yellow),
                _ => ("?", Color::Gray),
            };

            let duration_text = if let Some(duration) = job.execution_duration() {
                let secs = duration.num_seconds();
                let hours = secs / 3600;
                let mins = (secs % 3600) / 60;
                let secs = secs % 60;
                format!("{}:{:02}:{:02}", hours, mins, secs)
            } else {
                "--:--:--".to_string()
            };

            let error_text = if let Some(error) = &job.error_message {
                format!("\n  Error: {}", error)
            } else {
                String::new()
            };

            let text = format!(
                "{} {} | Duration: {}{}",
                status_icon, filename, duration_text, error_text
            );

            ListItem::new(text)
                .style(Style::default().fg(status_color))
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
