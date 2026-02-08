use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration du daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub daemon: DaemonSettings,
    pub encoding: EncodingSettings,
    pub encoder: EncoderSettings,
    pub ui: UiSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonSettings {
    pub max_concurrent_jobs: usize,
    pub socket_path: String,
    pub log_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodingSettings {
    pub default_encoder: String,
    pub default_audio_mode: String,
    pub default_audio_bitrate: u32,
    pub output_suffix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncoderSettings {
    #[serde(rename = "svt-av1")]
    pub svt_av1: SvtAv1Settings,
    pub aom: AomSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvtAv1Settings {
    pub preset: u32,
    pub crf: u32,
    pub params: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AomSettings {
    #[serde(rename = "cpu-used")]
    pub cpu_used: u32,
    pub crf: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSettings {
    pub file_extensions: Vec<String>,
    pub refresh_interval_ms: u64,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            daemon: DaemonSettings {
                max_concurrent_jobs: 1,
                socket_path: "~/.local/share/encodetalker/daemon.sock".to_string(),
                log_level: "info".to_string(),
            },
            encoding: EncodingSettings {
                default_encoder: "svt-av1".to_string(),
                default_audio_mode: "opus".to_string(),
                default_audio_bitrate: 128,
                output_suffix: ".av1".to_string(),
            },
            encoder: EncoderSettings {
                svt_av1: SvtAv1Settings {
                    preset: 6,
                    crf: 30,
                    params: vec![
                        "--keyint".to_string(),
                        "240".to_string(),
                        "--tune".to_string(),
                        "3".to_string(),
                    ],
                },
                aom: AomSettings {
                    cpu_used: 4,
                    crf: 30,
                },
            },
            ui: UiSettings {
                file_extensions: vec![
                    ".mp4".to_string(),
                    ".mkv".to_string(),
                    ".avi".to_string(),
                    ".mov".to_string(),
                    ".webm".to_string(),
                ],
                refresh_interval_ms: 500,
            },
        }
    }
}

impl DaemonConfig {
    /// Charger la configuration depuis un fichier TOML
    pub fn load_from_file(path: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: DaemonConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Charger la configuration avec fallback sur défaut
    pub fn load_or_default(path: &PathBuf) -> Self {
        Self::load_from_file(path).unwrap_or_else(|_| {
            tracing::warn!(
                "Impossible de charger la config depuis {:?}, utilisation des valeurs par défaut",
                path
            );
            Self::default()
        })
    }
}
