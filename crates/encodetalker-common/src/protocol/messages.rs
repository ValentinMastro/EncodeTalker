use super::super::types::{EncodingConfig, EncodingJob, EncodingStats};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Étape de compilation d'une dépendance
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DepsCompilationStep {
    /// Téléchargement des sources
    Downloading,
    /// Compilation en cours
    Building,
    /// Vérification du binaire
    Verifying,
}

/// Informations sur l'état de compilation des dépendances
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepsStatusInfo {
    /// Toutes les dépendances sont présentes et prêtes
    pub all_present: bool,
    /// Compilation en cours
    pub compiling: bool,
    /// Nom de la dépendance en cours de compilation
    pub current_dep: Option<String>,
    /// Étape actuelle de compilation
    pub current_step: Option<DepsCompilationStep>,
    /// Nombre de dépendances compilées
    pub completed_count: usize,
    /// Nombre total de dépendances
    pub total_count: usize,
}

/// Requête du client vers le daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// ID unique de la requête (pour matching avec la réponse)
    pub id: Uuid,
    /// Payload de la requête
    pub payload: RequestPayload,
}

impl Request {
    pub fn new(payload: RequestPayload) -> Self {
        Self {
            id: Uuid::new_v4(),
            payload,
        }
    }
}

/// Types de requêtes supportées
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestPayload {
    /// Ajouter un job à la queue
    AddJob {
        input_path: PathBuf,
        output_path: PathBuf,
        config: EncodingConfig,
    },
    /// Annuler un job (queued ou running)
    CancelJob { job_id: Uuid },
    /// Retry un job failed
    RetryJob { job_id: Uuid },
    /// Obtenir la liste des jobs en queue
    ListQueue,
    /// Obtenir la liste des jobs actifs (running)
    ListActive,
    /// Obtenir l'historique (completed + failed + cancelled)
    ListHistory,
    /// Obtenir les détails d'un job spécifique
    GetJob { job_id: Uuid },
    /// Obtenir les stats actuelles d'un job running
    GetStats { job_id: Uuid },
    /// Supprimer un job spécifique de l'historique
    RemoveFromHistory { job_id: Uuid },
    /// Clear l'historique
    ClearHistory,
    /// Shutdown graceful du daemon
    Shutdown,
    /// Ping (healthcheck)
    Ping,
    /// Obtenir l'état de compilation des dépendances
    GetDepsStatus,
}

/// Réponse du daemon vers le client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// ID de la requête correspondante
    pub request_id: Uuid,
    /// Payload de la réponse
    pub payload: ResponsePayload,
}

impl Response {
    pub fn new(request_id: Uuid, payload: ResponsePayload) -> Self {
        Self {
            request_id,
            payload,
        }
    }

    pub fn ok(request_id: Uuid) -> Self {
        Self::new(request_id, ResponsePayload::Ok)
    }

    pub fn error(request_id: Uuid, message: String) -> Self {
        Self::new(request_id, ResponsePayload::Error { message })
    }
}

/// Types de réponses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponsePayload {
    /// Succès générique
    Ok,
    /// Erreur
    Error { message: String },
    /// ID d'un job créé
    JobId { job_id: Uuid },
    /// Un job unique
    Job { job: Box<EncodingJob> },
    /// Liste de jobs
    JobList { jobs: Vec<EncodingJob> },
    /// Stats d'un job
    Stats { stats: EncodingStats },
    /// Pong (réponse à Ping)
    Pong,
    /// État de compilation des dépendances
    DepsStatus { status: DepsStatusInfo },
}

/// Événement push du daemon vers les clients (broadcast)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// ID unique de l'événement
    pub id: Uuid,
    /// Timestamp de l'événement
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Payload de l'événement
    pub payload: EventPayload,
}

impl Event {
    pub fn new(payload: EventPayload) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            payload,
        }
    }
}

/// Types d'événements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventPayload {
    /// Job ajouté à la queue
    JobAdded { job_id: Uuid },
    /// Job démarré
    JobStarted { job_id: Uuid },
    /// Progression d'un job
    JobProgress { job_id: Uuid, stats: EncodingStats },
    /// Job terminé avec succès
    JobCompleted { job_id: Uuid },
    /// Job échoué
    JobFailed { job_id: Uuid, error: String },
    /// Job annulé
    JobCancelled { job_id: Uuid },
    /// Daemon en cours de shutdown
    DaemonShutdown,
    /// Compilation des dépendances démarrée
    DepsCompilationStarted {
        /// Nombre total de dépendances à compiler
        total_deps: usize,
    },
    /// Progression de compilation d'une dépendance
    DepsCompilationProgress {
        /// Nom de la dépendance (ex: "FFmpeg", "SVT-AV1-PSY")
        dep_name: String,
        /// Index de la dépendance (0-based)
        dep_index: usize,
        /// Nombre total de dépendances
        total_deps: usize,
        /// Étape actuelle
        step: DepsCompilationStep,
    },
    /// Une dépendance a été compilée avec succès
    DepsCompilationItemCompleted {
        /// Nom de la dépendance
        dep_name: String,
        /// Index de la dépendance
        dep_index: usize,
        /// Nombre total de dépendances
        total_deps: usize,
    },
    /// Toutes les dépendances ont été compilées
    DepsCompilationCompleted,
    /// Erreur lors de la compilation d'une dépendance
    DepsCompilationFailed {
        /// Nom de la dépendance qui a échoué
        dep_name: String,
        /// Message d'erreur
        error: String,
    },
}

/// Message IPC (peut être Request, Response ou Event)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcMessage {
    Request(Request),
    Response(Response),
    Event(Event),
}

impl From<Request> for IpcMessage {
    fn from(req: Request) -> Self {
        IpcMessage::Request(req)
    }
}

impl From<Response> for IpcMessage {
    fn from(resp: Response) -> Self {
        IpcMessage::Response(resp)
    }
}

impl From<Event> for IpcMessage {
    fn from(event: Event) -> Self {
        IpcMessage::Event(event)
    }
}
