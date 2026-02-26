use crate::app::{
    AppState, ConfirmAction, Dialog, EncodeConfigDialog, LastClick, View, VmafGraphData,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use encodetalker_common::{AudioMode, EncoderType, VideoContentType};
use std::time::{Duration, Instant};

/// Obtenir le nombre max de threads disponibles sur la machine
///
/// Safe: nombre de cœurs réaliste (8-256) bien inférieur à `u32::MAX`
#[allow(clippy::cast_possible_truncation)]
#[inline]
fn get_max_threads() -> u32 {
    std::thread::available_parallelism().map_or(16, |n| n.get().min(u32::MAX as usize) as u32)
}

/// Gérer un événement clavier
pub fn handle_key_event(state: &mut AppState, key: KeyEvent) -> InputAction {
    // Si on est en Loading, bloquer toutes les touches sauf 'q'
    if state.current_view == View::Loading {
        if matches!(key.code, KeyCode::Char('q' | 'Q')) {
            state.dialog = Some(Dialog::Confirm {
                message: "Voulez-vous quitter l'application ?\n(La compilation continuera en arrière-plan)".to_string(),
                on_confirm: ConfirmAction::Quit,
            });
        }
        return InputAction::None;
    }

    // Si un dialogue est ouvert, le gérer en priorité
    if state.dialog.is_some() {
        return handle_dialog_key(state, key);
    }

    // Gestion des touches globales
    match key.code {
        KeyCode::Char('q' | 'Q') => {
            // Ctrl+Q : Quitter directement sans confirmation
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                state.should_quit = true;
                return InputAction::None;
            }

            // q simple : Demander confirmation
            state.dialog = Some(Dialog::Confirm {
                message: "Voulez-vous quitter l'application ?".to_string(),
                on_confirm: ConfirmAction::Quit,
            });
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
        View::Loading => InputAction::None, // Ne devrait pas arriver (déjà géré au début)
        View::FileBrowser => handle_file_browser_key(state, key),
        View::Queue => handle_queue_key(state, key),
        View::Active => handle_active_key(state, key),
        View::History => handle_history_key(state, key),
    }
}

/// Gérer un événement souris
///
/// # Panics
/// Panic si `state.dialog` est `Some` mais `state.layout.dialog_area` est `None` (incohérence d'état).
pub fn handle_mouse_event(state: &mut AppState, mouse: MouseEvent) -> InputAction {
    // Ignorer complètement la souris en mode Loading
    if state.current_view == View::Loading {
        return InputAction::None;
    }

    let MouseEvent {
        kind, column, row, ..
    } = mouse;

    // Priorité au dialogue si ouvert
    if state.dialog.is_some() {
        let dialog_area = state
            .layout
            .dialog_area
            .expect("dialog_area doit être Some si dialog est Some");
        // Scroll dans le dialogue
        match &mut state.dialog {
            Some(Dialog::EncodeConfig(ref mut config)) => match kind {
                MouseEventKind::ScrollUp => {
                    if config.selected_field > 0 {
                        config.selected_field -= 1;
                    }
                    return InputAction::None;
                }
                MouseEventKind::ScrollDown => {
                    if config.selected_field < 7 {
                        config.selected_field += 1;
                    }
                    return InputAction::None;
                }
                _ => {}
            },
            Some(Dialog::VideoInfo {
                scroll_offset,
                output,
                ..
            }) => {
                match kind {
                    MouseEventKind::ScrollUp => {
                        *scroll_offset = scroll_offset.saturating_sub(1);
                        return InputAction::None;
                    }
                    MouseEventKind::ScrollDown => {
                        let total_lines = output.lines().count();
                        // Calculer hauteur visible réelle (80% hauteur - bordures - titre)
                        let visible_lines = if let Some(dialog_area) = state.layout.dialog_area {
                            dialog_area.height.saturating_sub(2) as usize // -2 pour bordures
                        } else {
                            40 // Fallback si pas de dialog_area
                        };
                        let max_scroll = total_lines.saturating_sub(visible_lines);
                        *scroll_offset = (*scroll_offset + 1).min(max_scroll);
                        return InputAction::None;
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        // Clic dans la zone du dialogue
        if column >= dialog_area.x
            && column < dialog_area.x + dialog_area.width
            && row >= dialog_area.y
            && row < dialog_area.y + dialog_area.height
        {
            // Clic dans le dialogue — ne rien faire (géré par clavier)
            return InputAction::None;
        }

        // Clic en dehors du dialogue → fermer et réinitialiser le tracking de clic
        if matches!(kind, MouseEventKind::Down(MouseButton::Left)) {
            state.dialog = None;
            state.layout.dialog_area = None; // Libérer la zone du dialogue
            state.last_click = None; // Empêcher faux double-clic après fermeture
            return InputAction::None;
        }

        return InputAction::None;
    }

    // Scroll dans la zone de contenu
    if column >= state.layout.content.x
        && column < state.layout.content.x + state.layout.content.width
        && row >= state.layout.content.y
        && row < state.layout.content.y + state.layout.content.height
    {
        match kind {
            MouseEventKind::ScrollUp => {
                state.move_up();
                return InputAction::None;
            }
            MouseEventKind::ScrollDown => {
                state.move_down();
                return InputAction::None;
            }
            _ => {}
        }
    }

    // Clic gauche sur un item de la zone de contenu
    if matches!(kind, MouseEventKind::Down(MouseButton::Left)) {
        return handle_content_click(state, row, column);
    }

    InputAction::None
}

/// Actions possibles suite à un input
#[derive(Debug, Clone)]
pub enum InputAction {
    None,
    RefreshLists,
    AddJob {
        input_path: std::path::PathBuf,
        output_path: std::path::PathBuf,
        config: encodetalker_common::EncodingConfig,
    },
    /// Ajouter plusieurs jobs avec la même config
    AddBatchJobs {
        jobs: Vec<(std::path::PathBuf, std::path::PathBuf)>,
        config: encodetalker_common::EncodingConfig,
    },
    CancelJob {
        job_id: uuid::Uuid,
    },
    RetryJob {
        job_id: uuid::Uuid,
    },
    RemoveFromHistory {
        job_id: uuid::Uuid,
    },
    ClearHistory,
}

/// Gérer un clic sur le contenu (détection double-clic et sélection)
fn handle_content_click(state: &mut AppState, row: u16, column: u16) -> InputAction {
    let content_inner = state.layout.content_inner;

    if column >= content_inner.x
        && column < content_inner.x + content_inner.width
        && row >= content_inner.y
        && row < content_inner.y + content_inner.height
    {
        // Calculer l'index cliqué (approximation du scroll)
        #[allow(clippy::cast_possible_truncation)]
        let visible_height = content_inner.height.saturating_sub(1) as usize;
        let scroll_offset = state.selected_index.saturating_sub(visible_height);
        #[allow(clippy::cast_possible_truncation)]
        let relative_row = row.saturating_sub(content_inner.y) as usize;
        let clicked_index = scroll_offset + relative_row;

        // Vérifier que l'index est valide
        let list_len = state.get_current_list_len();
        if clicked_index < list_len {
            // Détecter le double-clic (même position dans les 500ms)
            let now = Instant::now();
            let is_double_click = state.last_click.as_ref().is_some_and(|last| {
                now.duration_since(last.timestamp) < Duration::from_millis(500)
                    && last.row == row
                    && last.column == column
            });

            // Enregistrer ce clic
            state.last_click = Some(LastClick {
                timestamp: now,
                row,
                column,
            });

            state.selected_index = clicked_index;

            // Double-clic dans FileBrowser → action Enter (ouvrir dossier ou dialogue)
            if is_double_click && state.current_view == View::FileBrowser {
                return handle_file_browser_enter(state);
            }

            // Simple clic dans FileBrowser : toggle selection si vidéo
            if state.current_view == View::FileBrowser {
                state.file_browser.toggle_selection(clicked_index);
            }
        }

        return InputAction::None;
    }

    InputAction::None
}

/// Gérer l'action Enter dans le file browser (ouvrir dossier ou dialogue encodage)
fn handle_file_browser_enter(state: &mut AppState) -> InputAction {
    let selected_files = state.file_browser.get_selected_files();

    if let Some(entry) = state.file_browser.get_selected(state.selected_index) {
        if entry.is_dir {
            // Toujours naviguer dans les dossiers (priorité)
            state.file_browser.navigate_to(entry.path.clone());
            state.selected_index = 0;
        } else if !selected_files.is_empty() {
            // Batch avec fichiers sélectionnés
            state.dialog = Some(Dialog::EncodeConfig(EncodeConfigDialog::new_batch(
                selected_files,
            )));
        } else if entry.is_video {
            // Single file: comportement actuel
            state.dialog = Some(Dialog::EncodeConfig(EncodeConfigDialog::new(
                entry.path.clone(),
            )));
        }
    }
    InputAction::None
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

        // Toggle sélection avec ESPACE
        KeyCode::Char(' ') => {
            state.file_browser.toggle_selection(state.selected_index);
            InputAction::None
        }

        // Ctrl+A sélectionner toutes les vidéos
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.file_browser.select_all_videos();
            InputAction::None
        }

        // Ctrl+D désélectionner tout
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.file_browser.clear_selection();
            InputAction::None
        }

        // Logique Enter pour batch
        KeyCode::Enter => handle_file_browser_enter(state),

        // 'a' : Shortcut pour single file (ignore les sélections, compatibilité)
        KeyCode::Char('a') => {
            if let Some(entry) = state.file_browser.get_selected(state.selected_index) {
                if entry.is_video {
                    state.dialog = Some(Dialog::EncodeConfig(EncodeConfigDialog::new(
                        entry.path.clone(),
                    )));
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

        KeyCode::Char('i') => {
            // Afficher les informations vidéo (ffmpeg -i)
            if let Some(entry) = state.file_browser.get_selected(state.selected_index) {
                if entry.is_video {
                    let file_path = state.file_browser.current_dir.join(&entry.name);

                    // Chercher .dependencies/ en remontant depuis l'exécutable (mode portable)
                    let find_portable_deps = || {
                        let mut dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
                        loop {
                            let candidate = dir.join(".dependencies");
                            if candidate.is_dir() {
                                return Some(candidate);
                            }
                            if !dir.pop() {
                                return None;
                            }
                        }
                    };

                    // Chemin ffmpeg : .dependencies/bin/ffmpeg (portable) ou XDG
                    let ffmpeg_bin = find_portable_deps()
                        .map(|d| d.join("bin").join("ffmpeg"))
                        .or_else(|| {
                            dirs::data_local_dir().map(|d| d.join("encodetalker/deps/bin/ffmpeg"))
                        })
                        .unwrap_or_else(|| std::path::PathBuf::from("ffmpeg"));

                    // Exécuter ffmpeg -hide_banner -i <fichier>
                    match std::process::Command::new(&ffmpeg_bin)
                        .arg("-hide_banner")
                        .arg("-i")
                        .arg(&file_path)
                        .output()
                    {
                        Ok(output) => {
                            // ffmpeg écrit les infos sur stderr
                            let info = String::from_utf8_lossy(&output.stderr).to_string();
                            state.dialog = Some(Dialog::VideoInfo {
                                path: file_path,
                                output: info,
                                scroll_offset: 0,
                            });
                        }
                        Err(e) => {
                            state.dialog = Some(Dialog::Error {
                                message: format!("Impossible de lancer ffmpeg: {e}"),
                            });
                        }
                    }
                }
            }
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
        KeyCode::Char('r') => InputAction::RefreshLists,
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
        KeyCode::Char('r') => InputAction::RefreshLists,
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
        KeyCode::Enter => {
            if let Some(job) = state.history_jobs.get(state.selected_index) {
                if let Some(vmaf_path) = job.stats.as_ref().and_then(|s| s.vmaf_json_path.as_ref())
                {
                    if vmaf_path.exists() {
                        let filename = job
                            .input_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        match VmafGraphData::from_json_file(vmaf_path, filename) {
                            Ok(data) => {
                                state.dialog = Some(Dialog::VmafGraph(data));
                            }
                            Err(e) => {
                                state.dialog = Some(Dialog::Error {
                                    message: format!("Erreur de lecture VMAF: {e}"),
                                });
                            }
                        }
                    } else {
                        state.dialog = Some(Dialog::Error {
                            message: "Fichier VMAF introuvable".to_string(),
                        });
                    }
                }
            }
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
        KeyCode::Char('c') => {
            // Effacer une tâche (minuscule)
            if state.history_jobs.get(state.selected_index).is_some() {
                state.dialog = Some(Dialog::Confirm {
                    message: "Effacer cette tâche de l'historique ?".to_string(),
                    on_confirm: ConfirmAction::RemoveFromHistory,
                });
            }
            InputAction::None
        }
        KeyCode::Char('C') => {
            // Effacer tout l'historique (majuscule)
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
        Some(Dialog::Confirm { on_confirm, .. }) => {
            handle_confirm_dialog_key(state, key, on_confirm)
        }
        Some(Dialog::Error { .. }) => {
            // N'importe quelle touche ferme l'erreur
            state.dialog = None;
            InputAction::None
        }
        Some(Dialog::VmafGraph(_)) => {
            if matches!(key.code, KeyCode::Esc | KeyCode::Enter) {
                state.dialog = None;
            }
            InputAction::None
        }
        Some(Dialog::VideoInfo { .. }) => {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    state.dialog = None;
                    InputAction::None
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if let Some(Dialog::VideoInfo { scroll_offset, .. }) = &mut state.dialog {
                        *scroll_offset = scroll_offset.saturating_sub(1);
                    }
                    InputAction::None
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if let Some(Dialog::VideoInfo {
                        scroll_offset,
                        output,
                        ..
                    }) = &mut state.dialog
                    {
                        let total_lines = output.lines().count();
                        // Calculer hauteur visible réelle (80% hauteur - bordures - titre)
                        let visible_lines = if let Some(dialog_area) = state.layout.dialog_area {
                            dialog_area.height.saturating_sub(2) as usize // -2 pour bordures
                        } else {
                            40 // Fallback si pas de dialog_area
                        };
                        let max_scroll = total_lines.saturating_sub(visible_lines);
                        *scroll_offset = (*scroll_offset + 1).min(max_scroll);
                    }
                    InputAction::None
                }
                _ => InputAction::None,
            }
        }
        None => InputAction::None,
    }
}

/// Gérer l'édition du chemin de sortie
fn handle_output_path_editing(config: &mut EncodeConfigDialog, key: KeyEvent) -> InputAction {
    match key.code {
        KeyCode::Esc => {
            // Annuler et restaurer
            config.output_path_string = config.output_path.display().to_string();
            config.is_editing_output = false;
            InputAction::None
        }
        KeyCode::Enter => {
            config.stop_editing_output();
            InputAction::None
        }
        KeyCode::Left => {
            if config.output_path_cursor > 0 {
                config.output_path_cursor -= 1;
            }
            InputAction::None
        }
        KeyCode::Right => {
            let char_count = config.output_path_string.chars().count();
            if config.output_path_cursor < char_count {
                config.output_path_cursor += 1;
            }
            InputAction::None
        }
        KeyCode::Home => {
            config.output_path_cursor = 0;
            InputAction::None
        }
        KeyCode::End => {
            config.output_path_cursor = config.output_path_string.chars().count();
            InputAction::None
        }
        KeyCode::Backspace => {
            if config.output_path_cursor > 0 {
                let mut chars: Vec<char> = config.output_path_string.chars().collect();
                if config.output_path_cursor <= chars.len() {
                    chars.remove(config.output_path_cursor - 1);
                    config.output_path_string = chars.into_iter().collect();
                    config.output_path_cursor -= 1;
                }
            }
            InputAction::None
        }
        KeyCode::Delete => {
            let mut chars: Vec<char> = config.output_path_string.chars().collect();
            if config.output_path_cursor < chars.len() {
                chars.remove(config.output_path_cursor);
                config.output_path_string = chars.into_iter().collect();
            }
            InputAction::None
        }
        KeyCode::Char(c) => {
            let mut chars: Vec<char> = config.output_path_string.chars().collect();
            chars.insert(config.output_path_cursor, c);
            config.output_path_string = chars.into_iter().collect();
            config.output_path_cursor += 1;
            InputAction::None
        }
        _ => InputAction::None,
    }
}

/// Gérer les touches dans le dialogue de config d'encodage
fn handle_encode_config_dialog_key(state: &mut AppState, key: KeyEvent) -> InputAction {
    if let Some(Dialog::EncodeConfig(ref mut config)) = state.dialog {
        // Si en mode édition du chemin (désactivé si batch)
        if config.is_editing_output && !config.is_batch() {
            return handle_output_path_editing(config, key);
        }

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
                // Si sur field 7 (output path) et batch, ne rien faire
                if config.selected_field == 7 && key.code == KeyCode::Right {
                    if !config.is_batch() {
                        config.start_editing_output();
                    }
                } else {
                    toggle_field_value(config, key.code == KeyCode::Right);
                }
                return InputAction::None;
            }

            // Validation avec logique batch
            KeyCode::Enter => {
                // Si sur field 7 et pas batch, activer l'édition
                if config.selected_field == 7 && !config.is_batch() {
                    config.start_editing_output();
                    return InputAction::None;
                }

                // Early return pour clarté
                if !config.is_batch() {
                    // Single job: comportement actuel
                    let input_path = config.input_paths[0].clone();
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

                // Batch jobs
                {
                    let encoding_config = config.config.clone();
                    // Créer plusieurs jobs
                    let jobs: Vec<(std::path::PathBuf, std::path::PathBuf)> = config
                        .input_paths
                        .iter()
                        .map(|input| {
                            let output = generate_output_path(input);
                            (input.clone(), output)
                        })
                        .collect();

                    state.dialog = None;
                    state.set_status(format!("{} jobs ajoutés à la queue", jobs.len()));

                    // Clear les sélections après ajout
                    state.file_browser.clear_selection();

                    return InputAction::AddBatchJobs {
                        jobs,
                        config: encoding_config,
                    };
                }
            }
            _ => {}
        }
    }

    InputAction::None
}

/// Générer le chemin de sortie pour un fichier d'entrée
fn generate_output_path(input: &std::path::Path) -> std::path::PathBuf {
    let mut output = input.to_path_buf();
    output.set_extension("");
    std::path::PathBuf::from(format!("{}.av1.mkv", output.display()))
}

/// Changer la valeur d'un champ dans le dialogue de config
#[allow(clippy::match_same_arms)]
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
                AudioMode::Copy | AudioMode::Custom { .. } => AudioMode::Opus { bitrate: 128 },
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
        4 => {
            // Threads
            let max_threads = get_max_threads();

            match config.config.encoder_params.threads {
                None => {
                    // Auto → 1 ou Auto → max
                    if increment {
                        config.config.encoder_params.threads = Some(1);
                    } else {
                        config.config.encoder_params.threads = Some(max_threads);
                    }
                }
                Some(n) => {
                    if increment {
                        if n < max_threads {
                            config.config.encoder_params.threads = Some(n + 1);
                        } else {
                            // max → Auto
                            config.config.encoder_params.threads = None;
                        }
                    } else if n > 1 {
                        config.config.encoder_params.threads = Some(n - 1);
                    } else {
                        // 1 → Auto
                        config.config.encoder_params.threads = None;
                    }
                }
            }
        }
        5 => {
            // VMAF toggle
            config.config.enable_vmaf = !config.config.enable_vmaf;
        }
        6 => {
            // Content Type: cycle Default → Anime → LiveAction → GrainedFilm → Default
            config.config.encoder_params.content_type =
                match config.config.encoder_params.content_type {
                    VideoContentType::Default => {
                        if increment {
                            VideoContentType::Anime
                        } else {
                            VideoContentType::GrainedFilm
                        }
                    }
                    VideoContentType::Anime => {
                        if increment {
                            VideoContentType::LiveAction
                        } else {
                            VideoContentType::Default
                        }
                    }
                    VideoContentType::LiveAction => {
                        if increment {
                            VideoContentType::GrainedFilm
                        } else {
                            VideoContentType::Anime
                        }
                    }
                    VideoContentType::GrainedFilm => {
                        if increment {
                            VideoContentType::Default
                        } else {
                            VideoContentType::LiveAction
                        }
                    }
                };
        }
        7 => {
            // Output path: géré par le mode édition, ne rien faire ici
        }
        _ => {
            // Autres cas: ne rien faire
        }
    }
}

/// Gérer les touches dans le dialogue de confirmation
fn handle_confirm_dialog_key(
    state: &mut AppState,
    key: KeyEvent,
    on_confirm: ConfirmAction,
) -> InputAction {
    match key.code {
        KeyCode::Char('y' | 'Y' | 'o' | 'O') | KeyCode::Enter => {
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
                ConfirmAction::RemoveFromHistory => {
                    if let Some(job) = state.history_jobs.get(state.selected_index) {
                        return InputAction::RemoveFromHistory { job_id: job.id };
                    }
                }
                ConfirmAction::ClearHistory => {
                    return InputAction::ClearHistory;
                }
                ConfirmAction::Quit => {
                    state.should_quit = true;
                    return InputAction::None;
                }
            }

            InputAction::None
        }
        KeyCode::Char('n' | 'N') | KeyCode::Esc => {
            // Annuler
            state.dialog = None;
            InputAction::None
        }
        _ => InputAction::None,
    }
}
