use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use super::PathsConfig;

/// Ajouter le suffixe d'exécutable (.exe sur Windows, rien sur Unix)
pub fn binary_name(name: &str) -> String {
    format!("{}{}", name, std::env::consts::EXE_SUFFIX)
}

/// Helper pour obtenir le chemin IPC par défaut (socket Unix ou Named Pipe)
fn get_default_ipc_path(data_dir: &Path) -> PathBuf {
    #[cfg(unix)]
    {
        data_dir.join("daemon.sock")
    }
    #[cfg(windows)]
    {
        let _ = data_dir; // Utilisé seulement sur Unix
        PathBuf::from(r"\\.\pipe\encodetalker")
    }
}

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
    /// Créer les chemins avec valeurs par défaut XDG
    ///
    /// Équivalent à `AppPaths::from_config(None)`
    pub fn new() -> Result<Self> {
        Self::from_config(None)
    }

    /// Créer les chemins avec configuration personnalisée
    ///
    /// Ordre de priorité pour deps_dir:
    /// 1. Valeur explicite dans paths_config (ex: deps_dir = "/custom/deps")
    /// 2. Dossier .dependencies/ à côté de l'exécutable (mode portable)
    /// 3. Valeur dérivée de data_dir ou défaut XDG (data_dir/deps)
    ///
    /// # Arguments
    /// * `paths_config` - Configuration optionnelle des chemins depuis [paths] du TOML
    ///
    /// # Exemples
    /// ```no_run
    /// use encodetalker_common::{AppPaths, PathsConfig};
    ///
    /// // Valeurs par défaut XDG
    /// let paths = AppPaths::from_config(None).unwrap();
    ///
    /// // Personnalisation partielle
    /// let config = PathsConfig {
    ///     deps_dir: Some("/mnt/ssd/deps".to_string()),
    ///     ..Default::default()
    /// };
    /// let paths = AppPaths::from_config(Some(config)).unwrap();
    /// ```
    pub fn from_config(paths_config: Option<PathsConfig>) -> Result<Self> {
        let config = paths_config.unwrap_or_default();

        // 1. Déterminer data_dir (custom ou défaut XDG)
        let data_dir = if let Some(ref custom) = config.data_dir {
            PathsConfig::expand_path(custom)
                .context("Impossible d'expanser data_dir personnalisé")?
        } else {
            Self::get_default_data_dir()?
        };

        // 2. config_dir TOUJOURS depuis XDG (non configurable pour éviter confusion)
        let config_dir = Self::get_default_config_dir()?;

        // 3. Déterminer deps_dir (custom, .dependencies/ portable, ou défaut XDG)
        let deps_dir = if let Some(ref custom) = config.deps_dir {
            PathsConfig::expand_path(custom)
                .context("Impossible d'expanser deps_dir personnalisé")?
        } else if let Some(portable) = Self::find_portable_deps_dir() {
            portable
        } else {
            // Dérivé de data_dir (personnalisé ou XDG)
            data_dir.join("deps")
        };

        // 4. Déterminer socket_path (custom, dérivé de data_dir, ou défaut IPC)
        let socket_path = if let Some(ref custom) = config.socket_path {
            PathsConfig::expand_path(custom)
                .context("Impossible d'expanser socket_path personnalisé")?
        } else {
            // Dérivé de data_dir ou chemin par défaut selon l'OS
            get_default_ipc_path(&data_dir)
        };

        // 5. Construire tous les chemins
        Ok(Self {
            config_file: config_dir.join("config.toml"),
            state_file: data_dir.join("state.json"),
            log_file: data_dir.join("daemon.log"),
            deps_bin_dir: deps_dir.join("bin"),
            deps_src_dir: deps_dir.join("src"),
            data_dir,
            config_dir,
            deps_dir,
            socket_path,
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

    /// Chercher un dossier .dependencies/ à côté de l'exécutable (mode portable)
    fn find_portable_deps_dir() -> Option<PathBuf> {
        let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
        let portable = exe_dir.join(".dependencies");
        if portable.is_dir() {
            Some(portable)
        } else {
            None
        }
    }

    /// Obtenir le répertoire de données par défaut (XDG)
    fn get_default_data_dir() -> Result<PathBuf> {
        let data_dir = dirs::data_local_dir()
            .context("Impossible de déterminer le répertoire de données local")?
            .join("encodetalker");
        Ok(data_dir)
    }

    /// Obtenir le répertoire de configuration par défaut (XDG)
    fn get_default_config_dir() -> Result<PathBuf> {
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
    use std::env;

    #[test]
    fn test_default_paths_unchanged() {
        // Vérifier que new() fonctionne comme avant (rétrocompatibilité)
        let paths = AppPaths::new().unwrap();
        assert!(paths.data_dir.ends_with("encodetalker"));
        assert!(paths.config_dir.ends_with("encodetalker"));
        assert!(paths.deps_dir.ends_with("deps"));

        // Sur Unix, socket_path est un fichier, sur Windows c'est un Named Pipe
        #[cfg(unix)]
        assert_eq!(paths.socket_path.file_name().unwrap(), "daemon.sock");
        #[cfg(windows)]
        assert!(paths.socket_path.to_string_lossy().contains("encodetalker"));
    }

    #[test]
    fn test_from_config_none_is_same_as_new() {
        let paths1 = AppPaths::new().unwrap();
        let paths2 = AppPaths::from_config(None).unwrap();

        assert_eq!(paths1.data_dir, paths2.data_dir);
        assert_eq!(paths1.deps_dir, paths2.deps_dir);
        assert_eq!(paths1.socket_path, paths2.socket_path);
    }

    #[test]
    fn test_custom_data_dir() {
        let config = PathsConfig {
            data_dir: Some("/tmp/custom_data".to_string()),
            deps_dir: None,
            socket_path: None,
        };

        let paths = AppPaths::from_config(Some(config)).unwrap();
        assert_eq!(paths.data_dir, PathBuf::from("/tmp/custom_data"));

        // Vérifier chemins dérivés
        assert_eq!(paths.deps_dir, PathBuf::from("/tmp/custom_data/deps"));

        // Socket path est dérivé sur Unix, mais Named Pipe fixe sur Windows
        #[cfg(unix)]
        assert_eq!(
            paths.socket_path,
            PathBuf::from("/tmp/custom_data/daemon.sock")
        );
        #[cfg(windows)]
        assert!(paths.socket_path.to_string_lossy().contains("encodetalker"));

        assert_eq!(
            paths.state_file,
            PathBuf::from("/tmp/custom_data/state.json")
        );
    }

    #[test]
    fn test_custom_all_paths() {
        let config = PathsConfig {
            data_dir: Some("/data".to_string()),
            deps_dir: Some("/deps".to_string()),
            socket_path: Some("/tmp/custom.sock".to_string()),
        };

        let paths = AppPaths::from_config(Some(config)).unwrap();
        assert_eq!(paths.data_dir, PathBuf::from("/data"));
        assert_eq!(paths.deps_dir, PathBuf::from("/deps"));
        assert_eq!(paths.socket_path, PathBuf::from("/tmp/custom.sock"));

        // state_file et log_file toujours dérivés de data_dir
        assert_eq!(paths.state_file, PathBuf::from("/data/state.json"));
        assert_eq!(paths.log_file, PathBuf::from("/data/daemon.log"));
    }

    #[test]
    fn test_custom_deps_only() {
        let config = PathsConfig {
            data_dir: None,
            deps_dir: Some("/mnt/ssd/deps".to_string()),
            socket_path: None,
        };

        let paths = AppPaths::from_config(Some(config)).unwrap();

        // data_dir et socket_path utilisent valeurs XDG
        assert!(paths.data_dir.ends_with("encodetalker"));

        #[cfg(unix)]
        assert!(paths.socket_path.ends_with("daemon.sock"));
        #[cfg(windows)]
        assert!(paths.socket_path.to_string_lossy().contains("encodetalker"));

        // deps_dir est personnalisé
        assert_eq!(paths.deps_dir, PathBuf::from("/mnt/ssd/deps"));
    }

    #[test]
    fn test_tilde_expansion() {
        let config = PathsConfig {
            data_dir: Some("~/test_encodetalker".to_string()),
            deps_dir: None,
            socket_path: None,
        };

        let paths = AppPaths::from_config(Some(config)).unwrap();
        // Vérifier que ~ a été expansé
        assert!(!paths.data_dir.to_string_lossy().contains('~'));
        assert!(paths.data_dir.is_absolute());
    }

    #[test]
    fn test_env_var_expansion() {
        env::set_var("TEST_DIR", "/tmp/test");
        let config = PathsConfig {
            socket_path: Some("$TEST_DIR/encodetalker.sock".to_string()),
            data_dir: None,
            deps_dir: None,
        };

        let paths = AppPaths::from_config(Some(config)).unwrap();
        assert_eq!(
            paths.socket_path,
            PathBuf::from("/tmp/test/encodetalker.sock")
        );
    }

    #[test]
    fn test_config_dir_always_xdg() {
        // config_dir ne peut PAS être personnalisé
        let config = PathsConfig {
            data_dir: Some("/custom".to_string()),
            deps_dir: Some("/custom/deps".to_string()),
            socket_path: Some("/custom/socket".to_string()),
        };

        let paths = AppPaths::from_config(Some(config)).unwrap();
        // config_dir reste XDG
        assert!(paths.config_dir.ends_with("encodetalker"));
        assert!(paths.config_file.ends_with("config.toml"));
    }
}
