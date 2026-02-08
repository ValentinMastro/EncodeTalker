use crate::{DependencyBuilder, DepsError, Downloader, Result};
use std::path::{Path, PathBuf};
use tracing::{error, info};

const AOM_URL: &str = "https://aomedia.googlesource.com/aom";

pub struct AomBuilder {
    downloader: Downloader,
}

impl AomBuilder {
    pub fn new(src_dir: PathBuf) -> Self {
        Self {
            downloader: Downloader::new(src_dir),
        }
    }

    fn get_num_cores(&self) -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    }
}

#[async_trait::async_trait]
impl DependencyBuilder for AomBuilder {
    fn name(&self) -> &str {
        "libaom"
    }

    async fn download(&self) -> Result<PathBuf> {
        self.downloader.clone_git(AOM_URL, "aom").await
    }

    async fn build(&self, source_dir: PathBuf, install_prefix: PathBuf) -> Result<()> {
        info!("Configuration de libaom...");

        let build_dir = source_dir.join("build_release");
        tokio::fs::create_dir_all(&build_dir).await?;

        // CMake configure
        let cmake_output = tokio::process::Command::new("cmake")
            .current_dir(&build_dir)
            .args([
                "..",
                &format!("-DCMAKE_INSTALL_PREFIX={}", install_prefix.display()),
                "-DCMAKE_BUILD_TYPE=Release",
                "-DENABLE_DOCS=0",
                "-DENABLE_TESTS=0",
                "-DENABLE_EXAMPLES=0",
            ])
            .output()
            .await?;

        if !cmake_output.status.success() {
            let stderr = String::from_utf8_lossy(&cmake_output.stderr);
            error!("Échec de la configuration libaom: {}", stderr);
            return Err(DepsError::Build(format!(
                "CMake configure failed: {}",
                stderr
            )));
        }

        info!("Compilation de libaom (cela peut prendre 15-30 minutes)...");

        // Make
        let num_cores = self.get_num_cores();
        let make_output = tokio::process::Command::new("make")
            .current_dir(&build_dir)
            .args(["-j", &num_cores.to_string()])
            .output()
            .await?;

        if !make_output.status.success() {
            let stderr = String::from_utf8_lossy(&make_output.stderr);
            error!("Échec de la compilation libaom: {}", stderr);
            return Err(DepsError::Build(format!("Make failed: {}", stderr)));
        }

        info!("Installation de libaom...");

        // Make install
        let install_output = tokio::process::Command::new("make")
            .current_dir(&build_dir)
            .arg("install")
            .output()
            .await?;

        if !install_output.status.success() {
            let stderr = String::from_utf8_lossy(&install_output.stderr);
            error!("Échec de l'installation libaom: {}", stderr);
            return Err(DepsError::Build(format!("Make install failed: {}", stderr)));
        }

        info!("libaom installé avec succès");
        Ok(())
    }

    fn verify(&self, bin_dir: &Path) -> bool {
        bin_dir.join("aomenc").exists()
    }
}
