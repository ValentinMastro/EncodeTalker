use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration des chemins personnalisables (section [paths] du TOML)
///
/// Tous les champs sont optionnels. Si absents, les valeurs XDG par défaut sont utilisées.
/// Support de l'expansion de ~ (home directory) et variables d'environnement ($HOME, $USER, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PathsConfig {
    /// Répertoire de données principal
    /// Défaut: ~/.local/share/encodetalker
    #[serde(default)]
    pub data_dir: Option<String>,

    /// Répertoire des dépendances compilées (~500 MB)
    /// Défaut: <data_dir>/deps
    /// Cas d'usage: déplacer sur SSD pour compilation plus rapide
    #[serde(default)]
    pub deps_dir: Option<String>,

    /// Socket Unix pour communication daemon<->TUI
    /// Défaut: <data_dir>/daemon.sock
    /// Cas d'usage: multi-utilisateurs avec /tmp/encodetalker-$USER.sock
    #[serde(default)]
    pub socket_path: Option<String>,
}

impl PathsConfig {
    /// Expander un chemin avec support de ~ et variables d'environnement
    ///
    /// Exemples:
    /// - "~/data" → "/home/user/data"
    /// - "$HOME/data" → "/home/user/data"
    /// - "/tmp/encodetalker-$USER.sock" → "/tmp/encodetalker-alice.sock"
    pub fn expand_path(path: &str) -> anyhow::Result<PathBuf> {
        let expanded = shellexpand::full(path).with_context(|| {
            format!(
                "Impossible d'expanser le chemin '{}'. Vérifiez que toutes les variables d'environnement existent.",
                path
            )
        })?;
        Ok(PathBuf::from(expanded.as_ref()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_expand_absolute_path() {
        let result = PathsConfig::expand_path("/tmp/test").unwrap();
        assert_eq!(result, PathBuf::from("/tmp/test"));
    }

    #[test]
    fn test_expand_tilde() {
        let result = PathsConfig::expand_path("~/test").unwrap();
        // Vérifier que ~ a été expansé (ne contient plus ~)
        assert!(!result.to_string_lossy().contains('~'));
        assert!(result.is_absolute());
    }

    #[test]
    fn test_expand_env_var() {
        env::set_var("TEST_VAR", "/tmp/test");
        let result = PathsConfig::expand_path("$TEST_VAR/subdir").unwrap();
        assert_eq!(result, PathBuf::from("/tmp/test/subdir"));
    }

    #[test]
    fn test_expand_nonexistent_var() {
        // Variable inexistante devrait échouer
        let result = PathsConfig::expand_path("$NONEXISTENT_VAR_12345/path");
        assert!(result.is_err());
    }

    #[test]
    fn test_default_is_all_none() {
        let config = PathsConfig::default();
        assert!(config.data_dir.is_none());
        assert!(config.deps_dir.is_none());
        assert!(config.socket_path.is_none());
    }
}
