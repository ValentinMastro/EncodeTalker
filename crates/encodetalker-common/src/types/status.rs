use serde::{Deserialize, Serialize};

/// Status d'un job d'encodage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    /// Job en attente dans la queue
    Queued,
    /// Job en cours d'exécution
    Running,
    /// Job terminé avec succès
    Completed,
    /// Job échoué
    Failed,
    /// Job annulé par l'utilisateur
    Cancelled,
}

impl JobStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
        )
    }

    pub fn is_active(&self) -> bool {
        matches!(self, JobStatus::Running)
    }
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStatus::Queued => write!(f, "Queued"),
            JobStatus::Running => write!(f, "Running"),
            JobStatus::Completed => write!(f, "Completed"),
            JobStatus::Failed => write!(f, "Failed"),
            JobStatus::Cancelled => write!(f, "Cancelled"),
        }
    }
}
