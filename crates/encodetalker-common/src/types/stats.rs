use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Statistiques d'encodage en temps réel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodingStats {
    /// Frame actuelle
    pub frame: u64,
    /// Total de frames à encoder (None si inconnu)
    pub total_frames: Option<u64>,
    /// FPS actuel
    pub fps: f64,
    /// Bitrate actuel (en kbps)
    pub bitrate: f64,
    /// Temps de progression (durée de la vidéo encodée)
    pub time_encoded: Duration,
    /// Durée totale de la vidéo (None si inconnue)
    pub total_duration: Option<Duration>,
    /// Progression en pourcentage (0.0 - 100.0)
    pub progress_percent: f64,
    /// ETA (temps restant estimé)
    pub eta: Option<Duration>,
}

impl Default for EncodingStats {
    fn default() -> Self {
        Self {
            frame: 0,
            total_frames: None,
            fps: 0.0,
            bitrate: 0.0,
            time_encoded: Duration::from_secs(0),
            total_duration: None,
            progress_percent: 0.0,
            eta: None,
        }
    }
}

impl EncodingStats {
    /// Calculer la progression en pourcentage
    pub fn calculate_progress(&mut self) {
        if let Some(total) = self.total_frames {
            if total > 0 {
                self.progress_percent = (self.frame as f64 / total as f64) * 100.0;
            }
        } else if let (Some(total_dur), _) = (self.total_duration, self.time_encoded) {
            let total_secs = total_dur.as_secs_f64();
            let encoded_secs = self.time_encoded.as_secs_f64();
            if total_secs > 0.0 {
                self.progress_percent = (encoded_secs / total_secs) * 100.0;
            }
        }
    }

    /// Calculer l'ETA basé sur le FPS actuel
    pub fn calculate_eta(&mut self) {
        if let Some(total) = self.total_frames {
            let remaining = total.saturating_sub(self.frame);
            if self.fps > 0.0 {
                let seconds_remaining = remaining as f64 / self.fps;
                self.eta = Some(Duration::from_secs_f64(seconds_remaining));
            }
        }
    }

    /// Mettre à jour les statistiques et recalculer progress/eta
    pub fn update(&mut self) {
        self.calculate_progress();
        self.calculate_eta();
    }
}
