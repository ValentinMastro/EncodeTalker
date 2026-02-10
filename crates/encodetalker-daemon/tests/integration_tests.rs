use anyhow::{Context, Result};
use encodetalker_common::{
    AudioMode, EncoderParams, EncoderType, EncodingConfig, EncodingJob, EncodingStats, JobStatus,
};
use encodetalker_daemon::encoder::EncodingPipeline;
use std::path::PathBuf;
use tokio::sync::mpsc;

/// Chemin vers les vidéos de test
fn test_video_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("vidéos_de_test")
}

/// Chemin vers les binaires compilés
fn deps_bin_dir() -> PathBuf {
    PathBuf::from(env!("HOME")).join(".local/share/encodetalker/deps/bin")
}

#[tokio::test]
#[ignore] // Ignorer par défaut (test lent, nécessite vidéo)
async fn test_encode_test1_mkv_with_svt_av1() -> Result<()> {
    // Setup
    let input_path = test_video_dir().join("test1.mkv");
    let output_path = test_video_dir().join("test1.av1.mkv");

    // Vérifier que l'input existe
    assert!(
        input_path.exists(),
        "Vidéo de test manquante: {}",
        input_path.display()
    );

    // Nettoyer l'output précédent
    if output_path.exists() {
        std::fs::remove_file(&output_path)?;
    }

    // Créer le pipeline
    let pipeline = EncodingPipeline::new(
        deps_bin_dir().join("ffmpeg"),
        deps_bin_dir().join("ffprobe"),
        deps_bin_dir().join("SvtAv1EncApp"),
        deps_bin_dir().join("aomenc"),
        false, // precise_frame_count désactivé pour vitesse
    );

    // Créer le job d'encodage
    let job = EncodingJob {
        id: uuid::Uuid::new_v4(),
        input_path,
        output_path: output_path.clone(),
        config: EncodingConfig {
            encoder: EncoderType::SvtAv1,
            encoder_params: EncoderParams {
                crf: 63,    // CRF maximum (encodage le plus rapide)
                preset: 13, // Preset le plus rapide pour SVT-AV1
                extra_params: vec![],
            },
            audio_mode: AudioMode::Opus { bitrate: 128 },
            audio_streams: None,
            subtitle_streams: None,
        },
        created_at: chrono::Utc::now(),
        status: JobStatus::Queued,
        stats: None,
        error_message: None,
        started_at: None,
        finished_at: None,
    };

    // Channels pour stats et cancel
    let (stats_tx, mut stats_rx) = mpsc::unbounded_channel::<EncodingStats>();
    let (_cancel_tx, cancel_rx) = mpsc::unbounded_channel::<()>();

    // Spawn task pour logger les stats
    let stats_task = tokio::spawn(async move {
        while let Some(stats) = stats_rx.recv().await {
            println!(
                "Progress: frame={} fps={:.1} bitrate={:.1} kbps",
                stats.frame, stats.fps, stats.bitrate
            );
        }
    });

    // Lancer l'encodage
    println!("Démarrage encodage de test1.mkv...");
    let result = pipeline.encode_job(&job, stats_tx, cancel_rx).await;

    // Attendre le stats task
    let _ = stats_task.await;

    // Vérifier que l'encodage a réussi
    assert!(
        result.is_ok(),
        "Encodage a échoué: {:?}",
        result.unwrap_err()
    );

    // Vérifier que le fichier output existe
    assert!(
        output_path.exists(),
        "Fichier output manquant: {}",
        output_path.display()
    );

    // Vérifier les streams du fichier output avec ffprobe
    verify_output_streams(&output_path).await?;

    println!("✅ Test d'encodage réussi!");
    Ok(())
}

/// Vérifier que le fichier output contient les streams attendus
async fn verify_output_streams(output_path: &std::path::Path) -> Result<()> {
    use tokio::process::Command;

    let output = Command::new(deps_bin_dir().join("ffprobe"))
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_streams",
            output_path.to_str().unwrap(),
        ])
        .output()
        .await
        .context("Échec ffprobe")?;

    assert!(output.status.success(), "ffprobe a échoué");

    let json = String::from_utf8(output.stdout)?;
    let data: serde_json::Value = serde_json::from_str(&json)?;
    let streams = data["streams"]
        .as_array()
        .context("Pas de streams dans output")?;

    // Vérifier présence stream vidéo AV1
    let video_stream = streams
        .iter()
        .find(|s| s["codec_type"] == "video")
        .context("Pas de stream vidéo")?;

    assert_eq!(
        video_stream["codec_name"], "av1",
        "Codec vidéo incorrect: attendu av1, trouvé {}",
        video_stream["codec_name"]
    );

    // Vérifier présence stream audio Opus
    let audio_stream = streams
        .iter()
        .find(|s| s["codec_type"] == "audio")
        .context("Pas de stream audio")?;

    assert_eq!(
        audio_stream["codec_name"], "opus",
        "Codec audio incorrect: attendu opus, trouvé {}",
        audio_stream["codec_name"]
    );

    // Vérifier présence stream sous-titres
    let subtitle_stream = streams.iter().find(|s| s["codec_type"] == "subtitle");

    assert!(
        subtitle_stream.is_some(),
        "Pas de stream sous-titres trouvé"
    );

    println!("✅ Tous les streams sont présents et corrects");
    Ok(())
}

#[tokio::test]
async fn test_probe_video() -> Result<()> {
    use encodetalker_daemon::encoder::probe_video;

    let input_path = test_video_dir().join("test1.mkv");

    if !input_path.exists() {
        println!("⚠️  Vidéo de test manquante, test ignoré");
        return Ok(());
    }

    let video_info = probe_video(
        &deps_bin_dir().join("ffprobe"),
        &deps_bin_dir().join("ffmpeg"),
        &input_path,
        false, // precise_frame_count
    )
    .await?;

    // Vérifications de base
    assert!(video_info.width > 0, "Largeur invalide");
    assert!(video_info.height > 0, "Hauteur invalide");
    assert!(video_info.fps > 0.0, "FPS invalide");
    assert!(video_info.total_frames.is_some(), "Total frames manquant");
    assert!(!video_info.audio_streams.is_empty(), "Pas de stream audio");
    assert!(
        !video_info.subtitle_streams.is_empty(),
        "Pas de stream sous-titres"
    );

    println!(
        "✅ Probe réussi: {}x{} @ {:.2} fps, {} frames",
        video_info.width,
        video_info.height,
        video_info.fps,
        video_info.total_frames.unwrap()
    );

    Ok(())
}
