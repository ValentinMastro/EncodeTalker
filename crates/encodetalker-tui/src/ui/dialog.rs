use crate::app::{AppState, Dialog};
use ratatui::{prelude::*, widgets::*};

/// Rendre un dialogue par-dessus l'interface
pub fn render_dialog(frame: &mut Frame, area: Rect, state: &AppState) {
    if let Some(dialog) = &state.dialog {
        match dialog {
            Dialog::EncodeConfig(config) => render_encode_config_dialog(frame, area, config),
            Dialog::Confirm { message, .. } => render_confirm_dialog(frame, area, message),
            Dialog::Error { message } => render_error_dialog(frame, area, message),
        }
    }
}

/// Rendre le dialogue de configuration d'encodage
fn render_encode_config_dialog(
    frame: &mut Frame,
    area: Rect,
    config: &crate::app::EncodeConfigDialog,
) {
    // Centrer le dialogue
    let dialog_area = centered_rect(70, 60, area);

    // Fond semi-transparent
    let clear = Clear;
    frame.render_widget(clear, dialog_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Configure Encoding ")
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(dialog_area);
    frame.render_widget(block, dialog_area);

    // Layout pour les champs
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Input file
            Constraint::Length(3), // Output file
            Constraint::Length(3), // Encoder
            Constraint::Length(3), // Audio mode
            Constraint::Length(3), // CRF
            Constraint::Length(3), // Preset
            Constraint::Length(2), // Instructions
        ])
        .split(inner);

    // Input file
    let input_text = format!("Input:  {}", config.input_path.display());
    let input = Paragraph::new(input_text).style(Style::default().fg(Color::White));
    frame.render_widget(input, chunks[0]);

    // Output file (éditable)
    let output_style = if config.selected_field == 4 {
        if config.is_editing_output {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        }
    } else {
        Style::default().fg(Color::White)
    };

    let output_text = if config.is_editing_output {
        // Mode édition : afficher avec curseur (utiliser chars() pour gérer UTF-8)
        let chars: Vec<char> = config.output_path_string.chars().collect();
        let before: String = chars[..config.output_path_cursor].iter().collect();
        let after: String = chars[config.output_path_cursor..].iter().collect();
        format!("Output: {}█{}", before, after)
    } else if config.selected_field == 4 {
        format!("Output: {} [→ to edit]", config.output_path_string)
    } else {
        format!("Output: {}", config.output_path_string)
    };

    let output = Paragraph::new(output_text).style(output_style);
    frame.render_widget(output, chunks[1]);

    // Encoder
    let encoder_text = format!("Encoder: {}", config.config.encoder);
    let encoder_style = if config.selected_field == 0 {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let encoder = Paragraph::new(encoder_text).style(encoder_style);
    frame.render_widget(encoder, chunks[2]);

    // Audio mode
    let audio_text = match &config.config.audio_mode {
        encodetalker_common::AudioMode::Opus { bitrate } => {
            format!("Audio:   Opus {} kbps", bitrate)
        }
        encodetalker_common::AudioMode::Copy => "Audio:   Copy".to_string(),
        encodetalker_common::AudioMode::Custom { codec, bitrate } => {
            format!("Audio:   {} {} kbps", codec, bitrate)
        }
    };
    let audio_style = if config.selected_field == 1 {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let audio = Paragraph::new(audio_text).style(audio_style);
    frame.render_widget(audio, chunks[3]);

    // CRF
    let crf_text = format!(
        "CRF:     {} (0-51, lower = better quality)",
        config.config.encoder_params.crf
    );
    let crf_style = if config.selected_field == 2 {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let crf = Paragraph::new(crf_text).style(crf_style);
    frame.render_widget(crf, chunks[4]);

    // Preset
    let max_preset = match config.config.encoder {
        encodetalker_common::EncoderType::SvtAv1 => 13,
        encodetalker_common::EncoderType::Aom => 8,
    };
    let preset_text = format!(
        "Preset:  {} (0-{}, higher = faster)",
        config.config.encoder_params.preset, max_preset
    );
    let preset_style = if config.selected_field == 3 {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let preset = Paragraph::new(preset_text).style(preset_style);
    frame.render_widget(preset, chunks[5]);

    // Instructions
    let instructions_text = if config.is_editing_output {
        "←→: Move cursor | Char: Insert | Backspace/Del: Delete | Enter: Done | ESC: Cancel"
    } else {
        "↑↓: Navigate | ←→: Change value | Enter: Add to queue | ESC: Cancel"
    };
    let instructions = Paragraph::new(instructions_text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(instructions, chunks[6]);
}

/// Rendre le dialogue de confirmation
fn render_confirm_dialog(frame: &mut Frame, area: Rect, message: &str) {
    let dialog_area = centered_rect(50, 30, area);

    let clear = Clear;
    frame.render_widget(clear, dialog_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Confirm ")
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(dialog_area);
    frame.render_widget(block, dialog_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(inner);

    let text = Paragraph::new(message)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::White));
    frame.render_widget(text, chunks[0]);

    let instructions = Paragraph::new("Y/Enter: Confirm | N/ESC: Cancel")
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(instructions, chunks[1]);
}

/// Rendre le dialogue d'erreur
fn render_error_dialog(frame: &mut Frame, area: Rect, message: &str) {
    let dialog_area = centered_rect(60, 30, area);

    let clear = Clear;
    frame.render_widget(clear, dialog_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Error ")
        .border_style(Style::default().fg(Color::Red));

    let inner = block.inner(dialog_area);
    frame.render_widget(block, dialog_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

    let text = Paragraph::new(message)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::Red));
    frame.render_widget(text, chunks[0]);

    let instructions = Paragraph::new("Press any key to close")
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(instructions, chunks[1]);
}

/// Créer un rectangle centré
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
