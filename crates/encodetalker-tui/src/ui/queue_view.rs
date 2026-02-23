use crate::app::AppState;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

/// Rendre la vue de la queue
pub fn render_queue_view(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" 📋 Queue ({} jobs) ", state.queue_jobs.len()))
        .border_style(Style::default().fg(Color::Yellow));

    if state.queue_jobs.is_empty() {
        let text = Paragraph::new(
            "Aucun job en queue\n\nUtilisez l'onglet Files pour ajouter des vidéos à encoder.",
        )
        .block(block)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(text, area);
        return;
    }

    // Créer les items de la liste
    let items: Vec<ListItem> = state
        .queue_jobs
        .iter()
        .map(|job| {
            let filename = job
                .input_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            let encoder = match job.config.encoder {
                encodetalker_common::EncoderType::SvtAv1 => "SVT-AV1",
                encodetalker_common::EncoderType::Aom => "libaom",
            };

            let audio = match &job.config.audio_mode {
                encodetalker_common::AudioMode::Opus { bitrate } => format!("Opus {bitrate}k"),
                encodetalker_common::AudioMode::Copy => "Copie".to_string(),
                encodetalker_common::AudioMode::Custom { codec, bitrate } => {
                    format!("{codec} {bitrate}k")
                }
            };

            let vmaf = if job.config.enable_vmaf { "oui" } else { "non" };

            let text = format!(
                "{}\n  Encoder: {} | Audio: {} | CRF: {} | Preset: {} | VMAF: {}",
                filename,
                encoder,
                audio,
                job.config.encoder_params.crf,
                job.config.encoder_params.preset,
                vmaf
            );

            ListItem::new(text).style(Style::default().fg(Color::White))
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
