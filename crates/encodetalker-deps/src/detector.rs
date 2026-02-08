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
        match Command::new(&bin_path).arg("--version").output() {
            Ok(output) => {
                if output.status.success() {
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
        let mkvmerge = self.check_dependency("mkvmerge");

        DependencyStatus {
            ffmpeg,
            ffprobe,
            svt_av1,
            aomenc,
            mkvmerge,
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
}

#[derive(Debug, Clone)]
pub struct DependencyStatus {
    pub ffmpeg: bool,
    pub ffprobe: bool,
    pub svt_av1: bool,
    pub aomenc: bool,
    pub mkvmerge: bool,
}

impl DependencyStatus {
    pub fn all_present(&self) -> bool {
        self.ffmpeg && self.ffprobe && self.svt_av1 && self.aomenc && self.mkvmerge
    }

    pub fn missing(&self) -> Vec<&str> {
        let mut missing = Vec::new();
        if !self.ffmpeg { missing.push("ffmpeg"); }
        if !self.ffprobe { missing.push("ffprobe"); }
        if !self.svt_av1 { missing.push("SvtAv1EncApp"); }
        if !self.aomenc { missing.push("aomenc"); }
        if !self.mkvmerge { missing.push("mkvmerge"); }
        missing
    }
}
