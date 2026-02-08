use super::{PersistedState, Persistence};
use crate::encoder::EncodingPipeline;
use anyhow::Result;
use encodetalker_common::{EncodingJob, EncodingStats, JobStatus};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{error, info, warn};
use uuid::Uuid;

/// Événement interne de la queue
#[derive(Debug, Clone)]
pub enum QueueEvent {
    JobAdded(Uuid),
    JobStarted(Uuid),
    JobProgress(Uuid, EncodingStats),
    JobCompleted(Uuid),
    JobFailed(Uuid, String),
    JobCancelled(Uuid),
}

/// Contrôle d'un job en cours
struct ActiveJobControl {
    cancel_tx: mpsc::UnboundedSender<()>,
}

/// Gestionnaire de queue d'encodage
pub struct QueueManager {
    /// Queue d'attente
    queue: Arc<RwLock<VecDeque<EncodingJob>>>,
    /// Jobs actifs (en cours d'encodage)
    active: Arc<RwLock<HashMap<Uuid, EncodingJob>>>,
    /// Historique (completed + failed + cancelled)
    history: Arc<RwLock<Vec<EncodingJob>>>,
    /// Contrôles des jobs actifs
    active_controls: Arc<Mutex<HashMap<Uuid, ActiveJobControl>>>,
    /// Nombre maximum de jobs simultanés
    max_concurrent: usize,
    /// Channel pour les événements
    event_tx: mpsc::UnboundedSender<QueueEvent>,
    /// Pipeline d'encodage
    pipeline: Arc<EncodingPipeline>,
    /// Persistance
    persistence: Arc<Persistence>,
    /// Flag pour arrêt
    accepting_jobs: Arc<RwLock<bool>>,
    /// Notify pour démarrage de jobs
    start_notify: Arc<tokio::sync::Notify>,
}

impl QueueManager {
    pub fn new(
        max_concurrent: usize,
        pipeline: EncodingPipeline,
        persistence: Persistence,
        event_tx: mpsc::UnboundedSender<QueueEvent>,
    ) -> Self {
        Self {
            queue: Arc::new(RwLock::new(VecDeque::new())),
            active: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(Vec::new())),
            active_controls: Arc::new(Mutex::new(HashMap::new())),
            max_concurrent,
            event_tx,
            pipeline: Arc::new(pipeline),
            persistence: Arc::new(persistence),
            accepting_jobs: Arc::new(RwLock::new(true)),
            start_notify: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// Charger l'état depuis le disque
    pub async fn load_state(&self) -> Result<()> {
        let state = self.persistence.load().await?;

        let mut queue = self.queue.write().await;
        *queue = state.queue;

        // Les jobs actifs sont remis en queue car on ne peut pas les reprendre mid-encoding
        for mut job in state.active {
            job.status = JobStatus::Queued;
            job.stats = None;
            queue.push_back(job);
        }

        let mut history = self.history.write().await;
        *history = state.history;

        Ok(())
    }

    /// Sauvegarder l'état sur le disque
    pub async fn save_state(&self) -> Result<()> {
        let state = PersistedState {
            queue: self.queue.read().await.clone(),
            active: self.active.read().await.values().cloned().collect(),
            history: self.history.read().await.clone(),
        };

        self.persistence.save(&state).await
    }

    /// Ajouter un job à la queue
    pub async fn add_job(&self, mut job: EncodingJob) -> Result<Uuid> {
        if !*self.accepting_jobs.read().await {
            anyhow::bail!("Le daemon n'accepte plus de nouveaux jobs");
        }

        job.status = JobStatus::Queued;
        let job_id = job.id;

        self.queue.write().await.push_back(job);

        info!("Job {} ajouté à la queue", job_id);
        let _ = self.event_tx.send(QueueEvent::JobAdded(job_id));

        // Notifier pour démarrage
        self.start_notify.notify_one();

        Ok(job_id)
    }

    /// Annuler un job
    pub async fn cancel_job(&self, job_id: Uuid) -> Result<()> {
        // Vérifier si c'est un job actif
        if self.active.read().await.contains_key(&job_id) {
            // Envoyer signal d'annulation
            let controls = self.active_controls.lock().await;
            if let Some(control) = controls.get(&job_id) {
                let _ = control.cancel_tx.send(());
                info!("Signal d'annulation envoyé au job {}", job_id);
                return Ok(());
            }
        }

        // Sinon chercher dans la queue
        let mut queue = self.queue.write().await;
        if let Some(pos) = queue.iter().position(|j| j.id == job_id) {
            let mut job = queue.remove(pos).unwrap();
            job.mark_cancelled();

            self.history.write().await.push(job);

            info!("Job {} retiré de la queue", job_id);
            let _ = self.event_tx.send(QueueEvent::JobCancelled(job_id));
            return Ok(());
        }

        anyhow::bail!("Job {} non trouvé", job_id);
    }

    /// Retry un job failed
    pub async fn retry_job(&self, job_id: Uuid) -> Result<()> {
        let mut history = self.history.write().await;

        if let Some(pos) = history
            .iter()
            .position(|j| j.id == job_id && j.status == JobStatus::Failed)
        {
            let mut job = history.remove(pos);
            job.status = JobStatus::Queued;
            job.error_message = None;
            job.stats = None;
            job.started_at = None;
            job.finished_at = None;

            self.queue.write().await.push_back(job);

            info!("Job {} remis en queue", job_id);
            let _ = self.event_tx.send(QueueEvent::JobAdded(job_id));

            self.start_notify.notify_one();
            return Ok(());
        }

        anyhow::bail!("Job {} non trouvé ou non failed", job_id);
    }

    /// Obtenir la queue
    pub async fn get_queue(&self) -> Vec<EncodingJob> {
        self.queue.read().await.iter().cloned().collect()
    }

    /// Obtenir les jobs actifs
    pub async fn get_active(&self) -> Vec<EncodingJob> {
        self.active.read().await.values().cloned().collect()
    }

    /// Obtenir l'historique
    pub async fn get_history(&self) -> Vec<EncodingJob> {
        self.history.read().await.clone()
    }

    /// Clear l'historique
    pub async fn clear_history(&self) -> Result<()> {
        self.history.write().await.clear();
        info!("Historique nettoyé");
        Ok(())
    }

    /// Obtenir un job spécifique
    pub async fn get_job(&self, job_id: Uuid) -> Option<EncodingJob> {
        // Chercher dans queue
        if let Some(job) = self.queue.read().await.iter().find(|j| j.id == job_id) {
            return Some(job.clone());
        }

        // Chercher dans active
        if let Some(job) = self.active.read().await.get(&job_id) {
            return Some(job.clone());
        }

        // Chercher dans history
        if let Some(job) = self.history.read().await.iter().find(|j| j.id == job_id) {
            return Some(job.clone());
        }

        None
    }

    /// Lancer la loop de démarrage de jobs (à appeler dans une tâche séparée)
    pub async fn run_job_starter(self: Arc<Self>) {
        loop {
            // Attendre une notification
            self.start_notify.notified().await;

            // Essayer de démarrer des jobs
            loop {
                let active_count = self.active.read().await.len();

                if active_count >= self.max_concurrent {
                    break;
                }

                let job = {
                    let mut queue = self.queue.write().await;
                    queue.pop_front()
                };

                if let Some(job) = job {
                    self.start_job(job).await;
                } else {
                    break;
                }
            }
        }
    }

    /// Démarrer un job
    async fn start_job(&self, mut job: EncodingJob) {
        job.mark_started();
        let job_id = job.id;

        self.active.write().await.insert(job_id, job.clone());

        info!("Démarrage du job {}", job_id);
        let _ = self.event_tx.send(QueueEvent::JobStarted(job_id));

        // Créer les channels de contrôle
        let (cancel_tx, cancel_rx) = mpsc::unbounded_channel::<()>();
        let (stats_tx, mut stats_rx) = mpsc::unbounded_channel::<EncodingStats>();

        // Stocker le contrôle
        self.active_controls
            .lock()
            .await
            .insert(job_id, ActiveJobControl { cancel_tx });

        // Clone des ressources pour la tâche
        let pipeline = self.pipeline.clone();
        let active = self.active.clone();
        let history = self.history.clone();
        let active_controls = self.active_controls.clone();
        let event_tx = self.event_tx.clone();
        let start_notify = self.start_notify.clone();

        // Lancer l'encodage dans une tâche
        tokio::spawn(async move {
            // Task pour propager les stats
            let stats_job_id = job_id;
            let stats_event_tx = event_tx.clone();
            let stats_active = active.clone();
            tokio::spawn(async move {
                while let Some(stats) = stats_rx.recv().await {
                    // Mettre à jour les stats dans le job actif
                    if let Some(job) = stats_active.write().await.get_mut(&stats_job_id) {
                        job.stats = Some(stats.clone());
                    }
                    let _ = stats_event_tx.send(QueueEvent::JobProgress(stats_job_id, stats));
                }
            });

            // Lancer le pipeline
            let result = pipeline.encode_job(&job, stats_tx, cancel_rx).await;

            // Nettoyer le contrôle
            active_controls.lock().await.remove(&job_id);

            // Retirer des actifs
            let mut job = active.write().await.remove(&job_id).unwrap();

            // Traiter le résultat
            match result {
                Ok(()) => {
                    job.mark_completed();
                    info!("Job {} terminé avec succès", job_id);
                    let _ = event_tx.send(QueueEvent::JobCompleted(job_id));
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    job.mark_failed(error_msg.clone());
                    error!("Job {} échoué: {}", job_id, error_msg);
                    let _ = event_tx.send(QueueEvent::JobFailed(job_id, error_msg));
                }
            }

            // Ajouter à l'historique
            history.write().await.push(job);

            // Notifier pour démarrer le prochain job
            start_notify.notify_one();
        });
    }

    /// Arrêter d'accepter les nouveaux jobs
    pub async fn stop_accepting_jobs(&self) {
        *self.accepting_jobs.write().await = false;
        info!("Le daemon n'accepte plus de nouveaux jobs");
    }

    /// Attendre que tous les jobs actifs se terminent (avec timeout)
    pub async fn wait_active_jobs(&self, timeout: std::time::Duration) {
        let start = std::time::Instant::now();

        loop {
            let active_count = self.active.read().await.len();
            if active_count == 0 {
                info!("Tous les jobs actifs sont terminés");
                break;
            }

            if start.elapsed() > timeout {
                warn!("Timeout atteint, {} jobs encore actifs", active_count);
                break;
            }

            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }
}
