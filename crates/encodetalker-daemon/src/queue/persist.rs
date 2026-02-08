use std::path::PathBuf;
use std::collections::VecDeque;
use anyhow::{Result, Context};
use encodetalker_common::EncodingJob;
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{info, error};

/// État persisté du daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedState {
    pub queue: VecDeque<EncodingJob>,
    pub active: Vec<EncodingJob>,
    pub history: Vec<EncodingJob>,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            queue: VecDeque::new(),
            active: Vec::new(),
            history: Vec::new(),
        }
    }
}

/// Gestionnaire de persistance
pub struct Persistence {
    state_file: PathBuf,
}

impl Persistence {
    pub fn new(state_file: PathBuf) -> Self {
        Self { state_file }
    }

    /// Charger l'état depuis le disque
    pub async fn load(&self) -> Result<PersistedState> {
        if !self.state_file.exists() {
            info!("Fichier d'état non trouvé, démarrage avec état vide");
            return Ok(PersistedState::default());
        }

        let content = tokio::fs::read_to_string(&self.state_file).await
            .context("Échec de lecture du fichier d'état")?;

        let state: PersistedState = serde_json::from_str(&content)
            .context("Échec du parsing de l'état")?;

        info!("État chargé: {} queued, {} active, {} history",
            state.queue.len(), state.active.len(), state.history.len());

        Ok(state)
    }

    /// Sauvegarder l'état sur le disque (écriture atomique)
    pub async fn save(&self, state: &PersistedState) -> Result<()> {
        let json = serde_json::to_string_pretty(state)
            .context("Échec de sérialisation de l'état")?;

        // Écriture atomique via fichier temporaire
        let temp_file = self.state_file.with_extension("tmp");

        let mut file = File::create(&temp_file).await
            .context("Échec de création du fichier temporaire")?;

        file.write_all(json.as_bytes()).await
            .context("Échec d'écriture du fichier temporaire")?;

        file.sync_all().await
            .context("Échec de sync du fichier temporaire")?;

        drop(file);

        // Rename atomique
        tokio::fs::rename(&temp_file, &self.state_file).await
            .context("Échec du rename atomique")?;

        Ok(())
    }
}
