use serde::{Deserialize, Serialize};

/// Configuration des sources de binaires (système vs compilés localement)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinarySourceSettings {
    /// Source pour ffmpeg/ffprobe: "system" (via PATH) ou "compiled" (local)
    #[serde(default = "default_system")]
    pub ffmpeg_source: String,

    /// Source pour SVT-AV1-PSY: "system" ou "compiled"
    #[serde(default = "default_compiled")]
    pub svt_av1_source: String,

    /// Source pour libaom: "system" ou "compiled"
    #[serde(default = "default_compiled")]
    pub aom_source: String,
}

impl Default for BinarySourceSettings {
    fn default() -> Self {
        Self {
            ffmpeg_source: "system".to_string(),
            svt_av1_source: "compiled".to_string(),
            aom_source: "compiled".to_string(),
        }
    }
}

fn default_system() -> String {
    "system".to_string()
}

fn default_compiled() -> String {
    "compiled".to_string()
}
