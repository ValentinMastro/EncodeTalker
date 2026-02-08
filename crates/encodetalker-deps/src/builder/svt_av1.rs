use std::path::PathBuf;
use tracing::{info, error};
use crate::{Result, DepsError, Downloader, DependencyBuilder};

const SVT_AV1_URL: &str = "https://github.com/BlueSwordM/svt-av1-psy.git";

pub struct SvtAv1Builder {
    downloader: Downloader,
}

impl SvtAv1Builder {
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
impl DependencyBuilder for SvtAv1Builder {
    fn name(&self) -> &str {
        "SVT-AV1-psy"
    }

    async fn download(&self) -> Result<PathBuf> {
        self.downloader
            .clone_git(SVT_AV1_URL, "svt-av1-psy")
            .await
    }

    async fn build(&self, source_dir: PathBuf, install_prefix: PathBuf) -> Result<()> {
        info!("Configuration de SVT-AV1-psy...");

        let build_dir = source_dir.join("Build");
        tokio::fs::create_dir_all(&build_dir).await?;

        // CMake configure
        let cmake_output = tokio::process::Command::new("cmake")
            .current_dir(&build_dir)
            .args(&[
                "..",
                &format!("-DCMAKE_INSTALL_PREFIX={}", install_prefix.display()),
                "-DCMAKE_BUILD_TYPE=Release",
                "-DBUILD_SHARED_LIBS=OFF",
            ])
            .output()
            .await?;

        if !cmake_output.status.success() {
            let stderr = String::from_utf8_lossy(&cmake_output.stderr);
            error!("Échec de la configuration SVT-AV1: {}", stderr);
            return Err(DepsError::Build(format!("CMake configure failed: {}", stderr)));
        }

        info!("Compilation de SVT-AV1-psy (cela peut prendre 15-30 minutes)...");

        // Make
        let num_cores = self.get_num_cores();
        let make_output = tokio::process::Command::new("make")
            .current_dir(&build_dir)
            .args(&["-j", &num_cores.to_string()])
            .output()
            .await?;

        if !make_output.status.success() {
            let stderr = String::from_utf8_lossy(&make_output.stderr);
            error!("Échec de la compilation SVT-AV1: {}", stderr);
            return Err(DepsError::Build(format!("Make failed: {}", stderr)));
        }

        info!("Installation de SVT-AV1-psy...");

        // Make install
        let install_output = tokio::process::Command::new("make")
            .current_dir(&build_dir)
            .arg("install")
            .output()
            .await?;

        if !install_output.status.success() {
            let stderr = String::from_utf8_lossy(&install_output.stderr);
            error!("Échec de l'installation SVT-AV1: {}", stderr);
            return Err(DepsError::Build(format!("Make install failed: {}", stderr)));
        }

        info!("SVT-AV1-psy installé avec succès");
        Ok(())
    }

    fn verify(&self, bin_dir: &PathBuf) -> bool {
        bin_dir.join("SvtAv1EncApp").exists()
    }
}
