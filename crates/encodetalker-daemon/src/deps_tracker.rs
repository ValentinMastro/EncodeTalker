use encodetalker_common::protocol::messages::{DepsCompilationStep, DepsStatusInfo};
use std::sync::{Arc, RwLock};

/// État de compilation des dépendances
#[derive(Debug, Clone, Default)]
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
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(DepsCompilationState::default())),
        }
    }

    /// Obtenir l'état actuel
    ///
    /// # Panics
    ///
    /// Peut paniquer si le lock est empoisonné (thread panic pendant le write).
    #[must_use]
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
    ///
    /// # Panics
    ///
    /// Peut paniquer si le lock est empoisonné (thread panic pendant le write).
    pub fn set_all_present(&self) {
        let mut state = self.state.write().unwrap();
        state.all_present = true;
        state.compiling = false;
        state.current_dep = None;
        state.current_step = None;
    }

    /// Démarrer la compilation
    ///
    /// # Panics
    ///
    /// Peut paniquer si le lock est empoisonné (thread panic pendant le write).
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
    ///
    /// # Panics
    ///
    /// Peut paniquer si le lock est empoisonné (thread panic pendant le write).
    pub fn set_current(&self, dep_name: String, step: DepsCompilationStep) {
        let mut state = self.state.write().unwrap();
        state.current_dep = Some(dep_name);
        state.current_step = Some(step);
    }

    /// Marquer une dépendance comme complétée
    ///
    /// # Panics
    ///
    /// Peut paniquer si le lock est empoisonné (thread panic pendant le write).
    pub fn complete_dep(&self) {
        let mut state = self.state.write().unwrap();
        state.completed_count += 1;
    }

    /// Terminer la compilation avec succès
    ///
    /// # Panics
    ///
    /// Peut paniquer si le lock est empoisonné (thread panic pendant le write).
    pub fn finish_compilation(&self) {
        let mut state = self.state.write().unwrap();
        state.all_present = true;
        state.compiling = false;
        state.current_dep = None;
        state.current_step = None;
    }

    /// Terminer la compilation avec erreur
    ///
    /// # Panics
    ///
    /// Peut paniquer si le lock est empoisonné (thread panic pendant le write).
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
