use crate::deps_tracker::DepsCompilationTracker;
use crate::queue::{QueueEvent, QueueManager};
use anyhow::Result;
use encodetalker_common::{
    EncodingJob, Event, EventPayload, IpcMessage, Request, RequestPayload, Response,
    ResponsePayload,
};
use futures::{SinkExt, StreamExt};
use std::path::Path;
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;
use tokio_serde::{formats::Bincode, Framed as SerdeFramed};
use tokio_util::codec::LengthDelimitedCodec;
use tracing::{error, info, warn};

/// Serveur IPC Unix socket
pub struct IpcServer {
    socket_path: std::path::PathBuf,
    queue_manager: Arc<QueueManager>,
    deps_tracker: Arc<DepsCompilationTracker>,
}

impl IpcServer {
    pub fn new(
        socket_path: impl AsRef<Path>,
        queue_manager: Arc<QueueManager>,
        deps_tracker: Arc<DepsCompilationTracker>,
    ) -> Self {
        Self {
            socket_path: socket_path.as_ref().to_path_buf(),
            queue_manager,
            deps_tracker,
        }
    }

    /// Démarrer le serveur IPC (wrapper pour compatibilité)
    pub async fn run(&self, event_rx: mpsc::UnboundedReceiver<QueueEvent>) -> Result<()> {
        self.run_with_listener(None, event_rx).await
    }

    /// Démarrer le serveur IPC avec un listener optionnel déjà créé
    pub async fn run_with_listener(
        &self,
        listener: Option<UnixListener>,
        mut event_rx: mpsc::UnboundedReceiver<QueueEvent>,
    ) -> Result<()> {
        let listener = if let Some(l) = listener {
            info!(
                "Utilisation du listener existant sur {:?}",
                self.socket_path
            );
            l
        } else {
            // Supprimer l'ancien socket s'il existe
            if self.socket_path.exists() {
                std::fs::remove_file(&self.socket_path)?;
            }
            let l = UnixListener::bind(&self.socket_path)?;
            info!("Serveur IPC en écoute sur {:?}", self.socket_path);
            l
        };

        // Channel pour broadcaster les événements à tous les clients
        let (broadcast_tx, _) = tokio::sync::broadcast::channel::<Event>(100);
        let broadcast_tx = Arc::new(broadcast_tx);

        // Tâche pour recevoir les événements de la queue et les broadcaster
        let broadcast_tx_clone = broadcast_tx.clone();
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                let ipc_event = match event {
                    QueueEvent::JobAdded(id) => Event::new(EventPayload::JobAdded { job_id: id }),
                    QueueEvent::JobStarted(id) => {
                        Event::new(EventPayload::JobStarted { job_id: id })
                    }
                    QueueEvent::JobProgress(id, stats) => {
                        Event::new(EventPayload::JobProgress { job_id: id, stats })
                    }
                    QueueEvent::JobCompleted(id) => {
                        Event::new(EventPayload::JobCompleted { job_id: id })
                    }
                    QueueEvent::JobFailed(id, error) => {
                        Event::new(EventPayload::JobFailed { job_id: id, error })
                    }
                    QueueEvent::JobCancelled(id) => {
                        Event::new(EventPayload::JobCancelled { job_id: id })
                    }
                    QueueEvent::DepsCompilationStarted { total_deps } => {
                        Event::new(EventPayload::DepsCompilationStarted { total_deps })
                    }
                    QueueEvent::DepsCompilationProgress {
                        dep_name,
                        dep_index,
                        total_deps,
                        step,
                    } => Event::new(EventPayload::DepsCompilationProgress {
                        dep_name,
                        dep_index,
                        total_deps,
                        step,
                    }),
                    QueueEvent::DepsCompilationItemCompleted {
                        dep_name,
                        dep_index,
                        total_deps,
                    } => Event::new(EventPayload::DepsCompilationItemCompleted {
                        dep_name,
                        dep_index,
                        total_deps,
                    }),
                    QueueEvent::DepsCompilationCompleted => {
                        Event::new(EventPayload::DepsCompilationCompleted)
                    }
                    QueueEvent::DepsCompilationFailed { dep_name, error } => {
                        Event::new(EventPayload::DepsCompilationFailed { dep_name, error })
                    }
                };

                let _ = broadcast_tx_clone.send(ipc_event);
            }
        });

        // Accepter les connexions
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let queue_manager = self.queue_manager.clone();
                    let deps_tracker = self.deps_tracker.clone();
                    let broadcast_rx = broadcast_tx.subscribe();
                    tokio::spawn(async move {
                        if let Err(e) =
                            Self::handle_client(stream, queue_manager, deps_tracker, broadcast_rx)
                                .await
                        {
                            error!("Erreur client: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Erreur d'acceptation de connexion: {}", e);
                }
            }
        }
    }

    /// Gérer une connexion client
    async fn handle_client(
        stream: UnixStream,
        queue_manager: Arc<QueueManager>,
        deps_tracker: Arc<DepsCompilationTracker>,
        mut broadcast_rx: tokio::sync::broadcast::Receiver<Event>,
    ) -> Result<()> {
        info!("Nouveau client connecté");

        // Setup framing avec length-delimited codec
        let length_framed = tokio_util::codec::Framed::new(stream, LengthDelimitedCodec::new());

        // Wrap avec tokio-serde pour bincode
        let framed = SerdeFramed::new(length_framed, Bincode::<IpcMessage, IpcMessage>::default());

        // Split pour lecture et écriture
        let (mut writer, mut reader) = framed.split();

        loop {
            tokio::select! {
                // Recevoir des requêtes du client
                msg = reader.next() => {
                    match msg {
                        Some(Ok(IpcMessage::Request(request))) => {
                            let response = Self::handle_request(&queue_manager, &deps_tracker, request).await;
                            writer.send(IpcMessage::Response(response)).await?;
                        }
                        Some(Ok(_)) => {
                            warn!("Message IPC non-request reçu du client");
                        }
                        Some(Err(e)) => {
                            error!("Erreur de lecture: {}", e);
                            break;
                        }
                        None => {
                            info!("Client déconnecté");
                            break;
                        }
                    }
                }

                // Broadcaster les événements au client
                event = broadcast_rx.recv() => {
                    match event {
                        Ok(event) => {
                            if let Err(e) = writer.send(IpcMessage::Event(event)).await {
                                error!("Échec d'envoi d'événement: {}", e);
                                break;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            warn!("Client en retard sur les événements");
                        }
                        Err(_) => break,
                    }
                }
            }
        }

        Ok(())
    }

    /// Traiter une requête et retourner une réponse
    async fn handle_request(
        queue_manager: &Arc<QueueManager>,
        deps_tracker: &Arc<DepsCompilationTracker>,
        request: Request,
    ) -> Response {
        let request_id = request.id;

        match request.payload {
            RequestPayload::AddJob {
                input_path,
                output_path,
                config,
            } => {
                let job = EncodingJob::new(input_path, output_path, config);
                match queue_manager.add_job(job.clone()).await {
                    Ok(job_id) => Response::new(request_id, ResponsePayload::JobId { job_id }),
                    Err(e) => Response::error(request_id, e.to_string()),
                }
            }

            RequestPayload::CancelJob { job_id } => match queue_manager.cancel_job(job_id).await {
                Ok(()) => Response::ok(request_id),
                Err(e) => Response::error(request_id, e.to_string()),
            },

            RequestPayload::RetryJob { job_id } => match queue_manager.retry_job(job_id).await {
                Ok(()) => Response::ok(request_id),
                Err(e) => Response::error(request_id, e.to_string()),
            },

            RequestPayload::ListQueue => {
                let jobs = queue_manager.get_queue().await;
                Response::new(request_id, ResponsePayload::JobList { jobs })
            }

            RequestPayload::ListActive => {
                let jobs = queue_manager.get_active().await;
                Response::new(request_id, ResponsePayload::JobList { jobs })
            }

            RequestPayload::ListHistory => {
                let jobs = queue_manager.get_history().await;
                Response::new(request_id, ResponsePayload::JobList { jobs })
            }

            RequestPayload::GetJob { job_id } => match queue_manager.get_job(job_id).await {
                Some(job) => Response::new(request_id, ResponsePayload::Job { job: Box::new(job) }),
                None => Response::error(request_id, format!("Job {} non trouvé", job_id)),
            },

            RequestPayload::GetStats { job_id } => match queue_manager.get_job(job_id).await {
                Some(job) => match job.stats {
                    Some(stats) => Response::new(request_id, ResponsePayload::Stats { stats }),
                    None => Response::error(request_id, "Job sans stats".to_string()),
                },
                None => Response::error(request_id, format!("Job {} non trouvé", job_id)),
            },

            RequestPayload::RemoveFromHistory { job_id } => {
                match queue_manager.remove_from_history(job_id).await {
                    Ok(()) => Response::ok(request_id),
                    Err(e) => Response::error(request_id, e.to_string()),
                }
            }

            RequestPayload::ClearHistory => match queue_manager.clear_history().await {
                Ok(()) => Response::ok(request_id),
                Err(e) => Response::error(request_id, e.to_string()),
            },

            RequestPayload::Shutdown => {
                info!("Shutdown demandé par un client");
                // Note: le shutdown réel est géré par le main
                Response::ok(request_id)
            }

            RequestPayload::Ping => Response::new(request_id, ResponsePayload::Pong),

            RequestPayload::GetDepsStatus => {
                let status = deps_tracker.get_status();
                Response::new(request_id, ResponsePayload::DepsStatus { status })
            }
        }
    }
}
