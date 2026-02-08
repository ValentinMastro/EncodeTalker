use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;
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

/// Prober un fichier vidéo avec ffprobe
pub fn probe_video(ffprobe_bin: &Path, input: &Path) -> Result<VideoInfo> {
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

    // Parser total frames
    let total_frames = video_stream
        .nb_frames
        .as_ref()
        .and_then(|f| f.parse::<u64>().ok());

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
}
