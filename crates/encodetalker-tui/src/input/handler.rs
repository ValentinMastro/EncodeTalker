use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use encodetalker_common::{EncoderType, AudioMode};
use crate::app::{AppState, View, Dialog, ConfirmAction, EncodeConfigDialog};

/// Gérer un événement clavier
pub fn handle_key_event(state: &mut AppState, key: KeyEvent) -> InputAction {
    // Si un dialogue est ouvert, le gérer en priorité
    if state.dialog.is_some() {
        return handle_dialog_key(state, key);
    }

    // Gestion des touches globales
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => {
            state.should_quit = true;
            return InputAction::None;
        }
        KeyCode::Tab => {
            state.switch_view(state.current_view.next());
            return InputAction::None;
        }
        KeyCode::BackTab => {
            state.switch_view(state.current_view.prev());
            return InputAction::None;
        }
        _ => {}
    }

    // Gestion des touches spécifiques à la vue
    match state.current_view {
        View::FileBrowser => handle_file_browser_key(state, key),
        View::Queue => handle_queue_key(state, key),
        View::Active => handle_active_key(state, key),
        View::History => handle_history_key(state, key),
    }
}

/// Actions possibles suite à un input
#[derive(Debug, Clone)]
pub enum InputAction {
    None,
    RefreshLists,
    AddJob { input_path: std::path::PathBuf, output_path: std::path::PathBuf, config: encodetalker_common::EncodingConfig },
    CancelJob { job_id: uuid::Uuid },
    RetryJob { job_id: uuid::Uuid },
    ClearHistory,
}

/// Gérer les touches dans le file browser
fn handle_file_browser_key(state: &mut AppState, key: KeyEvent) -> InputAction {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            state.move_up();
            InputAction::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.move_down();
            InputAction::None
        }
        KeyCode::Enter => {
            // Naviguer ou sélectionner
            if let Some(entry) = state.file_browser.get_selected(state.selected_index) {
                if entry.is_dir {
                    // Naviguer vers le répertoire
                    state.file_browser.navigate_to(entry.path.clone());
                    state.selected_index = 0;
                } else if entry.is_video {
                    // Ouvrir le dialogue de configuration
                    state.dialog = Some(Dialog::EncodeConfig(EncodeConfigDialog::new(entry.path.clone())));
                }
            }
            InputAction::None
        }
        KeyCode::Char('a') => {
            // Ajouter le fichier sélectionné (shortcut)
            if let Some(entry) = state.file_browser.get_selected(state.selected_index) {
                if entry.is_video {
                    state.dialog = Some(Dialog::EncodeConfig(EncodeConfigDialog::new(entry.path.clone())));
                }
            }
            InputAction::None
        }
        KeyCode::Char('r') => {
            // Rafraîchir
            state.file_browser.refresh();
            state.selected_index = 0;
            InputAction::None
        }
        _ => InputAction::None,
    }
}

/// Gérer les touches dans la queue
fn handle_queue_key(state: &mut AppState, key: KeyEvent) -> InputAction {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            state.move_up();
            InputAction::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.move_down();
            InputAction::None
        }
        KeyCode::Char('c') => {
            // Annuler le job sélectionné
            if let Some(job) = state.queue_jobs.get(state.selected_index) {
                state.dialog = Some(Dialog::Confirm {
                    message: format!("Annuler le job {} ?", job.id),
                    on_confirm: ConfirmAction::CancelJob,
                });
            }
            InputAction::None
        }
        KeyCode::Char('r') => {
            InputAction::RefreshLists
        }
        _ => InputAction::None,
    }
}

/// Gérer les touches dans les jobs actifs
fn handle_active_key(state: &mut AppState, key: KeyEvent) -> InputAction {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            state.move_up();
            InputAction::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.move_down();
            InputAction::None
        }
        KeyCode::Char('c') => {
            // Annuler le job sélectionné
            if let Some(job) = state.active_jobs.get(state.selected_index) {
                state.dialog = Some(Dialog::Confirm {
                    message: format!("Annuler le job {} en cours ?", job.id),
                    on_confirm: ConfirmAction::CancelJob,
                });
            }
            InputAction::None
        }
        KeyCode::Char('r') => {
            InputAction::RefreshLists
        }
        _ => InputAction::None,
    }
}

/// Gérer les touches dans l'historique
fn handle_history_key(state: &mut AppState, key: KeyEvent) -> InputAction {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            state.move_up();
            InputAction::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.move_down();
            InputAction::None
        }
        KeyCode::Char('r') => {
            // Retry un job failed
            if let Some(job) = state.history_jobs.get(state.selected_index) {
                if matches!(job.status, encodetalker_common::JobStatus::Failed) {
                    return InputAction::RetryJob { job_id: job.id };
                }
            }
            InputAction::RefreshLists
        }
        KeyCode::Char('C') => {
            // Clear history (maj + C)
            state.dialog = Some(Dialog::Confirm {
                message: "Effacer tout l'historique ?".to_string(),
                on_confirm: ConfirmAction::ClearHistory,
            });
            InputAction::None
        }
        _ => InputAction::None,
    }
}

/// Gérer les touches dans un dialogue
fn handle_dialog_key(state: &mut AppState, key: KeyEvent) -> InputAction {
    let dialog = state.dialog.clone();
    match dialog {
        Some(Dialog::EncodeConfig(_)) => handle_encode_config_dialog_key(state, key),
        Some(Dialog::Confirm { on_confirm, .. }) => handle_confirm_dialog_key(state, key, on_confirm),
        Some(Dialog::Error { .. }) => {
            // N'importe quelle touche ferme l'erreur
            state.dialog = None;
            InputAction::None
        }
        None => InputAction::None,
    }
}

/// Gérer les touches dans le dialogue de config d'encodage
fn handle_encode_config_dialog_key(state: &mut AppState, key: KeyEvent) -> InputAction {
    if let Some(Dialog::EncodeConfig(ref mut config)) = state.dialog {
        match key.code {
            KeyCode::Esc => {
                state.dialog = None;
                return InputAction::None;
            }
            KeyCode::Up => {
                config.move_field_up();
                return InputAction::None;
            }
            KeyCode::Down => {
                config.move_field_down();
                return InputAction::None;
            }
            KeyCode::Left | KeyCode::Right => {
                // Changer la valeur du champ sélectionné
                toggle_field_value(config, key.code == KeyCode::Right);
                return InputAction::None;
            }
            KeyCode::Enter => {
                // Valider et ajouter le job
                let input_path = config.input_path.clone();
                let output_path = config.output_path.clone();
                let encoding_config = config.config.clone();

                state.dialog = None;
                state.set_status("Job ajouté à la queue");

                return InputAction::AddJob {
                    input_path,
                    output_path,
                    config: encoding_config,
                };
            }
            _ => {}
        }
    }

    InputAction::None
}

/// Changer la valeur d'un champ dans le dialogue de config
fn toggle_field_value(config: &mut EncodeConfigDialog, increment: bool) {
    match config.selected_field {
        0 => {
            // Encodeur
            config.config.encoder = match config.config.encoder {
                EncoderType::SvtAv1 => EncoderType::Aom,
                EncoderType::Aom => EncoderType::SvtAv1,
            };
        }
        1 => {
            // Audio mode
            config.config.audio_mode = match config.config.audio_mode {
                AudioMode::Opus { .. } => AudioMode::Copy,
                AudioMode::Copy => AudioMode::Opus { bitrate: 128 },
                AudioMode::Custom { .. } => AudioMode::Opus { bitrate: 128 },
            };
        }
        2 => {
            // CRF
            if increment && config.config.encoder_params.crf < 51 {
                config.config.encoder_params.crf += 1;
            } else if !increment && config.config.encoder_params.crf > 0 {
                config.config.encoder_params.crf -= 1;
            }
        }
        3 => {
            // Preset
            let max_preset = match config.config.encoder {
                EncoderType::SvtAv1 => 13,
                EncoderType::Aom => 8,
            };
            if increment && config.config.encoder_params.preset < max_preset {
                config.config.encoder_params.preset += 1;
            } else if !increment && config.config.encoder_params.preset > 0 {
                config.config.encoder_params.preset -= 1;
            }
        }
        _ => {}
    }
}

/// Gérer les touches dans le dialogue de confirmation
fn handle_confirm_dialog_key(state: &mut AppState, key: KeyEvent, on_confirm: ConfirmAction) -> InputAction {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
            // Confirmer
            state.dialog = None;

            match on_confirm {
                ConfirmAction::CancelJob => {
                    // Obtenir le job ID depuis la vue active
                    let job_id = match state.current_view {
                        View::Queue => state.queue_jobs.get(state.selected_index).map(|j| j.id),
                        View::Active => state.active_jobs.get(state.selected_index).map(|j| j.id),
                        _ => None,
                    };

                    if let Some(job_id) = job_id {
                        return InputAction::CancelJob { job_id };
                    }
                }
                ConfirmAction::ClearHistory => {
                    return InputAction::ClearHistory;
                }
            }

            InputAction::None
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            // Annuler
            state.dialog = None;
            InputAction::None
        }
        _ => InputAction::None,
    }
}
