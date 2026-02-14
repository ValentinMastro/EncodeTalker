use encodetalker_common::protocol::messages::{DepsCompilationStep, DepsStatusInfo};
use std::sync::{Arc, RwLock};

/// État de compilation des dépendances
#[derive(Debug, Clone)]
#[derive(Default)]
struct DepsCompilationState {
    /// Toutes les dépendances sont présentes
    all_present: bool,
    /// Compilation en cours
    compiling: bool,
    /// Nom de la dépendance en cours de compilation
    current_dep: Option<String>,
    /// Étape actuelle de compilation
    current_step: Option<DepsCompilationStep>,
    /// Nombre de dépendances compilées
    completed_count: usize,
    /// Nombre total de dépendances
    total_count: usize,
}


/// Tracker de compilation des dépendances (thread-safe)
#[derive(Debug, Clone)]
pub struct DepsCompilationTracker {
    state: Arc<RwLock<DepsCompilationState>>,
}

impl DepsCompilationTracker {
    /// Créer un nouveau tracker
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(DepsCompilationState::default())),
        }
    }

    /// Obtenir l'état actuel
    pub fn get_status(&self) -> DepsStatusInfo {
        let state = self.state.read().unwrap();
        DepsStatusInfo {
            all_present: state.all_present,
            compiling: state.compiling,
            current_dep: state.current_dep.clone(),
            current_step: state.current_step.clone(),
            completed_count: state.completed_count,
            total_count: state.total_count,
        }
    }

    /// Marquer toutes les dépendances comme présentes
    pub fn set_all_present(&self) {
        let mut state = self.state.write().unwrap();
        state.all_present = true;
        state.compiling = false;
        state.current_dep = None;
        state.current_step = None;
    }

    /// Démarrer la compilation
    pub fn start_compilation(&self, total_deps: usize) {
        let mut state = self.state.write().unwrap();
        state.all_present = false;
        state.compiling = true;
        state.completed_count = 0;
        state.total_count = total_deps;
        state.current_dep = None;
        state.current_step = None;
    }

    /// Définir la dépendance et l'étape courante
    pub fn set_current(&self, dep_name: String, step: DepsCompilationStep) {
        let mut state = self.state.write().unwrap();
        state.current_dep = Some(dep_name);
        state.current_step = Some(step);
    }

    /// Marquer une dépendance comme complétée
    pub fn complete_dep(&self) {
        let mut state = self.state.write().unwrap();
        state.completed_count += 1;
    }

    /// Terminer la compilation avec succès
    pub fn finish_compilation(&self) {
        let mut state = self.state.write().unwrap();
        state.all_present = true;
        state.compiling = false;
        state.current_dep = None;
        state.current_step = None;
    }

    /// Terminer la compilation avec erreur
    pub fn fail_compilation(&self) {
        let mut state = self.state.write().unwrap();
        state.compiling = false;
        state.current_dep = None;
        state.current_step = None;
    }
}

impl Default for DepsCompilationTracker {
    fn default() -> Self {
        Self::new()
    }
}
