use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;

/// Informations sur le fichier vidéo source
#[derive(Debug, Clone)]
pub struct VideoInfo {
    pub duration: Option<Duration>,
    pub total_frames: Option<u64>,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub audio_streams: Vec<AudioStreamInfo>,
    pub subtitle_streams: Vec<SubtitleStreamInfo>,
}

#[derive(Debug, Clone)]
pub struct AudioStreamInfo {
    pub index: usize,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SubtitleStreamInfo {
    pub index: usize,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
}

/// Sortie JSON de ffprobe
#[derive(Debug, Deserialize)]
struct FFProbeOutput {
    format: FFProbeFormat,
    streams: Vec<FFProbeStream>,
}

#[derive(Debug, Deserialize)]
struct FFProbeFormat {
    duration: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FFProbeStream {
    index: u32,
    codec_type: String,
    codec_name: String,
    width: Option<u32>,
    height: Option<u32>,
    r_frame_rate: Option<String>,
    nb_frames: Option<String>,
    tags: Option<FFProbeTags>,
}

#[derive(Debug, Deserialize)]
struct FFProbeTags {
    language: Option<String>,
    title: Option<String>,
}

/// Compter précisément les frames via ffmpeg -c copy -f null
/// ATTENTION: LENT (lit tout le fichier vidéo)
async fn count_frames_precisely(ffmpeg_bin: &Path, input: &Path) -> Result<u64> {
    use regex::Regex;
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;

    tracing::info!("Comptage précis des frames (peut prendre du temps)...");

    let mut child = Command::new(ffmpeg_bin)
        .arg("-i")
        .arg(input)
        .arg("-map")
        .arg("0:v:0")
        .arg("-c")
        .arg("copy")
        .arg("-f")
        .arg("null")
        .arg("-")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Échec du comptage de frames")?;

    let stderr = child.stderr.take().unwrap();
    let reader = BufReader::new(stderr);
    let mut lines = reader.lines();

    let frame_regex = Regex::new(r"frame=\s*(\d+)").unwrap();
    let mut last_frame = 0u64;

    // NOUVEAU: Timeout global de 5 minutes
    let count_task = async {
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(caps) = frame_regex.captures(&line) {
                if let Ok(frame) = caps[1].parse::<u64>() {
                    last_frame = frame;
                }
            }
        }
        last_frame
    };

    let last_frame = match tokio::time::timeout(Duration::from_secs(300), count_task).await {
        Ok(frames) => frames,
        Err(_) => {
            tracing::warn!("Timeout comptage frames (5 min), arrêt du processus");
            let _ = child.kill().await;
            anyhow::bail!("Timeout comptage précis des frames");
        }
    };

    child.wait().await?;

    tracing::info!("Comptage précis terminé: {} frames", last_frame);
    Ok(last_frame)
}

/// Prober un fichier vidéo avec ffprobe
pub async fn probe_video(
    ffprobe_bin: &Path,
    ffmpeg_bin: &Path,
    input: &Path,
    precise_count: bool,
) -> Result<VideoInfo> {
    use tokio::process::Command;

    let output = Command::new(ffprobe_bin)
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            input.to_str().unwrap(),
        ])
        .output()
        .await
        .context("Échec de l'exécution de ffprobe")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("ffprobe a échoué: {}", stderr);
    }

    let json = String::from_utf8(output.stdout)?;
    let probe: FFProbeOutput =
        serde_json::from_str(&json).context("Échec du parsing de la sortie ffprobe")?;

    // Extraire durée
    let duration = probe
        .format
        .duration
        .and_then(|d| d.parse::<f64>().ok())
        .map(Duration::from_secs_f64);

    // Trouver le stream vidéo principal
    let video_stream = probe
        .streams
        .iter()
        .find(|s| s.codec_type == "video")
        .context("Aucun stream vidéo trouvé")?;

    let width = video_stream.width.context("Largeur manquante")?;
    let height = video_stream.height.context("Hauteur manquante")?;

    // Parser FPS (format: "24000/1001" ou "24")
    let fps = video_stream
        .r_frame_rate
        .as_ref()
        .and_then(|r| parse_frame_rate(r))
        .unwrap_or(30.0);

    // Parser total frames avec fallback sur estimation
    let total_frames_from_metadata = video_stream
        .nb_frames
        .as_ref()
        .and_then(|f| f.parse::<u64>().ok());

    // Stratégie de comptage des frames (3 niveaux)
    let total_frames = match total_frames_from_metadata {
        Some(frames) => {
            tracing::info!("Total frames: {} (source: metadata nb_frames)", frames);
            Some(frames)
        }
        None => {
            if precise_count {
                // Niveau 2: Comptage précis via ffmpeg (lent mais exact)
                match count_frames_precisely(ffmpeg_bin, input).await {
                    Ok(frames) => {
                        tracing::info!("Total frames: {} (source: comptage précis ffmpeg)", frames);
                        Some(frames)
                    }
                    Err(e) => {
                        tracing::error!("Échec du comptage précis: {}, fallback sur estimation", e);
                        // Fallback sur estimation si le comptage échoue
                        if let Some(duration) = duration {
                            let duration_secs = duration.as_secs_f64();
                            let estimated = (duration_secs * fps).ceil() as u64;
                            tracing::info!(
                                "Estimation fallback: {} frames (durée={:.2}s × fps={:.2})",
                                estimated,
                                duration_secs,
                                fps
                            );
                            Some(estimated)
                        } else {
                            None
                        }
                    }
                }
            } else {
                // Niveau 3: Estimation rapide (durée × fps)
                if let Some(duration) = duration {
                    let duration_secs = duration.as_secs_f64();
                    let estimated = (duration_secs * fps).ceil() as u64;
                    tracing::info!(
                        "nb_frames absent, estimation: {} frames (durée={:.2}s × fps={:.2})",
                        estimated,
                        duration_secs,
                        fps
                    );
                    Some(estimated)
                } else {
                    tracing::warn!(
                        "Impossible d'estimer total_frames: durée et nb_frames manquants"
                    );
                    None
                }
            }
        }
    };

    // Extraire streams audio
    let audio_streams = probe
        .streams
        .iter()
        .filter(|s| s.codec_type == "audio")
        .map(|s| AudioStreamInfo {
            index: s.index as usize,
            codec: s.codec_name.clone(),
            language: s.tags.as_ref().and_then(|t| t.language.clone()),
            title: s.tags.as_ref().and_then(|t| t.title.clone()),
        })
        .collect();

    // Extraire streams sous-titres
    let subtitle_streams = probe
        .streams
        .iter()
        .filter(|s| s.codec_type == "subtitle")
        .map(|s| SubtitleStreamInfo {
            index: s.index as usize,
            codec: s.codec_name.clone(),
            language: s.tags.as_ref().and_then(|t| t.language.clone()),
            title: s.tags.as_ref().and_then(|t| t.title.clone()),
        })
        .collect();

    Ok(VideoInfo {
        duration,
        total_frames,
        width,
        height,
        fps,
        audio_streams,
        subtitle_streams,
    })
}

/// Parser un frame rate (format "24000/1001" ou "24")
fn parse_frame_rate(rate_str: &str) -> Option<f64> {
    if let Some((num, den)) = rate_str.split_once('/') {
        let num = num.parse::<f64>().ok()?;
        let den = den.parse::<f64>().ok()?;
        if den > 0.0 {
            Some(num / den)
        } else {
            None
        }
    } else {
        rate_str.parse::<f64>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frame_rate() {
        assert_eq!(parse_frame_rate("24"), Some(24.0));
        assert_eq!(parse_frame_rate("30"), Some(30.0));
        assert!((parse_frame_rate("24000/1001").unwrap() - 23.976).abs() < 0.001);
    }

    #[test]
    fn test_frame_estimation() {
        // 2 minutes à 24 fps
        let duration = Duration::from_secs(120);
        let fps = 24.0;
        let estimated = (duration.as_secs_f64() * fps).ceil() as u64;
        assert_eq!(estimated, 2880);

        // NTSC (23.976 fps)
        let fps_ntsc = 23.976;
        let estimated_ntsc = (duration.as_secs_f64() * fps_ntsc).ceil() as u64;
        assert_eq!(estimated_ntsc, 2878);

        // FPS fractionnaire (29.97)
        let fps_frac = 30000.0 / 1001.0;
        let estimated_frac = (duration.as_secs_f64() * fps_frac).ceil() as u64;
        assert_eq!(estimated_frac, 3597);

        // Durée avec décimales (120.5s à 25 fps)
        let duration_decimal = Duration::from_secs_f64(120.5);
        let fps_25 = 25.0;
        let estimated_decimal = (duration_decimal.as_secs_f64() * fps_25).ceil() as u64;
        assert_eq!(estimated_decimal, 3013);
    }
}
