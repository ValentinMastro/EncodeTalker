use serde::{Deserialize, Serialize};
use uuid::Uuid;
use super::super::types::{EncodingJob, EncodingStats, EncodingConfig};
use std::path::PathBuf;

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
    /// Clear l'historique
    ClearHistory,
    /// Shutdown graceful du daemon
    Shutdown,
    /// Ping (healthcheck)
    Ping,
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
