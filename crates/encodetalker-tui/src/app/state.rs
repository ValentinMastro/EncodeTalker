use encodetalker_common::{EncodingConfig, EncodingJob};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Vue active de l'application
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    FileBrowser,
    Queue,
    Active,
    History,
}

impl View {
    pub fn next(&self) -> Self {
        match self {
            View::FileBrowser => View::Queue,
            View::Queue => View::Active,
            View::Active => View::History,
            View::History => View::FileBrowser,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            View::FileBrowser => View::History,
            View::Queue => View::FileBrowser,
            View::Active => View::Queue,
            View::History => View::Active,
        }
    }

    pub fn title(&self) -> &str {
        match self {
            View::FileBrowser => "Nouvel encodage",
            View::Queue => "Queue",
            View::Active => "Encodage en cours",
            View::History => "Historique",
        }
    }
}

/// État de l'application TUI
pub struct AppState {
    /// Vue active
    pub current_view: View,
    /// Doit quitter l'application
    pub should_quit: bool,
    /// État du file browser
    pub file_browser: FileBrowserState,
    /// Jobs en queue
    pub queue_jobs: Vec<EncodingJob>,
    /// Jobs actifs
    pub active_jobs: Vec<EncodingJob>,
    /// Historique
    pub history_jobs: Vec<EncodingJob>,
    /// Index de sélection dans la vue active
    pub selected_index: usize,
    /// Dialogue ouvert
    pub dialog: Option<Dialog>,
    /// Message de status
    pub status_message: Option<String>,
}

impl AppState {
    pub fn new(start_dir: PathBuf) -> Self {
        Self {
            current_view: View::FileBrowser,
            should_quit: false,
            file_browser: FileBrowserState::new(start_dir),
            queue_jobs: Vec::new(),
            active_jobs: Vec::new(),
            history_jobs: Vec::new(),
            selected_index: 0,
            dialog: None,
            status_message: None,
        }
    }

    /// Changer de vue
    pub fn switch_view(&mut self, view: View) {
        self.current_view = view;
        self.selected_index = 0;
    }

    /// Naviguer vers le haut dans la liste
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Naviguer vers le bas dans la liste
    pub fn move_down(&mut self) {
        let max = self.get_current_list_len();
        if self.selected_index < max.saturating_sub(1) {
            self.selected_index += 1;
        }
    }

    /// Obtenir la longueur de la liste active
    fn get_current_list_len(&self) -> usize {
        match self.current_view {
            View::FileBrowser => self.file_browser.entries.len(),
            View::Queue => self.queue_jobs.len(),
            View::Active => self.active_jobs.len(),
            View::History => self.history_jobs.len(),
        }
    }

    /// Définir un message de status
    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = Some(message.into());
    }

    /// Effacer le message de status
    pub fn clear_status(&mut self) {
        self.status_message = None;
    }
}

/// État du navigateur de fichiers
pub struct FileBrowserState {
    /// Répertoire courant
    pub current_dir: PathBuf,
    /// Entrées du répertoire
    pub entries: Vec<DirEntry>,
    /// Fichiers vidéo sélectionnés (chemins absolus)
    pub selected_files: HashSet<PathBuf>,
}

impl FileBrowserState {
    pub fn new(start_dir: PathBuf) -> Self {
        let mut state = Self {
            current_dir: start_dir,
            entries: Vec::new(),
            selected_files: HashSet::new(),
        };
        state.refresh();
        state
    }

    /// Rafraîchir la liste des entrées
    pub fn refresh(&mut self) {
        self.entries.clear();

        // Ajouter l'entrée parent si on n'est pas à la racine
        if self.current_dir.parent().is_some() {
            self.entries.push(DirEntry {
                path: self.current_dir.parent().unwrap().to_path_buf(),
                name: "..".to_string(),
                is_dir: true,
                is_video: false,
            });
        }

        // Lire le répertoire
        if let Ok(entries) = std::fs::read_dir(&self.current_dir) {
            let mut items: Vec<DirEntry> = entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let path = e.path();
                    let name = e.file_name().to_string_lossy().to_string();
                    let is_dir = path.is_dir();

                    // Filtrer les fichiers cachés
                    if name.starts_with('.') {
                        return None;
                    }

                    let is_video = !is_dir && is_video_file(&path);

                    Some(DirEntry {
                        path,
                        name,
                        is_dir,
                        is_video,
                    })
                })
                .collect();

            // Trier : dossiers d'abord, puis fichiers
            items.sort_by(|a, b| match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            });

            self.entries.extend(items);
        }
    }

    /// Naviguer vers un répertoire
    pub fn navigate_to(&mut self, path: PathBuf) {
        if path.is_dir() {
            self.current_dir = path;
            self.selected_files.clear();
            self.refresh();
        }
    }

    /// Obtenir l'entrée sélectionnée
    pub fn get_selected(&self, index: usize) -> Option<&DirEntry> {
        self.entries.get(index)
    }

    /// Toggle la sélection d'un fichier vidéo
    pub fn toggle_selection(&mut self, index: usize) {
        if let Some(entry) = self.entries.get(index) {
            if entry.is_video {
                if self.selected_files.contains(&entry.path) {
                    self.selected_files.remove(&entry.path);
                } else {
                    self.selected_files.insert(entry.path.clone());
                }
            }
        }
    }

    /// Sélectionner toutes les vidéos (Ctrl+A)
    pub fn select_all_videos(&mut self) {
        for entry in &self.entries {
            if entry.is_video && entry.name != ".." {
                self.selected_files.insert(entry.path.clone());
            }
        }
    }

    /// Désélectionner tout (Ctrl+D)
    pub fn clear_selection(&mut self) {
        self.selected_files.clear();
    }

    /// Vérifier si un fichier est sélectionné
    pub fn is_selected(&self, path: &Path) -> bool {
        self.selected_files.contains(path)
    }

    /// Obtenir la liste des fichiers sélectionnés (triée)
    pub fn get_selected_files(&self) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = self.selected_files.iter().cloned().collect();
        files.sort();
        files
    }
}

/// Entrée de répertoire
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub is_video: bool,
}

/// Vérifier si un fichier est une vidéo
fn is_video_file(path: &Path) -> bool {
    const VIDEO_EXTENSIONS: &[&str] = &[
        ".mp4", ".mkv", ".avi", ".mov", ".webm", ".flv", ".wmv", ".m4v", ".m2ts",
    ];

    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        VIDEO_EXTENSIONS
            .iter()
            .any(|&e| e.trim_start_matches('.') == ext_str)
    } else {
        false
    }
}

/// Types de dialogues
#[derive(Debug, Clone)]
pub enum Dialog {
    /// Dialogue de configuration d'encodage
    EncodeConfig(EncodeConfigDialog),
    /// Dialogue de confirmation
    Confirm {
        message: String,
        on_confirm: ConfirmAction,
    },
    /// Dialogue d'erreur
    Error { message: String },
}

/// Actions de confirmation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    CancelJob,
    RemoveFromHistory,
    ClearHistory,
    Quit,
}

/// Dialogue de configuration d'encodage
#[derive(Debug, Clone)]
pub struct EncodeConfigDialog {
    /// Chemins d'entrée (1 si single, N si batch)
    pub input_paths: Vec<PathBuf>,
    pub output_path: PathBuf,
    pub output_path_string: String,
    pub output_path_cursor: usize,
    pub is_editing_output: bool,
    pub config: EncodingConfig,
    pub selected_field: usize,
}

impl EncodeConfigDialog {
    /// Créer dialogue pour un fichier unique
    pub fn new(input_path: PathBuf) -> Self {
        Self::new_batch(vec![input_path])
    }

    /// Créer dialogue pour plusieurs fichiers
    pub fn new_batch(input_paths: Vec<PathBuf>) -> Self {
        let output_path = if input_paths.len() == 1 {
            let mut out = input_paths[0].clone();
            out.set_extension("");
            PathBuf::from(format!("{}.av1.mkv", out.display()))
        } else {
            PathBuf::from("<auto-generated>")
        };

        let output_path_string = output_path.display().to_string();

        Self {
            input_paths,
            output_path,
            output_path_string,
            output_path_cursor: 0,
            is_editing_output: false,
            config: EncodingConfig::default(),
            selected_field: 0,
        }
    }

    /// Est-ce un batch?
    pub fn is_batch(&self) -> bool {
        self.input_paths.len() > 1
    }

    pub fn move_field_up(&mut self) {
        if self.selected_field > 0 {
            self.selected_field -= 1;
        }
    }

    pub fn move_field_down(&mut self) {
        // 6 champs : encodeur, audio mode, CRF, preset, threads, output path
        if self.selected_field < 5 {
            self.selected_field += 1;
        }
    }

    pub fn start_editing_output(&mut self) {
        // Désactiver l'édition si batch
        if self.is_batch() {
            return;
        }
        self.is_editing_output = true;
        self.output_path_cursor = self.output_path_string.chars().count();
    }

    pub fn stop_editing_output(&mut self) {
        self.is_editing_output = false;
        self.sync_output_path();
    }

    pub fn sync_output_path(&mut self) {
        self.output_path = PathBuf::from(&self.output_path_string);
    }
}
