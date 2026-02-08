use super::{EncodingStats, JobStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Configuration d'encodage pour un job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodingConfig {
    /// Encodeur à utiliser (svt-av1, aom)
    pub encoder: EncoderType,
    /// Mode audio
    pub audio_mode: AudioMode,
    /// Streams audio à inclure (None = tous)
    pub audio_streams: Option<Vec<usize>>,
    /// Streams de sous-titres à inclure (None = tous)
    pub subtitle_streams: Option<Vec<usize>>,
    /// Paramètres spécifiques à l'encodeur
    pub encoder_params: EncoderParams,
}

impl Default for EncodingConfig {
    fn default() -> Self {
        Self {
            encoder: EncoderType::SvtAv1,
            audio_mode: AudioMode::Opus { bitrate: 128 },
            audio_streams: None,
            subtitle_streams: None,
            encoder_params: EncoderParams::default(),
        }
    }
}

/// Type d'encodeur vidéo
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncoderType {
    /// SVT-AV1 (recommandé, rapide)
    SvtAv1,
    /// libaom AV1 (plus lent, meilleure qualité)
    Aom,
}

impl std::fmt::Display for EncoderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncoderType::SvtAv1 => write!(f, "SVT-AV1"),
            EncoderType::Aom => write!(f, "libaom AV1"),
        }
    }
}

/// Mode de traitement audio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioMode {
    /// Ré-encoder en Opus avec bitrate spécifié (kbps)
    Opus { bitrate: u32 },
    /// Copier les streams audio sans ré-encodage
    Copy,
    /// Custom (pour usage futur)
    Custom { codec: String, bitrate: u32 },
}

impl Default for AudioMode {
    fn default() -> Self {
        Self::Opus { bitrate: 128 }
    }
}

/// Paramètres spécifiques aux encodeurs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncoderParams {
    /// CRF (Constant Rate Factor) - qualité (0-63, plus bas = meilleure qualité)
    pub crf: u32,
    /// Preset de vitesse (pour SVT-AV1: 0-13, pour aom: 0-8)
    pub preset: u32,
    /// Paramètres additionnels en ligne de commande
    pub extra_params: Vec<String>,
}

impl Default for EncoderParams {
    fn default() -> Self {
        Self {
            crf: 30,
            preset: 6,
            extra_params: vec![],
        }
    }
}

/// Job d'encodage complet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodingJob {
    /// Identifiant unique du job
    pub id: Uuid,
    /// Fichier source
    pub input_path: PathBuf,
    /// Fichier de destination
    pub output_path: PathBuf,
    /// Configuration d'encodage
    pub config: EncodingConfig,
    /// Status actuel
    pub status: JobStatus,
    /// Statistiques d'encodage (Some si Running)
    pub stats: Option<EncodingStats>,
    /// Message d'erreur (Some si Failed)
    pub error_message: Option<String>,
    /// Date de création du job
    pub created_at: DateTime<Utc>,
    /// Date de début d'exécution (Some si Running ou terminé)
    pub started_at: Option<DateTime<Utc>>,
    /// Date de fin (Some si terminé)
    pub finished_at: Option<DateTime<Utc>>,
}

impl EncodingJob {
    /// Créer un nouveau job
    pub fn new(input_path: PathBuf, output_path: PathBuf, config: EncodingConfig) -> Self {
        Self {
            id: Uuid::new_v4(),
            input_path,
            output_path,
            config,
            status: JobStatus::Queued,
            stats: None,
            error_message: None,
            created_at: Utc::now(),
            started_at: None,
            finished_at: None,
        }
    }

    /// Marquer le job comme démarré
    pub fn mark_started(&mut self) {
        self.status = JobStatus::Running;
        self.started_at = Some(Utc::now());
        self.stats = Some(EncodingStats::default());
    }

    /// Marquer le job comme terminé
    pub fn mark_completed(&mut self) {
        self.status = JobStatus::Completed;
        self.finished_at = Some(Utc::now());
    }

    /// Marquer le job comme échoué
    pub fn mark_failed(&mut self, error: String) {
        self.status = JobStatus::Failed;
        self.error_message = Some(error);
        self.finished_at = Some(Utc::now());
    }

    /// Marquer le job comme annulé
    pub fn mark_cancelled(&mut self) {
        self.status = JobStatus::Cancelled;
        self.finished_at = Some(Utc::now());
    }

    /// Obtenir la durée d'exécution
    pub fn execution_duration(&self) -> Option<chrono::Duration> {
        let started = self.started_at?;
        let finished = self.finished_at.unwrap_or_else(Utc::now);
        Some(finished - started)
    }
}
