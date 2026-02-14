use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};

use encodetalker_common::AppPaths;
use encodetalker_tui::{
    ensure_daemon_running, handle_key_event, render_ui, AppState, InputAction, IpcClient,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialiser le logging
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stderr) // Log vers stderr pour ne pas polluer le TUI
        .init();

    info!("EncodeTalker TUI v{}", env!("CARGO_PKG_VERSION"));

    // Créer les chemins de l'application
    let paths = AppPaths::new()?;
    paths.ensure_dirs_exist()?;

    // Chemin du binaire daemon
    let daemon_bin = std::env::current_exe()?
        .parent()
        .unwrap()
        .join("encodetalker-daemon");

    // S'assurer que le daemon est en cours d'exécution
    info!("Vérification du daemon...");
    if let Err(e) = ensure_daemon_running(&daemon_bin, &paths.socket_path).await {
        eprintln!("Échec du démarrage du daemon: {}", e);
        eprintln!(
            "Assurez-vous que le binaire encodetalker-daemon est présent dans le même répertoire."
        );
        return Err(e);
    }

    // Se connecter au daemon
    info!("Connexion au daemon...");
    let client = IpcClient::connect(&paths.socket_path).await?;

    // Ping pour vérifier la connexion
    client.ping().await?;
    info!("Connecté au daemon avec succès");

    // Vérifier l'état des dépendances
    let deps_status = client.get_deps_status().await?;
    info!(
        "État des dépendances: all_present={}, compiling={}",
        deps_status.all_present, deps_status.compiling
    );

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Créer l'état de l'application
    let start_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));
    let mut app_state = AppState::new(start_dir);

    // Ajuster la vue initiale selon l'état des dépendances
    if deps_status.all_present {
        // Dépendances prêtes, passer directement à FileBrowser
        app_state.current_view = encodetalker_tui::View::FileBrowser;
        app_state.loading_state = None;
    } else if deps_status.compiling {
        // Compilation en cours, rester en Loading et afficher l'état actuel
        app_state.loading_state = Some(encodetalker_tui::LoadingState::from_status(deps_status));
    }
    // Sinon, rester en Loading avec état vide (en attente du démarrage de la compilation)

    // Charger les listes initiales
    match client.refresh_all().await {
        Ok((queue, active, history)) => {
            app_state.queue_jobs = queue;
            app_state.active_jobs = active;
            app_state.history_jobs = history;
        }
        Err(e) => {
            error!("Échec du chargement initial: {}", e);
        }
    }

    // Boucle principale
    let tick_rate = Duration::from_millis(500); // Rafraîchir toutes les 500ms
    let mut last_tick = std::time::Instant::now();

    loop {
        // Rendre l'interface
        terminal.draw(|f| render_ui(f, &app_state))?;

        // Gérer les événements
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                // Gérer l'événement clavier
                let action = handle_key_event(&mut app_state, key);

                // Traiter l'action
                match action {
                    InputAction::None => {}
                    InputAction::RefreshLists => {
                        if let Ok((queue, active, history)) = client.refresh_all().await {
                            app_state.queue_jobs = queue;
                            app_state.active_jobs = active;
                            app_state.history_jobs = history;
                        }
                    }
                    InputAction::AddJob {
                        input_path,
                        output_path,
                        config,
                    } => {
                        match client
                            .add_job(input_path.clone(), output_path, config)
                            .await
                        {
                            Ok(job_id) => {
                                app_state.set_status(format!("Job {} ajouté", job_id));
                                // Rafraîchir les listes
                                if let Ok((queue, active, history)) = client.refresh_all().await {
                                    app_state.queue_jobs = queue;
                                    app_state.active_jobs = active;
                                    app_state.history_jobs = history;
                                }
                            }
                            Err(e) => {
                                app_state.dialog = Some(encodetalker_tui::Dialog::Error {
                                    message: format!("Échec de l'ajout du job: {}", e),
                                });
                            }
                        }
                    }
                    InputAction::AddBatchJobs { jobs, config } => {
                        let total = jobs.len();
                        let mut success_count = 0;
                        let mut errors = Vec::new();

                        for (input_path, output_path) in jobs {
                            match client
                                .add_job(input_path.clone(), output_path, config.clone())
                                .await
                            {
                                Ok(_job_id) => {
                                    success_count += 1;
                                }
                                Err(e) => {
                                    let filename = input_path
                                        .file_name()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .to_string();
                                    errors.push(format!("{}: {}", filename, e));
                                }
                            }
                        }

                        if success_count == total {
                            app_state.set_status(format!("{} jobs ajoutés avec succès", total));
                        } else {
                            app_state.dialog = Some(encodetalker_tui::Dialog::Error {
                                message: format!(
                                    "{}/{} jobs ajoutés. Échecs:\n{}",
                                    success_count,
                                    total,
                                    errors.join("\n")
                                ),
                            });
                        }

                        // Rafraîchir les listes
                        if let Ok((queue, active, history)) = client.refresh_all().await {
                            app_state.queue_jobs = queue;
                            app_state.active_jobs = active;
                            app_state.history_jobs = history;
                        }
                    }
                    InputAction::CancelJob { job_id } => {
                        match client.cancel_job(job_id).await {
                            Ok(()) => {
                                app_state.set_status(format!("Job {} annulé", job_id));
                                // Rafraîchir les listes
                                if let Ok((queue, active, history)) = client.refresh_all().await {
                                    app_state.queue_jobs = queue;
                                    app_state.active_jobs = active;
                                    app_state.history_jobs = history;
                                }
                            }
                            Err(e) => {
                                app_state.dialog = Some(encodetalker_tui::Dialog::Error {
                                    message: format!("Échec de l'annulation: {}", e),
                                });
                            }
                        }
                    }
                    InputAction::RetryJob { job_id } => {
                        match client.retry_job(job_id).await {
                            Ok(()) => {
                                app_state.set_status(format!("Job {} relancé", job_id));
                                // Rafraîchir les listes
                                if let Ok((queue, active, history)) = client.refresh_all().await {
                                    app_state.queue_jobs = queue;
                                    app_state.active_jobs = active;
                                    app_state.history_jobs = history;
                                }
                            }
                            Err(e) => {
                                app_state.dialog = Some(encodetalker_tui::Dialog::Error {
                                    message: format!("Échec du retry: {}", e),
                                });
                            }
                        }
                    }
                    InputAction::RemoveFromHistory { job_id } => {
                        match client.remove_from_history(job_id).await {
                            Ok(()) => {
                                app_state.set_status("Tâche supprimée de l'historique");
                                app_state.history_jobs.retain(|j| j.id != job_id);
                                // Ajuster l'index si nécessaire
                                if app_state.selected_index >= app_state.history_jobs.len()
                                    && app_state.selected_index > 0
                                {
                                    app_state.selected_index -= 1;
                                }
                            }
                            Err(e) => {
                                app_state.dialog = Some(encodetalker_tui::Dialog::Error {
                                    message: format!("Échec de la suppression: {}", e),
                                });
                            }
                        }
                    }
                    InputAction::ClearHistory => match client.clear_history().await {
                        Ok(()) => {
                            app_state.set_status("Historique effacé");
                            app_state.history_jobs.clear();
                        }
                        Err(e) => {
                            app_state.dialog = Some(encodetalker_tui::Dialog::Error {
                                message: format!("Échec du clear: {}", e),
                            });
                        }
                    },
                }
            }
        }

        // Tick périodique
        if last_tick.elapsed() >= tick_rate {
            last_tick = std::time::Instant::now();

            // Recevoir les événements du daemon
            while let Some(event) = client.poll_event().await {
                match event.payload {
                    encodetalker_common::EventPayload::JobAdded { .. }
                    | encodetalker_common::EventPayload::JobStarted { .. }
                    | encodetalker_common::EventPayload::JobCompleted { .. }
                    | encodetalker_common::EventPayload::JobFailed { .. }
                    | encodetalker_common::EventPayload::JobCancelled { .. } => {
                        // Rafraîchir les listes
                        if let Ok((queue, active, history)) = client.refresh_all().await {
                            app_state.queue_jobs = queue;
                            app_state.active_jobs = active;
                            app_state.history_jobs = history;
                        }
                    }
                    encodetalker_common::EventPayload::JobProgress { job_id, stats } => {
                        // Mettre à jour les stats du job
                        if let Some(job) = app_state.active_jobs.iter_mut().find(|j| j.id == job_id)
                        {
                            job.stats = Some(stats);
                        }
                    }
                    encodetalker_common::EventPayload::DaemonShutdown => {
                        app_state.dialog = Some(encodetalker_tui::Dialog::Error {
                            message: "Le daemon s'est arrêté".to_string(),
                        });
                    }
                    // Événements de compilation des dépendances
                    encodetalker_common::EventPayload::DepsCompilationStarted { total_deps } => {
                        info!(
                            "Compilation des dépendances démarrée ({} dépendances)",
                            total_deps
                        );
                        let mut loading = encodetalker_tui::LoadingState::new();
                        loading.total_deps = total_deps;
                        app_state.loading_state = Some(loading);
                        app_state.current_view = encodetalker_tui::View::Loading;
                    }
                    encodetalker_common::EventPayload::DepsCompilationProgress {
                        dep_name,
                        step,
                        ..
                    } => {
                        if let Some(loading) = &mut app_state.loading_state {
                            loading.current_dep = Some(dep_name.clone());
                            loading.current_step = Some(step);
                        }
                    }
                    encodetalker_common::EventPayload::DepsCompilationItemCompleted { .. } => {
                        if let Some(loading) = &mut app_state.loading_state {
                            loading.completed_deps += 1;
                        }
                    }
                    encodetalker_common::EventPayload::DepsCompilationCompleted => {
                        info!("Compilation des dépendances terminée avec succès");
                        // Attendre 2 secondes pour afficher "✅ Prêt !"
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        // Basculer vers FileBrowser
                        app_state.current_view = encodetalker_tui::View::FileBrowser;
                        app_state.loading_state = None;
                        app_state.set_status("✅ Dépendances compilées avec succès");
                    }
                    encodetalker_common::EventPayload::DepsCompilationFailed {
                        dep_name,
                        error,
                    } => {
                        error!("Échec de compilation de {}: {}", dep_name, error);
                        if let Some(loading) = &mut app_state.loading_state {
                            loading.error = Some(format!("{}: {}", dep_name, error));
                        }
                    }
                }
            }

            // Effacer le message de status après 3 secondes
            // (simplifié ici, pourrait utiliser un timestamp)
        }

        // Quitter ?
        if app_state.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    info!("TUI fermé");
    Ok(())
}
