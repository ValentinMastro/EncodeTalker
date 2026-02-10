use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::mpsc;
// Ne pas importer Result de anyhow directement à cause de conflits potentiels
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};

use encodetalker_common::AppPaths;
use encodetalker_daemon::{DaemonConfig, EncodingPipeline, IpcServer, Persistence, QueueManager};
use encodetalker_deps::DependencyManager;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialiser le logging
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(true)
        .init();

    info!("EncodeTalker Daemon v{}", env!("CARGO_PKG_VERSION"));

    // Créer les chemins de l'application
    let paths = AppPaths::new().map_err(|e| anyhow::anyhow!("{}", e))?;
    paths
        .ensure_dirs_exist()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    info!("Répertoire de données: {:?}", paths.data_dir);

    // Charger la configuration AVANT de compiler les dépendances
    let config = DaemonConfig::load_or_default(&paths.config_file);
    info!("Configuration chargée");

    // Créer le socket Unix immédiatement pour que le TUI puisse se connecter
    // Supprimer l'ancien socket s'il existe
    if paths.socket_path.exists() {
        std::fs::remove_file(&paths.socket_path)?;
    }

    // Bind le socket maintenant
    let listener = tokio::net::UnixListener::bind(&paths.socket_path)?;
    info!("Socket créé sur {:?}", paths.socket_path);

    // Vérifier les dépendances
    info!("Vérification des dépendances...");
    let dep_manager = DependencyManager::new(paths.clone());
    let status = dep_manager.check_status();

    // Créer le pipeline d'encodage (même si les binaires n'existent pas encore)
    let pipeline = EncodingPipeline::new(
        dep_manager.get_binary_path("ffmpeg"),
        dep_manager.get_binary_path("ffprobe"),
        dep_manager.get_binary_path("SvtAv1EncApp"),
        dep_manager.get_binary_path("aomenc"),
        config.encoding.precise_frame_count,
    );

    // Créer la persistance
    let persistence = Persistence::new(paths.state_file.clone());

    // Channel pour les événements de la queue
    let (event_tx, event_rx) = mpsc::unbounded_channel();

    // Créer le queue manager
    let queue_manager = Arc::new(QueueManager::new(
        config.daemon.max_concurrent_jobs,
        pipeline,
        persistence,
        event_tx,
    ));

    // Charger l'état sauvegardé
    if let Err(e) = queue_manager.load_state().await {
        error!("Échec du chargement de l'état: {}", e);
    }

    // Lancer la loop de démarrage de jobs
    let queue_manager_starter = queue_manager.clone();
    let job_starter_task = tokio::spawn(async move {
        queue_manager_starter.run_job_starter().await;
    });

    // Créer le serveur IPC
    let ipc_server = IpcServer::new(&paths.socket_path, queue_manager.clone());

    // Tâche d'auto-save périodique
    let queue_manager_save = queue_manager.clone();
    let auto_save_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            interval.tick().await;
            if let Err(e) = queue_manager_save.save_state().await {
                error!("Échec de l'auto-save: {}", e);
            }
        }
    });

    // Lancer le serveur IPC avec le listener déjà créé
    let ipc_task = tokio::spawn(async move {
        if let Err(e) = ipc_server.run_with_listener(Some(listener), event_rx).await {
            error!("Erreur du serveur IPC: {}", e);
        }
    });

    info!("Daemon démarré, serveur IPC en cours d'exécution");

    // Compiler les dépendances EN ARRIÈRE-PLAN si nécessaire
    if !status.all_present() {
        info!("Dépendances manquantes: {:?}", status.missing());
        info!("Compilation des dépendances en arrière-plan (30-60 minutes)...");

        let paths_clone = paths.clone();
        tokio::spawn(async move {
            let dep_mgr = DependencyManager::new(paths_clone.clone());
            match dep_mgr.ensure_all_deps().await {
                Ok(()) => {
                    info!("✅ Toutes les dépendances sont maintenant compilées et prêtes");
                    info!("Vous pouvez maintenant ajouter des jobs d'encodage");
                }
                Err(e) => {
                    error!("❌ Échec de la compilation des dépendances: {}", e);
                    error!("Veuillez installer les dépendances système requises:");
                    error!("  sudo pacman -S base-devel cmake git nasm ruby libopus libvpx");
                    error!("Le daemon continue de fonctionner mais ne pourra pas encoder");
                }
            }
        });
    } else {
        info!("✅ Toutes les dépendances sont présentes");
    }

    // Attendre le signal de shutdown
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("Signal SIGINT reçu, arrêt graceful...");
        }
        _ = ipc_task => {
            info!("Serveur IPC terminé");
        }
    }

    // Arrêter d'accepter les nouveaux jobs
    queue_manager.stop_accepting_jobs().await;

    // Attendre que les jobs actifs se terminent (timeout 30s)
    info!("Attente de la fin des jobs actifs...");
    queue_manager
        .wait_active_jobs(Duration::from_secs(30))
        .await;

    // Sauvegarder l'état final
    info!("Sauvegarde de l'état final...");
    if let Err(e) = queue_manager.save_state().await {
        error!("Échec de la sauvegarde finale: {}", e);
    }

    // Nettoyer le socket
    if paths.socket_path.exists() {
        let _ = std::fs::remove_file(&paths.socket_path);
    }

    // Arrêter les tâches
    auto_save_task.abort();
    job_starter_task.abort();

    info!("Daemon arrêté proprement");
    anyhow::Ok(())
}
