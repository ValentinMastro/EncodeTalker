use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::mpsc;
// Ne pas importer Result de anyhow directement à cause de conflits potentiels
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};

use encodetalker_common::protocol::messages::DepsCompilationStep;
use encodetalker_common::AppPaths;
use encodetalker_daemon::{
    DaemonConfig, DepsCompilationTracker, EncodingPipeline, IpcServer, Persistence, QueueManager,
};
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

    // ÉTAPE 1: Créer AppPaths par défaut pour trouver config.toml
    let default_paths = AppPaths::new().map_err(|e| anyhow::anyhow!("{}", e))?;
    default_paths
        .ensure_dirs_exist()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // ÉTAPE 2: Charger config.toml (peut contenir [paths] personnalisés)
    let config = DaemonConfig::load_or_default(&default_paths.config_file);
    info!(
        "Configuration chargée depuis {:?}",
        default_paths.config_file
    );

    // ÉTAPE 3: Recréer AppPaths avec la config (chemins personnalisés si définis)
    let paths =
        AppPaths::from_config(Some(config.paths.clone())).map_err(|e| anyhow::anyhow!("{}", e))?;
    paths
        .ensure_dirs_exist()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // ÉTAPE 4: Logger les chemins utilisés
    info!("Chemins utilisés:");
    info!("  - Données:      {:?}", paths.data_dir);
    info!("  - Dépendances:  {:?}", paths.deps_dir);
    info!("  - Socket:       {:?}", paths.socket_path);
    info!("  - Configuration: {:?}", paths.config_file);

    // ÉTAPE 5: Créer le listener IPC immédiatement pour que le TUI puisse se connecter
    use encodetalker_common::ipc::IpcListener;

    IpcListener::cleanup(&paths.socket_path);
    let listener = IpcListener::bind(&paths.socket_path)?;
    info!("Listener IPC créé et en écoute");

    // Vérifier les dépendances
    info!("Vérification des dépendances...");
    info!(
        "Configuration FFmpeg: source = {}",
        config.binaries.ffmpeg_source
    );
    let dep_manager = DependencyManager::new(paths.clone(), config.binaries.clone());
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

    // Cloner event_tx avant de le passer au QueueManager (pour l'utiliser plus tard)
    let event_tx_clone = event_tx.clone();

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

    // Créer le tracker de compilation
    let deps_tracker = Arc::new(DepsCompilationTracker::new());

    // Si toutes les dépendances sont présentes, le marquer
    if status.all_present() {
        deps_tracker.set_all_present();
    }

    // Créer le serveur IPC
    let ipc_server = IpcServer::new(
        &paths.socket_path,
        queue_manager.clone(),
        deps_tracker.clone(),
    );

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
        let deps_tracker_clone = deps_tracker.clone();
        let binaries_config = config.binaries.clone();

        tokio::spawn(async move {
            match compile_deps_with_events(
                paths_clone.clone(),
                event_tx_clone,
                deps_tracker_clone,
                binaries_config,
            )
            .await
            {
                Ok(()) => {
                    info!("✅ Toutes les dépendances sont maintenant compilées et prêtes");
                    info!("Vous pouvez maintenant ajouter des jobs d'encodage");
                }
                Err(e) => {
                    error!("❌ Échec de la compilation des dépendances: {}", e);
                    error!("Veuillez installer les dépendances système requises:");
                    error!("  sudo pacman -S base-devel cmake git nasm");
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

    // Nettoyer le socket (Unix uniquement, Windows Named Pipes se nettoient automatiquement)
    #[cfg(unix)]
    {
        if paths.socket_path.exists() {
            let _ = std::fs::remove_file(&paths.socket_path);
        }
    }

    // Arrêter les tâches
    auto_save_task.abort();
    job_starter_task.abort();

    info!("Daemon arrêté proprement");
    anyhow::Ok(())
}

/// Compiler les dépendances et broadcaster les événements de progression
async fn compile_deps_with_events(
    paths: AppPaths,
    event_tx: mpsc::UnboundedSender<encodetalker_daemon::QueueEvent>,
    tracker: Arc<DepsCompilationTracker>,
    binaries_config: encodetalker_common::BinarySourceSettings,
) -> anyhow::Result<()> {
    use encodetalker_daemon::QueueEvent;
    use encodetalker_deps::{AomBuilder, FFmpegBuilder, SvtAv1Builder};

    // Liste des dépendances à compiler
    let deps_info = [
        (
            "FFmpeg",
            !DependencyManager::new(paths.clone(), binaries_config.clone())
                .check_status()
                .ffmpeg,
        ),
        (
            "SVT-AV1-PSY",
            !DependencyManager::new(paths.clone(), binaries_config.clone())
                .check_status()
                .svt_av1,
        ),
        (
            "libaom",
            !DependencyManager::new(paths.clone(), binaries_config.clone())
                .check_status()
                .aomenc,
        ),
    ];

    let total_deps = deps_info.iter().filter(|(_, missing)| *missing).count();

    if total_deps == 0 {
        tracker.set_all_present();
        return Ok(());
    }

    // Démarrer la compilation
    tracker.start_compilation(total_deps);
    let _ = event_tx.send(QueueEvent::DepsCompilationStarted { total_deps });

    let mut dep_index = 0;

    // Compiler FFmpeg si nécessaire
    if deps_info[0].1 {
        if let Err(e) = compile_single_dep(
            "FFmpeg",
            dep_index,
            total_deps,
            FFmpegBuilder::new(paths.deps_src_dir.clone()),
            &paths,
            &event_tx,
            &tracker,
        )
        .await
        {
            tracker.fail_compilation();
            let _ = event_tx.send(QueueEvent::DepsCompilationFailed {
                dep_name: "FFmpeg".to_string(),
                error: e.to_string(),
            });
            return Err(e);
        }
        dep_index += 1;
    }

    // Compiler SVT-AV1-PSY si nécessaire
    if deps_info[1].1 {
        if let Err(e) = compile_single_dep(
            "SVT-AV1-PSY",
            dep_index,
            total_deps,
            SvtAv1Builder::new(paths.deps_src_dir.clone()),
            &paths,
            &event_tx,
            &tracker,
        )
        .await
        {
            tracker.fail_compilation();
            let _ = event_tx.send(QueueEvent::DepsCompilationFailed {
                dep_name: "SVT-AV1-PSY".to_string(),
                error: e.to_string(),
            });
            return Err(e);
        }
        dep_index += 1;
    }

    // Compiler libaom si nécessaire
    if deps_info[2].1 {
        if let Err(e) = compile_single_dep(
            "libaom",
            dep_index,
            total_deps,
            AomBuilder::new(paths.deps_src_dir.clone()),
            &paths,
            &event_tx,
            &tracker,
        )
        .await
        {
            tracker.fail_compilation();
            let _ = event_tx.send(QueueEvent::DepsCompilationFailed {
                dep_name: "libaom".to_string(),
                error: e.to_string(),
            });
            return Err(e);
        }
    }

    // Compilation terminée
    tracker.finish_compilation();
    let _ = event_tx.send(QueueEvent::DepsCompilationCompleted);

    Ok(())
}

/// Compiler une seule dépendance avec événements
async fn compile_single_dep<B: encodetalker_deps::DependencyBuilder>(
    name: &str,
    dep_index: usize,
    total_deps: usize,
    builder: B,
    paths: &AppPaths,
    event_tx: &mpsc::UnboundedSender<encodetalker_daemon::QueueEvent>,
    tracker: &DepsCompilationTracker,
) -> anyhow::Result<()> {
    use encodetalker_daemon::QueueEvent;

    // Téléchargement
    tracker.set_current(name.to_string(), DepsCompilationStep::Downloading);
    let _ = event_tx.send(QueueEvent::DepsCompilationProgress {
        dep_name: name.to_string(),
        dep_index,
        total_deps,
        step: DepsCompilationStep::Downloading,
    });

    info!("Téléchargement de {}...", name);
    let source_dir = builder.download().await?;

    // Compilation
    tracker.set_current(name.to_string(), DepsCompilationStep::Building);
    let _ = event_tx.send(QueueEvent::DepsCompilationProgress {
        dep_name: name.to_string(),
        dep_index,
        total_deps,
        step: DepsCompilationStep::Building,
    });

    info!("Compilation de {}...", name);
    builder.build(source_dir, paths.deps_dir.clone()).await?;

    // Vérification
    tracker.set_current(name.to_string(), DepsCompilationStep::Verifying);
    let _ = event_tx.send(QueueEvent::DepsCompilationProgress {
        dep_name: name.to_string(),
        dep_index,
        total_deps,
        step: DepsCompilationStep::Verifying,
    });

    info!("Vérification de {}...", name);
    if !builder.verify(&paths.deps_bin_dir) {
        return Err(anyhow::anyhow!(
            "{} compilé mais vérification échouée",
            name
        ));
    }

    // Dépendance terminée
    tracker.complete_dep();
    let _ = event_tx.send(QueueEvent::DepsCompilationItemCompleted {
        dep_name: name.to_string(),
        dep_index,
        total_deps,
    });

    info!("✅ {} compilé avec succès", name);
    Ok(())
}
