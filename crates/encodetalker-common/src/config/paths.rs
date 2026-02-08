use anyhow::{Context, Result};
use std::path::PathBuf;

/// Chemins de l'application
#[derive(Debug, Clone)]
pub struct AppPaths {
    /// Répertoire de données (~/.local/share/encodetalker/)
    pub data_dir: PathBuf,
    /// Répertoire de configuration (~/.config/encodetalker/)
    pub config_dir: PathBuf,
    /// Fichier de configuration utilisateur
    pub config_file: PathBuf,
    /// Fichier de persistance de l'état
    pub state_file: PathBuf,
    /// Socket Unix pour IPC
    pub socket_path: PathBuf,
    /// Fichier de log du daemon
    pub log_file: PathBuf,
    /// Répertoire des dépendances compilées
    pub deps_dir: PathBuf,
    /// Répertoire bin des dépendances
    pub deps_bin_dir: PathBuf,
    /// Répertoire sources des dépendances
    pub deps_src_dir: PathBuf,
}

impl AppPaths {
    /// Créer les chemins de l'application
    pub fn new() -> Result<Self> {
        let data_dir = Self::get_data_dir()?;
        let config_dir = Self::get_config_dir()?;

        let deps_dir = data_dir.join("deps");

        Ok(Self {
            config_file: config_dir.join("config.toml"),
            state_file: data_dir.join("state.json"),
            socket_path: data_dir.join("daemon.sock"),
            log_file: data_dir.join("daemon.log"),
            deps_bin_dir: deps_dir.join("bin"),
            deps_src_dir: deps_dir.join("src"),
            data_dir,
            config_dir,
            deps_dir,
        })
    }

    /// Créer tous les répertoires nécessaires
    pub fn ensure_dirs_exist(&self) -> Result<()> {
        std::fs::create_dir_all(&self.data_dir)
            .context("Impossible de créer le répertoire de données")?;
        std::fs::create_dir_all(&self.config_dir)
            .context("Impossible de créer le répertoire de configuration")?;
        std::fs::create_dir_all(&self.deps_dir)
            .context("Impossible de créer le répertoire des dépendances")?;
        std::fs::create_dir_all(&self.deps_bin_dir)
            .context("Impossible de créer le répertoire bin des dépendances")?;
        std::fs::create_dir_all(&self.deps_src_dir)
            .context("Impossible de créer le répertoire src des dépendances")?;
        Ok(())
    }

    /// Obtenir le répertoire de données
    fn get_data_dir() -> Result<PathBuf> {
        let data_dir = dirs::data_local_dir()
            .context("Impossible de déterminer le répertoire de données local")?
            .join("encodetalker");
        Ok(data_dir)
    }

    /// Obtenir le répertoire de configuration
    fn get_config_dir() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Impossible de déterminer le répertoire de configuration")?
            .join("encodetalker");
        Ok(config_dir)
    }
}

impl Default for AppPaths {
    fn default() -> Self {
        Self::new().expect("Impossible de créer les chemins de l'application")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paths_creation() {
        let paths = AppPaths::new().unwrap();
        assert!(paths.data_dir.ends_with("encodetalker"));
        assert!(paths.config_dir.ends_with("encodetalker"));
        assert_eq!(paths.socket_path.file_name().unwrap(), "daemon.sock");
    }
}
