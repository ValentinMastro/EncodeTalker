use anyhow::{Context, Result};
use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::net::UnixStream;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio_serde::{formats::Bincode, Framed as SerdeFramed};
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tracing::{debug, error, info};
use uuid::Uuid;

use encodetalker_common::{
    EncodingConfig, EncodingJob, Event, IpcMessage, Request, RequestPayload,
    Response, ResponsePayload,
};

/// Client IPC pour communiquer avec le daemon
pub struct IpcClient {
    /// Sender pour envoyer des requêtes
    request_tx: mpsc::UnboundedSender<Request>,
    /// Receiver pour recevoir des événements
    event_rx: Arc<Mutex<mpsc::UnboundedReceiver<Event>>>,
    /// Map des pending responses (par request_id)
    pending_responses: Arc<Mutex<HashMap<Uuid, tokio::sync::oneshot::Sender<Response>>>>,
}

impl IpcClient {
    /// Se connecter au daemon
    pub async fn connect(socket_path: impl AsRef<Path>) -> Result<Self> {
        let stream = UnixStream::connect(socket_path.as_ref())
            .await
            .context("Échec de connexion au daemon")?;

        info!("Connecté au daemon");

        // Setup framing
        let length_framed = Framed::new(stream, LengthDelimitedCodec::new());
        let framed = SerdeFramed::new(length_framed, Bincode::<IpcMessage, IpcMessage>::default());

        // Split pour lecture et écriture
        let (mut writer, mut reader) = framed.split();

        // Channels pour communication interne
        let (request_tx, mut request_rx) = mpsc::unbounded_channel::<Request>();
        let (event_tx, event_rx) = mpsc::unbounded_channel::<Event>();

        let pending_responses: Arc<Mutex<HashMap<Uuid, tokio::sync::oneshot::Sender<Response>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Tâche d'écriture (envoyer les requêtes)
        tokio::spawn(async move {
            while let Some(request) = request_rx.recv().await {
                debug!("Envoi requête: {:?}", request.payload);
                if let Err(e) = writer.send(IpcMessage::Request(request)).await {
                    error!("Échec d'envoi de requête: {}", e);
                    break;
                }
            }
        });

        // Tâche de lecture (recevoir réponses et événements)
        let pending_responses_clone = pending_responses.clone();
        tokio::spawn(async move {
            while let Some(msg) = reader.next().await {
                match msg {
                    Ok(IpcMessage::Response(response)) => {
                        debug!("Réponse reçue pour request_id: {}", response.request_id);

                        // Vérifier si c'est une réponse attendue
                        let mut pending = pending_responses_clone.lock().await;
                        if let Some(tx) = pending.remove(&response.request_id) {
                            let _ = tx.send(response);
                        } else {
                            // Réponse non attendue, on l'ignore
                            debug!("Réponse non attendue pour request_id: {}", response.request_id);
                        }
                    }
                    Ok(IpcMessage::Event(event)) => {
                        debug!("Événement reçu: {:?}", event.payload);
                        let _ = event_tx.send(event);
                    }
                    Ok(IpcMessage::Request(_)) => {
                        error!("Requête reçue côté client (inattendu)");
                    }
                    Err(e) => {
                        error!("Erreur de lecture: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(Self {
            request_tx,
            event_rx: Arc::new(Mutex::new(event_rx)),
            pending_responses,
        })
    }

    /// Envoyer une requête et attendre la réponse
    async fn send_request(&self, payload: RequestPayload) -> Result<Response> {
        let request = Request::new(payload);
        let request_id = request.id;

        // Créer un channel pour recevoir la réponse
        let (tx, rx) = tokio::sync::oneshot::channel();

        // Enregistrer dans pending
        self.pending_responses.lock().await.insert(request_id, tx);

        // Envoyer la requête
        self.request_tx
            .send(request)
            .context("Échec d'envoi de requête")?;

        // Attendre la réponse avec timeout
        let response = tokio::time::timeout(std::time::Duration::from_secs(30), rx)
            .await
            .context("Timeout en attente de réponse")?
            .context("Channel fermé")?;

        Ok(response)
    }

    /// Ajouter un job à la queue
    pub async fn add_job(
        &self,
        input_path: std::path::PathBuf,
        output_path: std::path::PathBuf,
        config: EncodingConfig,
    ) -> Result<Uuid> {
        let response = self
            .send_request(RequestPayload::AddJob {
                input_path,
                output_path,
                config,
            })
            .await?;

        match response.payload {
            ResponsePayload::JobId { job_id } => Ok(job_id),
            ResponsePayload::Error { message } => anyhow::bail!("Erreur: {}", message),
            _ => anyhow::bail!("Réponse inattendue"),
        }
    }

    /// Annuler un job
    pub async fn cancel_job(&self, job_id: Uuid) -> Result<()> {
        let response = self
            .send_request(RequestPayload::CancelJob { job_id })
            .await?;

        match response.payload {
            ResponsePayload::Ok => Ok(()),
            ResponsePayload::Error { message } => anyhow::bail!("Erreur: {}", message),
            _ => anyhow::bail!("Réponse inattendue"),
        }
    }

    /// Retry un job failed
    pub async fn retry_job(&self, job_id: Uuid) -> Result<()> {
        let response = self
            .send_request(RequestPayload::RetryJob { job_id })
            .await?;

        match response.payload {
            ResponsePayload::Ok => Ok(()),
            ResponsePayload::Error { message } => anyhow::bail!("Erreur: {}", message),
            _ => anyhow::bail!("Réponse inattendue"),
        }
    }

    /// Obtenir la liste des jobs en queue
    pub async fn list_queue(&self) -> Result<Vec<EncodingJob>> {
        let response = self.send_request(RequestPayload::ListQueue).await?;

        match response.payload {
            ResponsePayload::JobList { jobs } => Ok(jobs),
            ResponsePayload::Error { message } => anyhow::bail!("Erreur: {}", message),
            _ => anyhow::bail!("Réponse inattendue"),
        }
    }

    /// Obtenir la liste des jobs actifs
    pub async fn list_active(&self) -> Result<Vec<EncodingJob>> {
        let response = self.send_request(RequestPayload::ListActive).await?;

        match response.payload {
            ResponsePayload::JobList { jobs } => Ok(jobs),
            ResponsePayload::Error { message } => anyhow::bail!("Erreur: {}", message),
            _ => anyhow::bail!("Réponse inattendue"),
        }
    }

    /// Obtenir l'historique
    pub async fn list_history(&self) -> Result<Vec<EncodingJob>> {
        let response = self.send_request(RequestPayload::ListHistory).await?;

        match response.payload {
            ResponsePayload::JobList { jobs } => Ok(jobs),
            ResponsePayload::Error { message } => anyhow::bail!("Erreur: {}", message),
            _ => anyhow::bail!("Réponse inattendue"),
        }
    }

    /// Clear l'historique
    pub async fn clear_history(&self) -> Result<()> {
        let response = self.send_request(RequestPayload::ClearHistory).await?;

        match response.payload {
            ResponsePayload::Ok => Ok(()),
            ResponsePayload::Error { message } => anyhow::bail!("Erreur: {}", message),
            _ => anyhow::bail!("Réponse inattendue"),
        }
    }

    /// Ping le daemon
    pub async fn ping(&self) -> Result<()> {
        let response = self.send_request(RequestPayload::Ping).await?;

        match response.payload {
            ResponsePayload::Pong => Ok(()),
            _ => anyhow::bail!("Réponse inattendue"),
        }
    }

    /// Recevoir un événement (non-blocking)
    pub async fn poll_event(&self) -> Option<Event> {
        self.event_rx.lock().await.try_recv().ok()
    }

    /// Rafraîchir toutes les listes
    pub async fn refresh_all(
        &self,
    ) -> Result<(Vec<EncodingJob>, Vec<EncodingJob>, Vec<EncodingJob>)> {
        let queue = self.list_queue().await?;
        let active = self.list_active().await?;
        let history = self.list_history().await?;
        Ok((queue, active, history))
    }
}

/// Démarrer le daemon s'il n'est pas déjà en cours d'exécution
pub async fn ensure_daemon_running(daemon_bin: &Path, socket_path: &Path) -> Result<()> {
    // Vérifier si le socket existe et est accessible
    if socket_path.exists() {
        // Essayer de se connecter
        match UnixStream::connect(socket_path).await {
            Ok(_) => {
                info!("Daemon déjà en cours d'exécution");
                return Ok(());
            }
            Err(_) => {
                // Socket existe mais connexion échoue, supprimer
                let _ = std::fs::remove_file(socket_path);
            }
        }
    }

    info!("Démarrage du daemon...");

    // Lancer le daemon en arrière-plan
    let mut cmd = tokio::process::Command::new(daemon_bin);
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    #[cfg(unix)]
    {
        #[allow(unused_imports)]
        use std::os::unix::process::CommandExt;
        // Démarrer dans un nouveau groupe de processus
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }

    cmd.spawn().context("Échec du démarrage du daemon")?;

    info!("Attente du démarrage du daemon...");
    info!("Note: La première fois, le daemon compile les dépendances (ffmpeg, SVT-AV1, etc.)");
    info!("      Cela peut prendre 30-60 minutes. Veuillez patienter...");

    // Attendre que le socket soit créé (max 3 minutes)
    // La première fois, le daemon compile les dépendances avant de créer le socket
    for i in 0..1800 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        if socket_path.exists() {
            // Attendre un peu plus pour que le daemon soit prêt
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            info!("Daemon démarré et prêt");
            return Ok(());
        }

        // Afficher un message toutes les 10 secondes
        if i % 100 == 0 && i > 0 {
            let seconds = i / 10;
            info!("Attente du daemon... ({} secondes écoulées)", seconds);
        }
    }

    anyhow::bail!("Timeout en attente du démarrage du daemon (3 minutes). Vérifiez les logs dans ~/.local/share/encodetalker/daemon.log");
}
