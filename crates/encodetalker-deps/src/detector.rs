use std::path::PathBuf;
use std::process::Command;
use tracing::{info, warn};

/// Détecteur de dépendances
pub struct DependencyDetector {
    bin_dir: PathBuf,
}

impl DependencyDetector {
    pub fn new(bin_dir: PathBuf) -> Self {
        Self { bin_dir }
    }

    /// Vérifier si une dépendance est présente et fonctionnelle
    pub fn check_dependency(&self, name: &str) -> bool {
        let bin_path = self.bin_dir.join(name);

        if !bin_path.exists() {
            warn!("{} non trouvé à {:?}", name, bin_path);
            return false;
        }

        // Vérifier que le binaire est exécutable en testant --version
        // Note: certains binaires (comme ffmpeg) retournent un exit code non-zero
        // même pour --version, donc on vérifie aussi la sortie
        match Command::new(&bin_path).arg("--version").output() {
            Ok(output) => {
                // Vérifier si la commande a produit une sortie (stdout ou stderr)
                let has_output = !output.stdout.is_empty() || !output.stderr.is_empty();

                if output.status.success() || has_output {
                    info!("{} détecté et fonctionnel", name);
                    true
                } else {
                    warn!("{} trouvé mais ne fonctionne pas", name);
                    false
                }
            }
            Err(e) => {
                warn!("{} trouvé mais non exécutable: {}", name, e);
                false
            }
        }
    }

    /// Vérifier toutes les dépendances requises
    pub fn check_all(&self) -> DependencyStatus {
        let ffmpeg = self.check_dependency("ffmpeg");
        let ffprobe = self.check_dependency("ffprobe");
        let svt_av1 = self.check_dependency("SvtAv1EncApp");
        let aomenc = self.check_dependency("aomenc");

        DependencyStatus {
            ffmpeg,
            ffprobe,
            svt_av1,
            aomenc,
        }
    }

    /// Vérifier les dépendances système nécessaires pour compiler
    pub fn check_system_deps() -> Vec<String> {
        let mut missing = Vec::new();

        let deps = [
            ("gcc", &["--version"]),
            ("g++", &["--version"]),
            ("make", &["--version"]),
            ("cmake", &["--version"]),
            ("git", &["--version"]),
            ("nasm", &["--version"]),
        ];

        for (name, args) in deps.iter() {
            if !Self::check_command(name, *args) {
                missing.push(name.to_string());
            }
        }

        missing
    }

    fn check_command(name: &str, args: &[&str]) -> bool {
        Command::new(name)
            .args(args)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Recherche un binaire dans le PATH système
    pub fn find_in_system_path(binary_name: &str) -> Option<PathBuf> {
        use std::env;

        let path_var = env::var("PATH").ok()?;

        for dir in path_var.split(':') {
            let candidate = PathBuf::from(dir).join(binary_name);
            if candidate.exists() && candidate.is_file() {
                // Vérifier que le binaire est exécutable
                if let Ok(metadata) = std::fs::metadata(&candidate) {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if metadata.permissions().mode() & 0o111 != 0 {
                            return Some(candidate);
                        }
                    }
                    #[cfg(not(unix))]
                    return Some(candidate);
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_in_system_path_existing() {
        // La plupart des systèmes ont /bin/sh
        let result = DependencyDetector::find_in_system_path("sh");
        assert!(result.is_some());
        assert!(result.unwrap().exists());
    }

    #[test]
    fn test_find_in_system_path_nonexistent() {
        let result =
            DependencyDetector::find_in_system_path("this_binary_definitely_does_not_exist_xyz123");
        assert!(result.is_none());
    }

    #[test]
    fn test_find_ffmpeg_if_installed() {
        // Test seulement si ffmpeg est installé
        if let Some(path) = DependencyDetector::find_in_system_path("ffmpeg") {
            assert!(path.exists());
            assert!(path.to_string_lossy().contains("ffmpeg"));
        }
    }
}

#[derive(Debug, Clone)]
pub struct DependencyStatus {
    pub ffmpeg: bool,
    pub ffprobe: bool,
    pub svt_av1: bool,
    pub aomenc: bool,
}

impl DependencyStatus {
    pub fn all_present(&self) -> bool {
        self.ffmpeg && self.ffprobe && self.svt_av1 && self.aomenc
    }

    pub fn missing(&self) -> Vec<&str> {
        let mut missing = Vec::new();
        if !self.ffmpeg {
            missing.push("ffmpeg");
        }
        if !self.ffprobe {
            missing.push("ffprobe");
        }
        if !self.svt_av1 {
            missing.push("SvtAv1EncApp");
        }
        if !self.aomenc {
            missing.push("aomenc");
        }
        missing
    }
}
