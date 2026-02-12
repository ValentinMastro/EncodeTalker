use super::{probe_video, StatsParser, VideoInfo};
use anyhow::{Context, Result};
use encodetalker_common::{AudioMode, EncoderType, EncodingJob, EncodingStats};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::info;

/// Pipeline d'encodage complet
pub struct EncodingPipeline {
    ffmpeg_bin: PathBuf,
    ffprobe_bin: PathBuf,
    svt_av1_bin: PathBuf,
    aom_bin: PathBuf,
    precise_frame_count: bool,
}

impl EncodingPipeline {
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
        let audio_temp = temp_dir.join(format!("{}.opus", uuid::Uuid::new_v4()));

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

        info!(
            "Encodage terminé avec succès: {}",
            job.output_path.display()
        );
        Ok(())
    }

    /// Encoder la piste vidéo avec pipe kernel direct (std::process)
    async fn encode_video(
        &self,
        job: &EncodingJob,
        video_info: &VideoInfo,
        output_path: &Path,
        stats_tx: mpsc::UnboundedSender<EncodingStats>,
        cancel_rx: &mut mpsc::UnboundedReceiver<()>,
    ) -> Result<()> {
        info!("Encodage vidéo avec {:?}", job.config.encoder);

        // 1. Spawner ffmpeg avec std::process (stdout piped)
        let mut ffmpeg_child = std::process::Command::new(&self.ffmpeg_bin)
            .arg("-nostats")
            .arg("-loglevel")
            .arg("error")
            .arg("-i")
            .arg(&job.input_path)
            .arg("-f")
            .arg("yuv4mpegpipe")
            .arg("-pix_fmt")
            .arg("yuv420p10le")
            .arg("-strict")
            .arg("-1")
            .arg("-")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Échec du démarrage de ffmpeg")?;

        // 2. Prendre stdout et stderr de ffmpeg
        let ffmpeg_stdout = ffmpeg_child
            .stdout
            .take()
            .context("Impossible de prendre stdout de ffmpeg")?;
        let ffmpeg_stderr = ffmpeg_child
            .stderr
            .take()
            .context("Impossible de prendre stderr de ffmpeg")?;

        // 3. Spawner l'encodeur avec stdin = ffmpeg_stdout (PIPE KERNEL DIRECT)
        let mut encoder_child = match job.config.encoder {
            EncoderType::SvtAv1 => self.build_svt_av1_std_command(job, output_path),
            EncoderType::Aom => self.build_aom_std_command(job, output_path),
        }
        .stdin(Stdio::from(ffmpeg_stdout)) // <-- LE FIX: pipe kernel direct
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("Échec du démarrage de l'encodeur")?;

        let encoder_stderr = encoder_child
            .stderr
            .take()
            .context("Impossible de prendre stderr de l'encodeur")?;

        // 4. Lire stderr de l'encodeur dans un thread OS (pour la progression)
        // Note: SvtAv1EncApp utilise \r pour mettre à jour la même ligne, donc on doit
        // lire octet par octet et splitter sur \r ET \n
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
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        if byte[0] == b'\r' || byte[0] == b'\n' {
                            // Ligne complète, parser
                            if !buffer.is_empty() {
                                if let Ok(line) = String::from_utf8(buffer.clone()) {
                                    let line = line.trim();
                                    if !line.is_empty() {
                                        // Parser la ligne (format SvtAv1EncApp ou aomenc)
                                        parser.parse_encoder_line(line);

                                        // Envoyer les stats via le canal
                                        if let Err(e) = stats_tx_clone.send(parser.clone_stats()) {
                                            tracing::error!("Échec d'envoi des stats: {}", e);
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
                        tracing::error!("Erreur lecture stderr encodeur: {}", e);
                        break;
                    }
                }
            }

            tracing::debug!("Lecture stderr encodeur terminée");
        });

        // 5. Drainer stderr de ffmpeg dans un autre thread OS
        let ffmpeg_stderr_handle = std::thread::spawn(move || {
            let reader = BufReader::new(ffmpeg_stderr);

            for line in reader.lines().map_while(Result::ok) {
                if !line.is_empty() {
                    tracing::error!("ffmpeg stderr: {}", line);
                }
            }

            tracing::debug!("Lecture stderr ffmpeg terminée");
        });

        // 6. Attendre les processus avec possibilité d'annulation
        let encoder_child_arc = std::sync::Arc::new(std::sync::Mutex::new(encoder_child));
        let ffmpeg_child_arc = std::sync::Arc::new(std::sync::Mutex::new(ffmpeg_child));

        let encoder_child_clone = encoder_child_arc.clone();
        let ffmpeg_child_clone = ffmpeg_child_arc.clone();

        tokio::select! {
            _ = cancel_rx.recv() => {
                info!("Annulation demandée, arrêt des processus");

                // Kill les deux processus
                if let Ok(mut encoder) = encoder_child_arc.lock() {
                    let _ = encoder.kill();
                }
                if let Ok(mut ffmpeg) = ffmpeg_child_arc.lock() {
                    let _ = ffmpeg.kill();
                }

                anyhow::bail!("Encodage annulé");
            }
            result = tokio::task::spawn_blocking(move || {
                // Attendre l'encodeur d'abord (il consomme les données)
                tracing::debug!("Attente de la fin de l'encodeur...");
                let encoder_status = encoder_child_clone
                    .lock()
                    .unwrap()
                    .wait()
                    .context("Échec d'attente de l'encodeur")?;

                if !encoder_status.success() {
                    anyhow::bail!("L'encodeur a échoué avec le code {:?}", encoder_status.code());
                }
                tracing::debug!("Encodeur terminé avec succès");

                // Attendre ffmpeg ensuite
                tracing::debug!("Attente de la fin de ffmpeg...");
                let ffmpeg_status = ffmpeg_child_clone
                    .lock()
                    .unwrap()
                    .wait()
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

        // 7. Joindre les threads stderr
        if let Err(e) = encoder_stderr_handle.join() {
            tracing::error!("Échec de jointure du thread stderr encodeur: {:?}", e);
        }
        if let Err(e) = ffmpeg_stderr_handle.join() {
            tracing::error!("Échec de jointure du thread stderr ffmpeg: {:?}", e);
        }

        info!("Encodage vidéo terminé avec succès");
        Ok(())
    }

    /// Construire la commande SVT-AV1 (std::process)
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

        // Ajouter les paramètres extra
        for param in &job.config.encoder_params.extra_params {
            cmd.arg(param);
        }

        cmd
    }

    /// Construire la commande aomenc (std::process)
    fn build_aom_std_command(&self, job: &EncodingJob, output: &Path) -> std::process::Command {
        let mut cmd = std::process::Command::new(&self.aom_bin);

        cmd.arg("-")
            .arg("--cq-level")
            .arg(job.config.encoder_params.crf.to_string())
            .arg("--cpu-used")
            .arg(job.config.encoder_params.preset.to_string())
            .arg("--end-usage=q");

        // Ajouter threads si spécifié
        if let Some(threads) = job.config.encoder_params.threads {
            cmd.arg("--threads").arg(threads.to_string());
        }

        cmd.arg("--ivf").arg("-o").arg(output);

        // Ajouter les paramètres extra
        for param in &job.config.encoder_params.extra_params {
            cmd.arg(param);
        }

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
                    .arg(format!("{}k", bitrate));

                // Sélectionner les streams audio spécifiques si configuré
                if let Some(streams) = &job.config.audio_streams {
                    for stream_idx in streams.iter() {
                        cmd.arg("-map").arg(format!("0:a:{}", stream_idx));
                    }
                } else {
                    cmd.arg("-map").arg("0:a"); // Tous les streams audio
                }

                cmd.arg(output);

                let output = cmd.output().await.context("Échec de l'encodage audio")?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    anyhow::bail!("Encodage audio échoué: {}", stderr);
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
                        cmd.arg("-map").arg(format!("0:a:{}", stream_idx));
                    }
                } else {
                    cmd.arg("-map").arg("0:a");
                }

                cmd.arg(output);

                let output = cmd.output().await.context("Échec de la copie audio")?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    anyhow::bail!("Copie audio échouée: {}", stderr);
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
                    .arg(format!("{}k", bitrate))
                    .arg(output);

                let output = cmd
                    .output()
                    .await
                    .context("Échec de l'encodage audio custom")?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    anyhow::bail!("Encodage audio custom échoué: {}", stderr);
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
                    cmd.arg("-map").arg(format!("2:s:{}", stream_idx));
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
            anyhow::bail!("Muxage ffmpeg échoué: {}", stderr);
        }

        info!("Muxage réussi");
        Ok(())
    }
}
