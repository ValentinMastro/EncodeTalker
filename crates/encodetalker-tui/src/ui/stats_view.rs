use crate::app::AppState;
use chrono::Local;
use encodetalker_common::JobStatus;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph},
};

/// Convertir un pourcentage (0.0-100.0) en u16 pour les gauges, en clampant à [0, 100]
///
/// Safe: clamp garantit [0, 100], pas de troncation ni de valeur négative
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#[inline]
fn percent_to_u16(percent: f64) -> u16 {
    percent.clamp(0.0, 100.0) as u16
}

/// Rendre la vue des jobs actifs
pub fn render_active_view(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(
            " ⚙️  Encodage en cours ({}) ",
            state.active_jobs.len()
        ))
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
            state
                .active_jobs
                .iter()
                .map(|_| Constraint::Length(8))
                .collect::<Vec<_>>(),
        )
        .split(area);

    for (i, job) in state.active_jobs.iter().enumerate() {
        if i < chunks.len() {
            render_active_job(frame, chunks[i], job, i == state.selected_index);
        }
    }
}

/// Rendre un job actif individuel
fn render_active_job(
    frame: &mut Frame,
    area: Rect,
    job: &encodetalker_common::EncodingJob,
    selected: bool,
) {
    let filename = job
        .input_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let border_style = if selected {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Green)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {filename} "))
        .border_style(border_style);

    if let Some(stats) = &job.stats {
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if stats.is_calculating_vmaf {
            render_vmaf_progress(frame, inner, stats);
        } else {
            render_encoding_progress(frame, inner, stats);
        }
    } else {
        let text = Paragraph::new("Démarrage...")
            .block(block)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(text, area);
    }
}

/// Rendre la progression du calcul VMAF
fn render_vmaf_progress(
    frame: &mut Frame,
    inner: Rect,
    stats: &encodetalker_common::EncodingStats,
) {
    let vmaf_info = format!(
        "Calcul VMAF... Frame: {} / {}",
        stats.frame,
        stats
            .total_frames
            .map_or("?".to_string(), |t| t.to_string())
    );

    let info_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .split(inner);

    let info = Paragraph::new(vmaf_info).style(Style::default().fg(Color::Cyan));
    frame.render_widget(info, info_chunks[1]);

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
        .percent(percent_to_u16(stats.progress_percent))
        .label(format!("VMAF: {:.1}%", stats.progress_percent));

    frame.render_widget(gauge, info_chunks[2]);
}

/// Rendre la progression de l'encodage vidéo
fn render_encoding_progress(
    frame: &mut Frame,
    inner: Rect,
    stats: &encodetalker_common::EncodingStats,
) {
    let progress = stats.progress_percent;

    let eta_text = if let Some(eta) = stats.eta {
        let secs = eta.as_secs();
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        let secs = secs % 60;
        format!("ETA: {hours:02}:{mins:02}:{secs:02}")
    } else {
        "ETA: --:--:--".to_string()
    };

    let info_text = format!(
        "Frame: {} | FPS: {:.1} | Bitrate: {:.1} kbps | {}",
        stats.frame, stats.fps, stats.bitrate, eta_text
    );

    let info_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .split(inner);

    let info = Paragraph::new(info_text).style(Style::default().fg(Color::White));
    frame.render_widget(info, info_chunks[1]);

    if stats.total_passes > 1 {
        // Double barre de progression (aomenc 2 passes)
        let pass1_progress = if stats.current_pass == 1 {
            stats.progress_percent
        } else {
            100.0
        };
        let pass2_progress = if stats.current_pass == 2 {
            stats.progress_percent
        } else {
            0.0
        };

        let gauge1 = Gauge::default()
            .block(Block::default().borders(Borders::NONE))
            .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
            .percent(percent_to_u16(pass1_progress))
            .label(format!("Passe 1: {pass1_progress:.1}%"));

        let gauge2 = Gauge::default()
            .block(Block::default().borders(Borders::NONE))
            .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray))
            .percent(percent_to_u16(pass2_progress))
            .label(format!("Passe 2: {pass2_progress:.1}%"));

        let gauge_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(info_chunks[2]);

        frame.render_widget(gauge1, gauge_chunks[0]);
        frame.render_widget(gauge2, gauge_chunks[1]);
    } else {
        // Barre unique (SVT-AV1)
        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::NONE))
            .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray))
            .percent(percent_to_u16(progress))
            .label(format!("{progress:.1}%"));

        frame.render_widget(gauge, info_chunks[2]);
    }
}

/// Rendre la vue de l'historique
pub fn render_history_view(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(
            " 📜 Historique ({} tâches) ",
            state.history_jobs.len()
        ))
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
    let items: Vec<ListItem> = state
        .history_jobs
        .iter()
        .map(|job| {
            let filename = job
                .input_path
                .file_name()
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
                format!("{hours}:{mins:02}:{secs:02}")
            } else {
                "--:--:--".to_string()
            };

            let vmaf_text = if let Some(vmaf) = job.stats.as_ref().and_then(|s| s.vmaf_score) {
                format!(" | VMAF: {vmaf:.2}")
            } else {
                String::new()
            };

            let started_text = match job.started_at {
                Some(dt) => dt
                    .with_timezone(&Local)
                    .format("%d/%m/%Y %H:%M:%S")
                    .to_string(),
                None => "--".to_string(),
            };

            let finished_text = match job.finished_at {
                Some(dt) => dt
                    .with_timezone(&Local)
                    .format("%d/%m/%Y %H:%M:%S")
                    .to_string(),
                None => "--".to_string(),
            };

            let error_text = if let Some(error) = &job.error_message {
                format!("\n  Erreur: {error}")
            } else {
                String::new()
            };

            let text = format!(
                "{status_icon} {filename} | Durée: {duration_text}{vmaf_text}\n  Début: {started_text}\n  Fin:   {finished_text}{error_text}"
            );

            ListItem::new(text).style(Style::default().fg(status_color))
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
