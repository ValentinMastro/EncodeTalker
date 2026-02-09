use super::{probe_video, StatsParser, VideoInfo};
use anyhow::{Context, Result};
use encodetalker_common::{AudioMode, EncoderType, EncodingJob, EncodingStats};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::info;

/// Pipeline d'encodage complet
pub struct EncodingPipeline {
    ffmpeg_bin: PathBuf,
    ffprobe_bin: PathBuf,
    svt_av1_bin: PathBuf,
    aom_bin: PathBuf,
}

impl EncodingPipeline {
    pub fn new(
        ffmpeg_bin: PathBuf,
        ffprobe_bin: PathBuf,
        svt_av1_bin: PathBuf,
        aom_bin: PathBuf,
    ) -> Self {
        Self {
            ffmpeg_bin,
            ffprobe_bin,
            svt_av1_bin,
            aom_bin,
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
        let video_info =
            probe_video(&self.ffprobe_bin, &job.input_path).context("Échec du probe vidéo")?;

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

    /// Encoder la piste vidéo
    async fn encode_video(
        &self,
        job: &EncodingJob,
        video_info: &VideoInfo,
        output_path: &Path,
        stats_tx: mpsc::UnboundedSender<EncodingStats>,
        cancel_rx: &mut mpsc::UnboundedReceiver<()>,
    ) -> Result<()> {
        info!("Encodage vidéo avec {:?}", job.config.encoder);

        // Construire la commande ffmpeg (demux + raw video)
        let mut ffmpeg = Command::new(&self.ffmpeg_bin)
            .arg("-i")
            .arg(&job.input_path)
            .arg("-f")
            .arg("yuv4mpegpipe")
            .arg("-pix_fmt")
            .arg("yuv420p")
            .arg("-strict")
            .arg("-1")
            .arg("-")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Échec du démarrage de ffmpeg")?;

        // Construire la commande de l'encodeur
        let mut encoder = match job.config.encoder {
            EncoderType::SvtAv1 => self.build_svt_av1_command(job, output_path),
            EncoderType::Aom => self.build_aom_command(job, output_path),
        }
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("Échec du démarrage de l'encodeur")?;

        // Pipe ffmpeg stdout -> encoder stdin
        let mut ffmpeg_stdout = ffmpeg.stdout.take().unwrap();
        let mut encoder_stdin = encoder.stdin.take().unwrap();

        let pipe_task =
            tokio::spawn(
                async move { tokio::io::copy(&mut ffmpeg_stdout, &mut encoder_stdin).await },
            );

        // Parser stderr de ffmpeg pour les stats
        let ffmpeg_stderr = ffmpeg.stderr.take().unwrap();
        let mut parser = StatsParser::new(video_info.total_frames, video_info.duration);

        let parse_task = tokio::spawn(async move {
            let reader = BufReader::new(ffmpeg_stderr);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                parser.parse_line(&line);
                let _ = stats_tx.send(parser.clone_stats());
            }
        });

        // Attendre fin avec possibilité d'annulation
        tokio::select! {
            result = encoder.wait() => {
                let status = result.context("Échec d'attente de l'encodeur")?;
                if !status.success() {
                    anyhow::bail!("L'encodeur a échoué avec le code {}", status);
                }
            }
            _ = cancel_rx.recv() => {
                info!("Annulation demandée, arrêt des processus");
                let _ = ffmpeg.kill().await;
                let _ = encoder.kill().await;
                anyhow::bail!("Encodage annulé");
            }
        }

        // Attendre que les tâches se terminent
        let _ = pipe_task.await;
        let _ = parse_task.await;

        Ok(())
    }

    /// Construire la commande SVT-AV1
    fn build_svt_av1_command(&self, job: &EncodingJob, output: &Path) -> Command {
        let mut cmd = Command::new(&self.svt_av1_bin);

        cmd.arg("-i")
            .arg("stdin")
            .arg("--crf")
            .arg(job.config.encoder_params.crf.to_string())
            .arg("--preset")
            .arg(job.config.encoder_params.preset.to_string())
            .arg("-b")
            .arg(output);

        // Ajouter les paramètres extra
        for param in &job.config.encoder_params.extra_params {
            cmd.arg(param);
        }

        cmd
    }

    /// Construire la commande aomenc
    fn build_aom_command(&self, job: &EncodingJob, output: &Path) -> Command {
        let mut cmd = Command::new(&self.aom_bin);

        cmd.arg("-")
            .arg("--cq-level")
            .arg(job.config.encoder_params.crf.to_string())
            .arg("--cpu-used")
            .arg(job.config.encoder_params.preset.to_string())
            .arg("--end-usage=q")
            .arg("--ivf")
            .arg("-o")
            .arg(output);

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

        cmd.arg("-y") // Écraser sans demander
            .arg("-i")
            .arg(video_path) // Vidéo AV1
            .arg("-i")
            .arg(audio_path); // Audio

        // Mapper vidéo et audio
        cmd.arg("-map").arg("0:v:0") // Vidéo du premier input
            .arg("-map")
            .arg("1:a:0"); // Audio du deuxième input

        // Ajouter les sous-titres depuis la source si demandé
        if !video_info.subtitle_streams.is_empty() {
            cmd.arg("-i").arg(&job.input_path); // Input source pour sous-titres

            if let Some(streams) = &job.config.subtitle_streams {
                for stream_idx in streams {
                    cmd.arg("-map").arg(format!("2:s:{}", stream_idx));
                }
            } else {
                // Par défaut, copier tous les sous-titres
                cmd.arg("-map").arg("2:s?");
            }

            // Copier les sous-titres sans réencodage
            cmd.arg("-c:s").arg("copy");
        }

        // Copier les streams sans réencodage
        cmd.arg("-c:v").arg("copy").arg("-c:a").arg("copy");

        // Output MKV
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
