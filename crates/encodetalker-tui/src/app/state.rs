use encodetalker_common::protocol::messages::{DepsCompilationStep, DepsStatusInfo};
use encodetalker_common::{EncodingConfig, EncodingJob};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Données VMAF par frame parsées pour l'affichage du graphe
#[derive(Debug, Clone)]
pub struct VmafGraphData {
    /// Scores VMAF par frame : (`numéro_frame`, `score_vmaf`)
    pub frames: Vec<(f64, f64)>,
    /// Score moyen
    pub mean: f64,
    /// Score minimum
    pub min: f64,
    /// Score maximum
    pub max: f64,
    /// Moyenne harmonique
    pub harmonic_mean: Option<f64>,
    /// Nom du fichier source (pour le titre)
    pub filename: String,
    /// Nombre total de frames
    pub total_frames: usize,
}

impl VmafGraphData {
    /// Parser un fichier JSON VMAF généré par libvmaf
    ///
    /// # Errors
    ///
    /// Retourne une erreur si le fichier ne peut pas être lu ou si le JSON est invalide.
    pub fn from_json_file(path: &Path, filename: String) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let data: serde_json::Value = serde_json::from_str(&content)?;

        let frames: Vec<(f64, f64)> = data
            .get("frames")
            .and_then(|f| f.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|frame| {
                        #[allow(clippy::cast_precision_loss)]
                        let num = frame.get("frameNum")?.as_u64()? as f64;
                        let vmaf = frame.get("metrics")?.get("vmaf")?.as_f64()?;
                        Some((num, vmaf))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let pooled = data.get("pooled_metrics").and_then(|p| p.get("vmaf"));
        let mean = pooled
            .and_then(|v| v.get("mean"))
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        let min = pooled
            .and_then(|v| v.get("min"))
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        let max = pooled
            .and_then(|v| v.get("max"))
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        let harmonic_mean = pooled
            .and_then(|v| v.get("harmonic_mean"))
            .and_then(serde_json::Value::as_f64);

        let total_frames = frames.len();

        Ok(Self {
            frames,
            mean,
            min,
            max,
            harmonic_mean,
            filename,
            total_frames,
        })
    }
}

/// Vue active de l'application
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Loading,
    FileBrowser,
    Queue,
    Active,
    History,
}

impl View {
    #[must_use]
    pub fn next(&self) -> Self {
        match self {
            View::Loading => View::Loading, // Bloquer navigation depuis Loading
            View::FileBrowser => View::Queue,
            View::Queue => View::Active,
            View::Active => View::History,
            View::History => View::FileBrowser,
        }
    }

    #[must_use]
    pub fn prev(&self) -> Self {
        match self {
            View::Loading => View::Loading, // Bloquer navigation depuis Loading
            View::FileBrowser => View::History,
            View::Queue => View::FileBrowser,
            View::Active => View::Queue,
            View::History => View::Active,
        }
    }

    #[must_use]
    pub fn title(&self) -> &str {
        match self {
            View::Loading => "Initialisation",
            View::FileBrowser => "Nouvel encodage",
            View::Queue => "Queue",
            View::Active => "Encodage en cours",
            View::History => "Historique",
        }
    }
}

/// État de la vue de chargement (compilation des dépendances)
#[derive(Debug, Clone)]
pub struct LoadingState {
    /// Nombre total de dépendances à compiler
    pub total_deps: usize,
    /// Nombre de dépendances compilées
    pub completed_deps: usize,
    /// Nom de la dépendance en cours de compilation
    pub current_dep: Option<String>,
    /// Étape actuelle de compilation
    pub current_step: Option<DepsCompilationStep>,
    /// Erreur de compilation
    pub error: Option<String>,
}

impl LoadingState {
    /// Créer un état de chargement vide (en attente)
    #[must_use]
    pub fn new() -> Self {
        Self {
            total_deps: 0,
            completed_deps: 0,
            current_dep: None,
            current_step: None,
            error: None,
        }
    }

    /// Créer depuis un `DepsStatusInfo`
    #[must_use]
    pub fn from_status(status: DepsStatusInfo) -> Self {
        Self {
            total_deps: status.total_count,
            completed_deps: status.completed_count,
            current_dep: status.current_dep,
            current_step: status.current_step,
            error: None,
        }
    }

    /// Calculer le pourcentage de progression
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    pub fn progress_percent(&self) -> u16 {
        if self.total_deps == 0 {
            0
        } else {
            ((self.completed_deps as f64 / self.total_deps as f64) * 100.0) as u16
        }
    }

    /// Obtenir le texte de l'étape actuelle
    #[must_use]
    pub fn step_text(&self) -> Option<String> {
        match (&self.current_dep, &self.current_step) {
            (Some(dep), Some(step)) => {
                let step_str = match step {
                    DepsCompilationStep::Downloading => "Téléchargement",
                    DepsCompilationStep::Building => "Compilation",
                    DepsCompilationStep::Verifying => "Vérification",
                };
                Some(format!("{dep}: {step_str}..."))
            }
            _ => None,
        }
    }
}

impl Default for LoadingState {
    fn default() -> Self {
        Self::new()
    }
}

/// État de l'application TUI
pub struct AppState {
    /// Vue active
    pub current_view: View,
    /// Doit quitter l'application
    pub should_quit: bool,
    /// État du chargement (compilation dépendances)
    pub loading_state: Option<LoadingState>,
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
    #[must_use]
    pub fn new(start_dir: PathBuf) -> Self {
        Self {
            current_view: View::Loading,
            should_quit: false,
            loading_state: Some(LoadingState::new()),
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
            View::Loading => 0,
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
    /// Fichiers vidéo dont on attend les métadonnées du daemon
    pub pending_probes: HashSet<PathBuf>,
}

impl FileBrowserState {
    #[must_use]
    pub fn new(start_dir: PathBuf) -> Self {
        let mut state = Self {
            current_dir: start_dir,
            entries: Vec::new(),
            selected_files: HashSet::new(),
            pending_probes: HashSet::new(),
        };
        state.refresh();
        state
    }

    /// Rafraîchir la liste des entrées
    ///
    /// # Panics
    ///
    /// Peut paniquer si `self.current_dir.parent()` retourne `Some` mais `unwrap()` échoue (ne devrait jamais arriver).
    pub fn refresh(&mut self) {
        self.entries.clear();
        self.pending_probes.clear();

        // Ajouter l'entrée parent si on n'est pas à la racine
        if self.current_dir.parent().is_some() {
            self.entries.push(DirEntry {
                path: self.current_dir.parent().unwrap().to_path_buf(),
                name: "..".to_string(),
                is_dir: true,
                is_video: false,
                size_bytes: None,
                duration_secs: None,
            });
        }

        // Lire le répertoire
        if let Ok(entries) = std::fs::read_dir(&self.current_dir) {
            let mut items: Vec<DirEntry> = entries
                .filter_map(std::result::Result::ok)
                .filter_map(|e| {
                    let path = e.path();
                    let name = e.file_name().to_string_lossy().to_string();
                    let is_dir = path.is_dir();

                    // Filtrer les fichiers cachés
                    if name.starts_with('.') {
                        return None;
                    }

                    let is_video = !is_dir && is_video_file(&path);

                    // Récupérer la taille du fichier
                    let size_bytes = std::fs::metadata(&path).ok().map(|m| m.len());

                    // Marquer les vidéos pour probe (durée sera remplie plus tard)
                    if is_video {
                        self.pending_probes.insert(path.clone());
                    }

                    Some(DirEntry {
                        path,
                        name,
                        is_dir,
                        is_video,
                        size_bytes,
                        duration_secs: None, // Sera rempli via IPC
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
    #[must_use]
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
    #[must_use]
    pub fn is_selected(&self, path: &Path) -> bool {
        self.selected_files.contains(path)
    }

    /// Obtenir la liste des fichiers sélectionnés (triée)
    #[must_use]
    pub fn get_selected_files(&self) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = self.selected_files.iter().cloned().collect();
        files.sort();
        files
    }

    /// Mettre à jour les informations d'une vidéo (appelé quand le daemon répond)
    pub fn update_video_info(&mut self, path: PathBuf, duration: Option<f64>) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.path == path) {
            entry.duration_secs = duration;
            self.pending_probes.remove(&path);
        }
    }

    /// Obtenir la liste des fichiers vidéo en attente de probe
    #[must_use]
    pub fn get_pending_probes(&self) -> Vec<PathBuf> {
        self.pending_probes.iter().cloned().collect()
    }
}

/// Entrée de répertoire
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub is_video: bool,
    pub size_bytes: Option<u64>,
    pub duration_secs: Option<f64>,
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

/// Formater une durée en secondes au format JJ:HH:MM:SS (toujours complet pour alignement)
pub fn format_duration(seconds: f64) -> String {
    let total_secs = seconds as u64;
    let days = total_secs / 86400;
    let hours = (total_secs % 86400) / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    // Toujours afficher les 4 composantes pour alignement
    format!("{:02}:{:02}:{:02}:{:02}", days, hours, minutes, secs)
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
    /// Graphe VMAF par frame
    VmafGraph(VmafGraphData),
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
    /// Résultat de la détection d'interlacing (None = pas encore détecté)
    pub is_interlaced: Option<bool>,
}

/// Détection synchrone de l'interlacing
fn detect_interlacing_sync(video_path: &Path) -> bool {
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Stream {
        field_order: Option<String>,
        codec_type: Option<String>,
    }

    #[derive(Deserialize)]
    struct Probe {
        streams: Vec<Stream>,
    }

    // Chemin assumé de ffprobe (même logique que daemon)
    let ffprobe_bin = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("encodetalker/deps/bin/ffprobe");

    // Si ffprobe n'existe pas, on assume non-interlacé
    if !ffprobe_bin.exists() {
        return false;
    }

    let output = std::process::Command::new(&ffprobe_bin)
        .arg("-v")
        .arg("quiet")
        .arg("-print_format")
        .arg("json")
        .arg("-show_streams")
        .arg("-select_streams")
        .arg("v:0")
        .arg(video_path)
        .output();

    let Ok(output) = output else { return false };

    let Ok(probe): Result<Probe, _> = serde_json::from_slice(&output.stdout) else {
        return false;
    };

    probe
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("video"))
        .and_then(|s| s.field_order.as_ref())
        .is_some_and(|fo| matches!(fo.as_str(), "tt" | "bb" | "tb" | "bt"))
}

impl EncodeConfigDialog {
    /// Créer dialogue pour un fichier unique
    #[must_use]
    pub fn new(input_path: PathBuf) -> Self {
        Self::new_batch(vec![input_path])
    }

    /// Créer dialogue pour plusieurs fichiers
    #[must_use]
    pub fn new_batch(input_paths: Vec<PathBuf>) -> Self {
        let output_path = if input_paths.len() == 1 {
            let mut out = input_paths[0].clone();
            out.set_extension("");
            PathBuf::from(format!("{}.av1.mkv", out.display()))
        } else {
            PathBuf::from("<auto-generated>")
        };

        let output_path_string = output_path.display().to_string();

        // Détection synchrone de l'interlacing sur le premier fichier
        let is_interlaced = if input_paths.is_empty() {
            None
        } else {
            Some(detect_interlacing_sync(&input_paths[0]))
        };

        Self {
            input_paths,
            output_path,
            output_path_string,
            output_path_cursor: 0,
            is_editing_output: false,
            config: EncodingConfig::default(),
            selected_field: 0,
            is_interlaced,
        }
    }

    /// Est-ce un batch?
    #[must_use]
    pub fn is_batch(&self) -> bool {
        self.input_paths.len() > 1
    }

    pub fn move_field_up(&mut self) {
        if self.selected_field > 0 {
            self.selected_field -= 1;
        }
    }

    pub fn move_field_down(&mut self) {
        // 8 champs : encodeur, audio mode, CRF, preset, threads, VMAF, content type, output path
        if self.selected_field < 7 {
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
