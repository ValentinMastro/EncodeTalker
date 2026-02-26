use crate::{AudioMode, EncoderType, EncodingConfig, VideoContentType};
use std::fmt::Write as _;
use std::path::Path;

/// Générer une preview de la commande ffmpeg demux (TOUS les paramètres)
#[must_use]
pub fn build_ffmpeg_demux_preview(input: &Path, is_interlaced: Option<bool>) -> String {
    let mut cmd = format!("ffmpeg -nostats -loglevel error -i {}", input.display());

    // Ajouter filtre yadif si interlacé
    match is_interlaced {
        Some(true) => cmd.push_str(" -vf yadif"),
        Some(false) => {}
        None => cmd.push_str(" [-vf yadif?]"), // Détection en cours
    }

    cmd.push_str(" -f yuv4mpegpipe -pix_fmt yuv420p10le -strict -1 -");
    cmd
}

/// Générer preview de la commande encodeur
#[must_use]
pub fn build_encoder_preview(config: &EncodingConfig, output_ivf: &str) -> String {
    match config.encoder {
        EncoderType::SvtAv1 => build_svt_av1_preview(config, output_ivf),
        EncoderType::Aom => build_aom_preview(config, output_ivf),
    }
}

/// Générer preview de la commande SVT-AV1
fn build_svt_av1_preview(config: &EncodingConfig, output: &str) -> String {
    let mut cmd = format!(
        "SvtAv1EncApp -i stdin --crf {} --preset {}",
        config.encoder_params.crf, config.encoder_params.preset
    );

    if let Some(threads) = config.encoder_params.threads {
        let _ = write!(cmd, " --lp {threads}");
    }

    cmd.push_str(" --progress 2");

    // Paramètres communs à tous les types de contenu
    let naf = match config.encoder_params.content_type {
        VideoContentType::Anime => 4,
        _ => 1,
    };
    let _ = write!(
        cmd,
        " --qm-min 8 --noise-adaptive-filtering {naf} --complex-hvs 1 --enable-dlf 2"
    );

    // Extra params
    for param in &config.encoder_params.extra_params {
        let _ = write!(cmd, " {param}");
    }

    let _ = write!(cmd, " -b {output}");
    cmd
}

/// Générer preview de la commande aomenc
fn build_aom_preview(config: &EncodingConfig, output: &str) -> String {
    let mut cmd = format!(
        "aomenc --cq-level={} --cpu-used={} --end-usage=q --passes=2",
        config.encoder_params.crf, config.encoder_params.preset
    );

    if let Some(threads) = config.encoder_params.threads {
        let _ = write!(cmd, " --threads={threads}");
    }

    // Extra params
    for param in &config.encoder_params.extra_params {
        let _ = write!(cmd, " {param}");
    }

    let _ = write!(cmd, " --ivf -o {output} -");
    cmd
}

/// Générer preview de la commande d'encodage audio
#[must_use]
pub fn build_audio_preview(input: &Path, config: &EncodingConfig, output_audio: &str) -> String {
    let input_display = input.display();
    match &config.audio_mode {
        AudioMode::Opus { bitrate } => {
            format!("ffmpeg -i {input_display} -vn -c:a libopus -b:a {bitrate}k -map 0:a {output_audio}")
        }
        AudioMode::Copy => {
            format!("ffmpeg -i {input_display} -vn -c:a copy -map 0:a {output_audio}")
        }
        AudioMode::Custom { codec, bitrate } => {
            format!("ffmpeg -i {input_display} -vn -c:a {codec} -b:a {bitrate}k {output_audio}")
        }
    }
}

/// Générer preview du muxing final
#[must_use]
pub fn build_muxing_preview(video_ivf: &str, audio_file: &str, output: &Path) -> String {
    format!(
        "ffmpeg -y -i {video_ivf} -i {audio_file} -map 0:v:0 -map 1:a:0 -c:v copy -c:a copy {}",
        output.display()
    )
}

/// Générer une preview multi-lignes complète du pipeline
#[must_use]
pub fn build_full_pipeline_preview(
    input: &Path,
    output: &Path,
    config: &EncodingConfig,
    is_interlaced: Option<bool>,
) -> Vec<String> {
    let mut lines = Vec::new();

    // Étape 1: Demux + Encode (combinés avec pipe)
    let demux_cmd = build_ffmpeg_demux_preview(input, is_interlaced);
    let encoder_cmd = build_encoder_preview(config, "video.ivf");
    lines.push(format!("{} | {}", demux_cmd, encoder_cmd));

    // Étape 2: Encodage audio
    let audio_ext = match config.audio_mode {
        AudioMode::Opus { .. } => "audio.opus",
        AudioMode::Copy => "audio.copy",
        AudioMode::Custom { ref codec, .. } => &format!("audio.{}", codec.to_lowercase()),
    };
    lines.push(build_audio_preview(input, config, audio_ext));

    // Étape 3: Muxing
    lines.push(build_muxing_preview("video.ivf", audio_ext, output));

    lines
}
