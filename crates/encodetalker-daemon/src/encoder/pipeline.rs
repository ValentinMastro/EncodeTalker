use super::{probe_video, StatsParser, VideoInfo};
use anyhow::{Context, Result};
use encodetalker_common::{AudioMode, EncoderType, EncodingJob, EncodingStats};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::info;

/// Obtenir le nombre de threads disponibles, en u32 pour les encodeurs
///
/// Safe: nombre de cœurs réaliste (8-256) bien inférieur à `u32::MAX`
#[allow(clippy::cast_possible_truncation)]
#[inline]
fn get_available_threads() -> u32 {
    std::thread::available_parallelism().map_or(1, |n| n.get().min(u32::MAX as usize) as u32)
}

// ============================================================================
// Fonctions helper pour refactoring des fonctions too_many_lines
// ============================================================================

/// Spawner un thread OS pour parser stderr ligne par ligne
fn spawn_stderr_parser_thread<F>(
    stderr: std::process::ChildStderr,
    parse_fn: F,
) -> std::thread::JoinHandle<()>
where
    F: Fn(&str) + Send + 'static,
{
    std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            let line = line.trim();
            if !line.is_empty() {
                parse_fn(line);
            }
        }
        tracing::debug!("Lecture stderr terminée");
    })
}

/// Attendre deux processus avec support d'annulation via mpsc
async fn wait_for_processes_with_cancellation(
    ffmpeg_child: std::process::Child,
    encoder_child: std::process::Child,
    cancel_rx: &mut mpsc::UnboundedReceiver<()>,
) -> Result<()> {
    let ffmpeg_arc = std::sync::Arc::new(std::sync::Mutex::new(ffmpeg_child));
    let encoder_arc = std::sync::Arc::new(std::sync::Mutex::new(encoder_child));

    let ffmpeg_clone = ffmpeg_arc.clone();
    let encoder_clone = encoder_arc.clone();

    tokio::select! {
        _ = cancel_rx.recv() => {
            info!("Annulation demandée, arrêt des processus");
            if let Ok(mut encoder) = encoder_arc.lock() {
                let _ = encoder.kill();
            }
            if let Ok(mut ffmpeg) = ffmpeg_arc.lock() {
                let _ = ffmpeg.kill();
            }
            anyhow::bail!("Encodage annulé");
        }
        result = tokio::task::spawn_blocking(move || {
            tracing::debug!("Attente de la fin de l'encodeur...");
            let encoder_status = encoder_clone.lock().unwrap().wait()
                .context("Échec d'attente de l'encodeur")?;
            if !encoder_status.success() {
                anyhow::bail!("L'encodeur a échoué avec le code {:?}", encoder_status.code());
            }
            tracing::debug!("Encodeur terminé avec succès");

            tracing::debug!("Attente de la fin de ffmpeg...");
            let ffmpeg_status = ffmpeg_clone.lock().unwrap().wait()
                .context("Échec d'attente de ffmpeg")?;
            if !ffmpeg_status.success() {
                anyhow::bail!("ffmpeg a échoué avec le code {:?}", ffmpeg_status.code());
            }
            tracing::debug!("ffmpeg terminé avec succès");

            Ok::<(), anyhow::Error>(())
        }) => {
            result??;
        }
    }

    Ok(())
}

/// Attendre un processus avec support d'annulation via mpsc
async fn wait_for_process_with_cancellation(
    child: std::process::Child,
    cancel_rx: &mut mpsc::UnboundedReceiver<()>,
) -> Result<()> {
    let child_arc = std::sync::Arc::new(std::sync::Mutex::new(child));
    let child_clone = child_arc.clone();

    tokio::select! {
        _ = cancel_rx.recv() => {
            info!("Annulation demandée");
            if let Ok(mut child) = child_arc.lock() {
                let _ = child.kill();
            }
            anyhow::bail!("Opération annulée");
        }
        result = tokio::task::spawn_blocking(move || {
            let status = child_clone.lock().unwrap().wait()
                .context("Échec d'attente du processus")?;
            if !status.success() {
                anyhow::bail!("Processus échoué avec le code {:?}", status.code());
            }
            Ok::<(), anyhow::Error>(())
        }) => {
            result??;
        }
    }

    Ok(())
}

/// Construire la commande ffmpeg pour décoder la vidéo
fn build_ffmpeg_decode_command(
    ffmpeg_bin: &Path,
    input: &Path,
    pix_fmt: &str,
    is_interlaced: bool,
) -> std::process::Command {
    let mut cmd = std::process::Command::new(ffmpeg_bin);
    cmd.arg("-nostats")
        .arg("-loglevel")
        .arg("error")
        .arg("-i")
        .arg(input);

    if is_interlaced {
        info!("Application du filtre yadif (désentrelacement)");
        cmd.arg("-vf").arg("yadif");
    }

    cmd.arg("-f")
        .arg("yuv4mpegpipe")
        .arg("-pix_fmt")
        .arg(pix_fmt)
        .arg("-strict")
        .arg("-1")
        .arg("-")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    cmd
}

/// Construire la commande ffmpeg pour calculer le VMAF
fn build_vmaf_command(
    ffmpeg_bin: &Path,
    input_ref: &Path,
    input_distorted: &Path,
    vmaf_log: &Path,
    threads: u32,
    is_interlaced: bool,
) -> std::process::Command {
    let ref_filter = if is_interlaced {
        "[0:v]yadif,setpts=PTS-STARTPTS[ref]"
    } else {
        "[0:v]setpts=PTS-STARTPTS[ref]"
    };

    let vmaf_filter = format!(
        "{ref_filter};[1:v]setpts=PTS-STARTPTS[dist];[dist][ref]libvmaf=n_threads={threads}:n_subsample=1:log_path={}:log_fmt=json",
        vmaf_log.display()
    );

    let mut cmd = std::process::Command::new(ffmpeg_bin);
    cmd.arg("-i")
        .arg(input_ref)
        .arg("-i")
        .arg(input_distorted)
        .arg("-lavfi")
        .arg(&vmaf_filter)
        .arg("-f")
        .arg("null")
        .arg("-")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    cmd
}

/// Parser le fichier JSON VMAF pour extraire les scores
fn parse_vmaf_json_log(vmaf_json: &str) -> Result<(Option<f64>, Option<f64>, Option<f64>)> {
    let vmaf_data: serde_json::Value =
        serde_json::from_str(vmaf_json).context("Échec du parsing JSON VMAF")?;

    let vmaf_metrics = vmaf_data.get("pooled_metrics").and_then(|p| p.get("vmaf"));

    let vmaf_mean = vmaf_metrics
        .and_then(|v| v.get("mean"))
        .and_then(serde_json::Value::as_f64);
    let vmaf_min = vmaf_metrics
        .and_then(|v| v.get("min"))
        .and_then(serde_json::Value::as_f64);
    let vmaf_max = vmaf_metrics
        .and_then(|v| v.get("max"))
        .and_then(serde_json::Value::as_f64);

    Ok((vmaf_mean, vmaf_min, vmaf_max))
}

/// Spawner un thread pour parser stderr de ffmpeg VMAF (byte par byte pour \r)
fn spawn_vmaf_stderr_parser_thread(
    stderr: std::process::ChildStderr,
    total_frames: u64,
    stats_tx: mpsc::UnboundedSender<EncodingStats>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        use std::io::Read;
        let mut reader = BufReader::new(stderr);
        let mut buffer = Vec::new();
        let mut byte = [0u8; 1];

        loop {
            match reader.read(&mut byte) {
                Ok(0) => break,
                Ok(_) => {
                    if byte[0] == b'\r' || byte[0] == b'\n' {
                        if !buffer.is_empty() {
                            if let Ok(line) = String::from_utf8(buffer.clone()) {
                                let line = line.trim();
                                if let Some(frame_str) = line
                                    .strip_prefix("frame=")
                                    .or_else(|| line.find("frame=").map(|pos| &line[pos + 6..]))
                                {
                                    if let Some(frame_num) = frame_str
                                        .split_whitespace()
                                        .next()
                                        .and_then(|s| s.parse::<u64>().ok())
                                    {
                                        let mut stats = EncodingStats {
                                            frame: frame_num,
                                            total_frames: Some(total_frames),
                                            is_calculating_vmaf: true,
                                            ..EncodingStats::default()
                                        };
                                        stats.calculate_progress();
                                        if stats_tx.send(stats).is_err() {
                                            break;
                                        }
                                    }
                                }
                            }
                            buffer.clear();
                        }
                    } else {
                        buffer.push(byte[0]);
                    }
                }
                Err(e) => {
                    tracing::error!("Erreur lecture stderr ffmpeg VMAF: {e}");
                    break;
                }
            }
        }
        tracing::debug!("Lecture stderr ffmpeg VMAF terminée");
    })
}

// ============================================================================
// Pipeline d'encodage
// ============================================================================

/// Pipeline d'encodage complet
pub struct EncodingPipeline {
    ffmpeg_bin: PathBuf,
    ffprobe_bin: PathBuf,
    svt_av1_bin: PathBuf,
    aom_bin: PathBuf,
    precise_frame_count: bool,
}

impl EncodingPipeline {
    #[must_use]
    pub fn new(
        ffmpeg_bin: PathBuf,
        ffprobe_bin: PathBuf,
        svt_av1_bin: PathBuf,
        aom_bin: PathBuf,
        precise_frame_count: bool,
    ) -> Self {
        Self {
            ffmpeg_bin,
            ffprobe_bin,
            svt_av1_bin,
            aom_bin,
            precise_frame_count,
        }
    }

    /// Encoder un job complet
    ///
    /// # Errors
    ///
    /// Retourne une erreur si le probe, l'encodage vidéo, l'encodage audio ou le muxage échoue.
    ///
    /// # Panics
    ///
    /// Peut paniquer si `job.output_path.parent()` retourne `None`.
    pub async fn encode_job(
        &self,
        job: &EncodingJob,
        stats_tx: mpsc::UnboundedSender<EncodingStats>,
        mut cancel_rx: mpsc::UnboundedReceiver<()>,
    ) -> Result<()> {
        info!(
            "Début d'encodage: {} -> {}",
            job.input_path.display(),
            job.output_path.display()
        );

        // 1. Probe du fichier source
        let video_info = probe_video(
            &self.ffprobe_bin,
            &self.ffmpeg_bin,
            &job.input_path,
            self.precise_frame_count,
        )
        .await
        .context("Échec du probe vidéo")?;

        info!(
            "Vidéo: {}x{} @ {:.2} fps, durée: {:?}",
            video_info.width, video_info.height, video_info.fps, video_info.duration
        );

        // 2. Préparer les chemins temporaires
        let temp_dir = job.output_path.parent().unwrap();
        let video_temp = temp_dir.join(format!("{}.ivf", uuid::Uuid::new_v4()));
        let audio_ext = match &job.config.audio_mode {
            AudioMode::Opus { .. } => "opus",
            _ => "mka",
        };
        let audio_temp = temp_dir.join(format!("{}.{}", uuid::Uuid::new_v4(), audio_ext));

        // 3. Encoder la vidéo
        self.encode_video(
            job,
            &video_info,
            &video_temp,
            stats_tx.clone(),
            &mut cancel_rx,
        )
        .await?;

        // 4. Encoder l'audio (en parallèle possible, mais pour simplifier on le fait après)
        self.encode_audio(job, &audio_temp).await?;

        // 5. Muxer le tout
        self.mux_final(job, &video_temp, &audio_temp, &video_info)
            .await?;

        // 6. Nettoyer les fichiers temporaires
        let _ = tokio::fs::remove_file(&video_temp).await;
        let _ = tokio::fs::remove_file(&audio_temp).await;

        // 7. Calculer VMAF si activé
        if job.config.enable_vmaf {
            if let Err(e) = self
                .calculate_vmaf(job, &video_info, stats_tx, &mut cancel_rx)
                .await
            {
                // Ne pas faire échouer le job si le calcul VMAF échoue
                tracing::warn!("Calcul VMAF échoué (l'encodage a réussi): {e}");
            }
        }

        info!(
            "Encodage terminé avec succès: {}",
            job.output_path.display()
        );
        Ok(())
    }

    /// Encoder la piste vidéo (gère automatiquement les 2 passes pour aomenc)
    async fn encode_video(
        &self,
        job: &EncodingJob,
        video_info: &VideoInfo,
        output_path: &Path,
        stats_tx: mpsc::UnboundedSender<EncodingStats>,
        cancel_rx: &mut mpsc::UnboundedReceiver<()>,
    ) -> Result<()> {
        info!("Encodage vidéo avec {:?}", job.config.encoder);

        match job.config.encoder {
            EncoderType::SvtAv1 => {
                let encoder_cmd = self.build_svt_av1_std_command(job, output_path);
                self.run_encode_pass(job, video_info, encoder_cmd, stats_tx, cancel_rx)
                    .await?;
            }
            EncoderType::Aom => {
                let fpf_path = output_path.with_extension("log");

                // Passe 1 : génère les statistiques
                info!("aomenc passe 1/2 : analyse");
                let encoder_cmd =
                    self.build_aom_std_command(job, Path::new("/dev/null"), 1, &fpf_path);
                self.run_encode_pass(job, video_info, encoder_cmd, stats_tx.clone(), cancel_rx)
                    .await?;

                // Passe 2 : encodage final
                info!("aomenc passe 2/2 : encodage");
                let encoder_cmd = self.build_aom_std_command(job, output_path, 2, &fpf_path);
                self.run_encode_pass(job, video_info, encoder_cmd, stats_tx, cancel_rx)
                    .await?;

                // Nettoyer le fichier de stats
                let _ = tokio::fs::remove_file(&fpf_path).await;
            }
        }

        info!("Encodage vidéo terminé avec succès");
        Ok(())
    }

    /// Lancer une passe d'encodage (ffmpeg → encodeur via pipe kernel)
    async fn run_encode_pass(
        &self,
        job: &EncodingJob,
        video_info: &VideoInfo,
        mut encoder_cmd: std::process::Command,
        stats_tx: mpsc::UnboundedSender<EncodingStats>,
        cancel_rx: &mut mpsc::UnboundedReceiver<()>,
    ) -> Result<()> {
        // 1. Construire et spawner ffmpeg
        let mut ffmpeg_cmd = build_ffmpeg_decode_command(
            &self.ffmpeg_bin,
            &job.input_path,
            "yuv420p10le",
            video_info.is_interlaced,
        );
        let mut ffmpeg_child = ffmpeg_cmd.spawn().context("Échec du démarrage de ffmpeg")?;

        let ffmpeg_stdout = ffmpeg_child
            .stdout
            .take()
            .context("Impossible de prendre stdout de ffmpeg")?;
        let ffmpeg_stderr = ffmpeg_child
            .stderr
            .take()
            .context("Impossible de prendre stderr de ffmpeg")?;

        // 2. Spawner l'encodeur avec stdin = ffmpeg_stdout (pipe kernel direct)
        let mut encoder_child = encoder_cmd
            .stdin(Stdio::from(ffmpeg_stdout))
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .context("Échec du démarrage de l'encodeur")?;

        let encoder_stderr = encoder_child
            .stderr
            .take()
            .context("Impossible de prendre stderr de l'encodeur")?;

        // 3. Parser stderr de l'encodeur (byte par byte pour gérer \r)
        let parser = StatsParser::new(video_info.total_frames, video_info.duration);
        let stats_tx_clone = stats_tx.clone();

        let encoder_stderr_handle = std::thread::spawn(move || {
            use std::io::Read;
            let mut reader = BufReader::new(encoder_stderr);
            let mut parser = parser;
            let mut buffer = Vec::new();
            let mut byte = [0u8; 1];

            loop {
                match reader.read(&mut byte) {
                    Ok(0) => break,
                    Ok(_) => {
                        if byte[0] == b'\r' || byte[0] == b'\n' {
                            if !buffer.is_empty() {
                                if let Ok(line) = String::from_utf8(buffer.clone()) {
                                    let line = line.trim();
                                    if !line.is_empty() {
                                        parser.parse_encoder_line(line);
                                        if stats_tx_clone.send(parser.clone_stats()).is_err() {
                                            break;
                                        }
                                    }
                                }
                                buffer.clear();
                            }
                        } else {
                            buffer.push(byte[0]);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Erreur lecture stderr encodeur: {e}");
                        break;
                    }
                }
            }
            tracing::debug!("Lecture stderr encodeur terminée");
        });

        // 4. Drainer stderr de ffmpeg
        let ffmpeg_stderr_handle = spawn_stderr_parser_thread(ffmpeg_stderr, |line| {
            tracing::error!("ffmpeg stderr: {line}");
        });

        // 5. Attendre les processus avec annulation
        wait_for_processes_with_cancellation(ffmpeg_child, encoder_child, cancel_rx).await?;

        // 6. Joindre les threads stderr
        if let Err(e) = encoder_stderr_handle.join() {
            tracing::error!("Échec de jointure du thread stderr encodeur: {e:?}");
        }
        if let Err(e) = ffmpeg_stderr_handle.join() {
            tracing::error!("Échec de jointure du thread stderr ffmpeg: {e:?}");
        }

        Ok(())
    }

    /// Construire la commande SVT-AV1 (`std::process`)
    fn build_svt_av1_std_command(&self, job: &EncodingJob, output: &Path) -> std::process::Command {
        let mut cmd = std::process::Command::new(&self.svt_av1_bin);

        cmd.arg("-i")
            .arg("stdin")
            .arg("--crf")
            .arg(job.config.encoder_params.crf.to_string())
            .arg("--preset")
            .arg(job.config.encoder_params.preset.to_string());

        // Ajouter threads si spécifié
        if let Some(threads) = job.config.encoder_params.threads {
            cmd.arg("--lp").arg(threads.to_string());
        }

        cmd.arg("--progress")
            .arg("2") // Activer la progression sur stderr
            .arg("-b")
            .arg(output);

        // Paramètres spécifiques au type de contenu
        match job.config.encoder_params.content_type {
            encodetalker_common::VideoContentType::GrainedFilm => {
                // Film granuleux : préservation du grain filmique
                cmd.arg("--enable-cdef").arg("0");
                cmd.arg("--enable-restoration").arg("0");
                cmd.arg("--enable-tf").arg("0");
                cmd.arg("--spy-rd").arg("1");
                cmd.arg("--noise-norm-strength").arg("3");
                cmd.arg("--qm-min").arg("10");
                cmd.arg("--tune").arg("0");
                cmd.arg("--qp-scale-compress-strength").arg("3");
                cmd.arg("--scm").arg("0");
                cmd.arg("--psy-rd").arg("4.0");
                cmd.arg("--hbd-mds").arg("1");
            }
            encodetalker_common::VideoContentType::Anime => {
                // Anime : réduction de bruit agressive
                cmd.arg("--qm-min").arg("8");
                cmd.arg("--noise-norm-strength").arg("4");
                cmd.arg("--enable-dlf").arg("2");
            }
            _ => {
                // Default et LiveAction : paramètres standards
                cmd.arg("--qm-min").arg("8");
                cmd.arg("--noise-norm-strength").arg("1");
                cmd.arg("--enable-dlf").arg("2");
            }
        }

        // Ajouter les paramètres extra
        for param in &job.config.encoder_params.extra_params {
            cmd.arg(param);
        }

        cmd
    }

    /// Construire la commande aomenc pour une passe donnée (`std::process`)
    fn build_aom_std_command(
        &self,
        job: &EncodingJob,
        output: &Path,
        pass: u32,
        fpf_path: &Path,
    ) -> std::process::Command {
        let mut cmd = std::process::Command::new(&self.aom_bin);

        cmd.arg(format!("--cq-level={}", job.config.encoder_params.crf))
            .arg(format!("--cpu-used={}", job.config.encoder_params.preset))
            .arg("--end-usage=q")
            .arg("--passes=2")
            .arg(format!("--pass={pass}"))
            .arg(format!("--fpf={}", fpf_path.display()));

        // Ajouter threads (auto-detect si None)
        let threads = job
            .config
            .encoder_params
            .threads
            .unwrap_or_else(get_available_threads);
        cmd.arg(format!("--threads={threads}"));

        // --ivf seulement pour la passe 2 (passe 1 écrit dans /dev/null)
        if pass == 2 {
            cmd.arg("--ivf");
        }

        cmd.arg("-o").arg(output);

        // Ajouter les paramètres extra
        for param in &job.config.encoder_params.extra_params {
            cmd.arg(param);
        }

        // Source stdin en dernier (argument positionnel)
        cmd.arg("-");

        cmd
    }

    /// Encoder l'audio
    async fn encode_audio(&self, job: &EncodingJob, output: &Path) -> Result<()> {
        info!("Encodage audio: {:?}", job.config.audio_mode);

        match &job.config.audio_mode {
            AudioMode::Opus { bitrate } => {
                let mut cmd = Command::new(&self.ffmpeg_bin);
                cmd.arg("-i")
                    .arg(&job.input_path)
                    .arg("-vn") // Pas de vidéo
                    .arg("-c:a")
                    .arg("libopus")
                    .arg("-b:a")
                    .arg(format!("{bitrate}k"));

                // Sélectionner les streams audio spécifiques si configuré
                if let Some(streams) = &job.config.audio_streams {
                    for stream_idx in streams {
                        cmd.arg("-map").arg(format!("0:a:{stream_idx}"));
                    }
                } else {
                    cmd.arg("-map").arg("0:a"); // Tous les streams audio
                }

                cmd.arg(output);

                let output = cmd.output().await.context("Échec de l'encodage audio")?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    anyhow::bail!("Encodage audio échoué: {stderr}");
                }
            }
            AudioMode::Copy => {
                // Copie directe sans ré-encodage
                let mut cmd = Command::new(&self.ffmpeg_bin);
                cmd.arg("-i")
                    .arg(&job.input_path)
                    .arg("-vn")
                    .arg("-c:a")
                    .arg("copy");

                if let Some(streams) = &job.config.audio_streams {
                    for stream_idx in streams {
                        cmd.arg("-map").arg(format!("0:a:{stream_idx}"));
                    }
                } else {
                    cmd.arg("-map").arg("0:a");
                }

                cmd.arg(output);

                let output = cmd.output().await.context("Échec de la copie audio")?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    anyhow::bail!("Copie audio échouée: {stderr}");
                }
            }
            AudioMode::Custom { codec, bitrate } => {
                // Custom codec
                let mut cmd = Command::new(&self.ffmpeg_bin);
                cmd.arg("-i")
                    .arg(&job.input_path)
                    .arg("-vn")
                    .arg("-c:a")
                    .arg(codec)
                    .arg("-b:a")
                    .arg(format!("{bitrate}k"))
                    .arg(output);

                let output = cmd
                    .output()
                    .await
                    .context("Échec de l'encodage audio custom")?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    anyhow::bail!("Encodage audio custom échoué: {stderr}");
                }
            }
        }

        Ok(())
    }

    /// Muxer vidéo + audio + sous-titres dans un MKV final
    async fn mux_final(
        &self,
        job: &EncodingJob,
        video_path: &Path,
        audio_path: &Path,
        video_info: &VideoInfo,
    ) -> Result<()> {
        info!("Muxage final avec ffmpeg");

        let mut cmd = Command::new(&self.ffmpeg_bin);

        // Étape 1: Ajouter TOUS les inputs d'abord
        cmd.arg("-y") // Écraser sans demander
            .arg("-i")
            .arg(video_path) // Input 0: Vidéo AV1
            .arg("-i")
            .arg(audio_path); // Input 1: Audio

        // Ajouter l'input source pour les sous-titres si nécessaire
        if !video_info.subtitle_streams.is_empty() {
            cmd.arg("-i").arg(&job.input_path); // Input 2: Source pour sous-titres
        }

        // Étape 2: Ajouter TOUS les -map ensuite
        cmd.arg("-map")
            .arg("0:v:0") // Vidéo du premier input
            .arg("-map")
            .arg("1:a:0"); // Audio du deuxième input

        if !video_info.subtitle_streams.is_empty() {
            if let Some(streams) = &job.config.subtitle_streams {
                for stream_idx in streams {
                    cmd.arg("-map").arg(format!("2:s:{stream_idx}"));
                }
            } else {
                // Par défaut, copier tous les sous-titres
                cmd.arg("-map").arg("2:s?");
            }
        }

        // Étape 3: Options de codec (copie sans réencodage)
        cmd.arg("-c:v").arg("copy").arg("-c:a").arg("copy");

        if !video_info.subtitle_streams.is_empty() {
            cmd.arg("-c:s").arg("copy");
        }

        // Étape 4: Output MKV
        cmd.arg(&job.output_path);

        let output = cmd.output().await.context("Échec du muxage")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Muxage ffmpeg échoué: {stderr}");
        }

        info!("Muxage réussi");
        Ok(())
    }

    /// Calculer le score VMAF en comparant la source et le fichier encodé frame par frame
    async fn calculate_vmaf(
        &self,
        job: &EncodingJob,
        video_info: &VideoInfo,
        stats_tx: mpsc::UnboundedSender<EncodingStats>,
        cancel_rx: &mut mpsc::UnboundedReceiver<()>,
    ) -> Result<()> {
        info!(
            "Calcul VMAF: {} vs {}",
            job.input_path.display(),
            job.output_path.display()
        );

        // Signaler au TUI que le calcul VMAF commence
        let mut vmaf_stats = EncodingStats {
            is_calculating_vmaf: true,
            total_frames: video_info.total_frames,
            total_duration: video_info.duration,
            ..EncodingStats::default()
        };
        let _ = stats_tx.send(vmaf_stats.clone());

        // Préparer le fichier JSON pour le log VMAF
        let vmaf_log = {
            let stem = job
                .output_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            job.output_path.with_file_name(format!("{stem}_vmaf.json"))
        };

        // Déterminer le nombre de threads
        let threads = job
            .config
            .encoder_params
            .threads
            .unwrap_or_else(get_available_threads);

        // Construire et spawner ffmpeg pour VMAF
        let mut ffmpeg_cmd = build_vmaf_command(
            &self.ffmpeg_bin,
            &job.input_path,
            &job.output_path,
            &vmaf_log,
            threads,
            video_info.is_interlaced,
        );
        let mut ffmpeg_child = ffmpeg_cmd
            .spawn()
            .context("Échec du démarrage de ffmpeg pour VMAF")?;

        let ffmpeg_stderr = ffmpeg_child
            .stderr
            .take()
            .context("Impossible de prendre stderr de ffmpeg VMAF")?;

        // Parser stderr pour la progression
        let stderr_handle = spawn_vmaf_stderr_parser_thread(
            ffmpeg_stderr,
            video_info.total_frames.unwrap_or(0),
            stats_tx.clone(),
        );

        // Attendre avec annulation
        wait_for_process_with_cancellation(ffmpeg_child, cancel_rx).await?;

        // Joindre le thread stderr
        if let Err(e) = stderr_handle.join() {
            tracing::error!("Échec de jointure du thread stderr VMAF: {e:?}");
        }

        // Parser le JSON VMAF
        let vmaf_json = tokio::fs::read_to_string(&vmaf_log)
            .await
            .context("Échec de lecture du fichier JSON VMAF")?;

        let (vmaf_mean, vmaf_min, vmaf_max) = parse_vmaf_json_log(&vmaf_json)?;

        if let Some(mean) = vmaf_mean {
            info!(
                "VMAF score: {mean:.2} (min: {:.2}, max: {:.2})",
                vmaf_min.unwrap_or(0.0),
                vmaf_max.unwrap_or(0.0)
            );
        } else {
            tracing::warn!("Impossible d'extraire le score VMAF du JSON");
        }

        // Envoyer les résultats finaux
        vmaf_stats.is_calculating_vmaf = false;
        vmaf_stats.vmaf_score = vmaf_mean;
        vmaf_stats.vmaf_min = vmaf_min;
        vmaf_stats.vmaf_max = vmaf_max;
        vmaf_stats.vmaf_json_path = Some(vmaf_log.clone());
        vmaf_stats.progress_percent = 100.0;
        let _ = stats_tx.send(vmaf_stats);

        info!(
            "Calcul VMAF terminé, JSON sauvegardé: {}",
            vmaf_log.display()
        );
        Ok(())
    }
}
