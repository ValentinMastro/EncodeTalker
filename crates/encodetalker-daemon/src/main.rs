use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::mpsc;
// Ne pas importer Result de anyhow directement à cause de conflits potentiels
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};

use encodetalker_common::AppPaths;
use encodetalker_daemon::{
    DaemonConfig, DepsCompilationTracker, EncodingPipeline, IpcServer, Persistence, QueueManager,
};

/// Vérifie que toutes les dépendances sont installées via le script shell
async fn check_dependencies_installed() -> anyhow::Result<()> {
    // Chemin du script de vérification (à la racine du projet)
    let script_path = std::env::current_exe()?
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .map(|p| p.join("CHECK_INSTALLED_DEPENDENCIES.sh"))
        .ok_or_else(|| anyhow::anyhow!("Cannot determine project root"))?;

    // Si le script n'existe pas, chercher dans le répertoire courant
    let script_path = if !script_path.exists() {
        std::env::current_dir()?.join("CHECK_INSTALLED_DEPENDENCIES.sh")
    } else {
        script_path
    };

    if !script_path.exists() {
        anyhow::bail!(
            "Script CHECK_INSTALLED_DEPENDENCIES.sh not found.\n\
            Please ensure you are running the daemon from the project directory."
        );
    }

    info!("Vérification des dépendances avec le script: {}", script_path.display());

    // Exécuter le script
    let output = Command::new("bash")
        .arg(&script_path)
        .output()
        .map_err(|e| anyhow::anyhow!("Échec de l'exécution du script de vérification: {}", e))?;

    // Vérifier le code de sortie
    if !output.status.success() {
        // Afficher la sortie du script pour que l'utilisateur sache ce qui manque
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        error!("Échec de la vérification des dépendances !");
        error!("{}", stdout);
        if !stderr.is_empty() {
            error!("{}", stderr);
        }

        anyhow::bail!(
            "\n╔════════════════════════════════════════════════════════════════╗\n\
             ║  DÉPENDANCES MANQUANTES                                        ║\n\
             ╠════════════════════════════════════════════════════════════════╣\n\
             ║  Certaines dépendances requises ne sont pas installées.       ║\n\
             ║                                                                ║\n\
             ║  Veuillez d'abord exécuter le script d'installation :         ║\n\
             ║                                                                ║\n\
             ║    ./INSTALL_DEPENDENCIES.sh                                  ║\n\
             ║                                                                ║\n\
             ║  Cela va compiler/télécharger :                               ║\n\
             ║    • FFmpeg (ffmpeg, ffprobe)                                 ║\n\
             ║    • SVT-AV1-PSY (SvtAv1EncApp)                               ║\n\
             ║    • libaom (aomenc)                                          ║\n\
             ║                                                                ║\n\
             ║  Temps estimé : ~60 minutes (Linux) ou ~3 minutes (Windows)   ║\n\
             ╚════════════════════════════════════════════════════════════════╝"
        );
    }

    info!("✓ Toutes les dépendances sont installées");
    Ok(())
}

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

    // Vérifier que les dépendances sont installées (exit si manquantes)
    check_dependencies_installed().await?;

    // Utiliser les binaires depuis le répertoire de dépendances
    let deps_bin = paths.deps_bin_dir.clone();
    #[cfg(unix)]
    let exe_suffix = "";
    #[cfg(windows)]
    let exe_suffix = ".exe";

    let ffmpeg_bin = deps_bin.join(format!("ffmpeg{}", exe_suffix));
    let ffprobe_bin = deps_bin.join(format!("ffprobe{}", exe_suffix));
    let svt_av1_bin = deps_bin.join(format!("SvtAv1EncApp{}", exe_suffix));
    let aomenc_bin = deps_bin.join(format!("aomenc{}", exe_suffix));

    // Créer le pipeline d'encodage
    let pipeline = EncodingPipeline::new(
        ffmpeg_bin,
        ffprobe_bin,
        svt_av1_bin,
        aomenc_bin,
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

    // Créer le tracker de compilation
    let deps_tracker = Arc::new(DepsCompilationTracker::new());

    // Les dépendances sont toutes présentes (vérification faite plus haut)
    deps_tracker.set_all_present();

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
