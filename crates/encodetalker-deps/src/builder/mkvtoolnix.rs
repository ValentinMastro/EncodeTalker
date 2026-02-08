use std::path::PathBuf;
use tracing::{info, error};
use crate::{Result, DepsError, Downloader, DependencyBuilder};

const MKVTOOLNIX_VERSION: &str = "82.0";
const MKVTOOLNIX_URL: &str = "https://mkvtoolnix.download/sources/mkvtoolnix-82.0.tar.xz";

pub struct MkvtoolnixBuilder {
    downloader: Downloader,
}

impl MkvtoolnixBuilder {
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
impl DependencyBuilder for MkvtoolnixBuilder {
    fn name(&self) -> &str {
        "mkvtoolnix"
    }

    async fn download(&self) -> Result<PathBuf> {
        let archive = self.downloader
            .download_tarball(MKVTOOLNIX_URL, "mkvtoolnix-82.0.tar.xz")
            .await?;

        self.downloader
            .extract_tarball(&archive, "mkvtoolnix-82.0")
            .await
    }

    async fn build(&self, source_dir: PathBuf, install_prefix: PathBuf) -> Result<()> {
        info!("Configuration de mkvtoolnix...");

        // Configure using rake (mkvtoolnix uses rake instead of autotools)
        let configure_output = tokio::process::Command::new("./configure")
            .current_dir(&source_dir)
            .args(&[
                &format!("--prefix={}", install_prefix.display()),
                "--disable-gui",
                "--disable-qt",
            ])
            .output()
            .await?;

        if !configure_output.status.success() {
            let stderr = String::from_utf8_lossy(&configure_output.stderr);
            error!("Échec de la configuration mkvtoolnix: {}", stderr);
            return Err(DepsError::Build(format!("Configure failed: {}", stderr)));
        }

        info!("Compilation de mkvtoolnix (cela peut prendre 10-20 minutes)...");

        // Use rake to build
        let num_cores = self.get_num_cores();
        let build_output = tokio::process::Command::new("rake")
            .current_dir(&source_dir)
            .env("MAKEFLAGS", format!("-j{}", num_cores))
            .output()
            .await?;

        if !build_output.status.success() {
            let stderr = String::from_utf8_lossy(&build_output.stderr);
            error!("Échec de la compilation mkvtoolnix: {}", stderr);
            return Err(DepsError::Build(format!("Rake failed: {}", stderr)));
        }

        info!("Installation de mkvtoolnix...");

        // Install
        let install_output = tokio::process::Command::new("rake")
            .current_dir(&source_dir)
            .arg("install")
            .output()
            .await?;

        if !install_output.status.success() {
            let stderr = String::from_utf8_lossy(&install_output.stderr);
            error!("Échec de l'installation mkvtoolnix: {}", stderr);
            return Err(DepsError::Build(format!("Rake install failed: {}", stderr)));
        }

        info!("mkvtoolnix installé avec succès");
        Ok(())
    }

    fn verify(&self, bin_dir: &PathBuf) -> bool {
        bin_dir.join("mkvmerge").exists()
    }
}
