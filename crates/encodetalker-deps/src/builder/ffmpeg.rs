use crate::{DependencyBuilder, DepsError, Downloader, Result};
use std::path::{Path, PathBuf};
use tracing::{error, info};
const FFMPEG_URL: &str = "https://ffmpeg.org/releases/ffmpeg-6.1.tar.xz";

pub struct FFmpegBuilder {
    downloader: Downloader,
}

impl FFmpegBuilder {
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
impl DependencyBuilder for FFmpegBuilder {
    fn name(&self) -> &str {
        "ffmpeg"
    }

    async fn download(&self) -> Result<PathBuf> {
        let archive = self
            .downloader
            .download_tarball(FFMPEG_URL, "ffmpeg-6.1.tar.xz")
            .await?;

        self.downloader
            .extract_tarball(&archive, "ffmpeg-6.1")
            .await
    }

    async fn build(&self, source_dir: PathBuf, install_prefix: PathBuf) -> Result<()> {
        info!("Configuration de FFmpeg...");

        // Configure
        let configure_output = tokio::process::Command::new("./configure")
            .current_dir(&source_dir)
            .args([
                &format!("--prefix={}", install_prefix.display()),
                "--enable-gpl",
                "--enable-libopus",
                "--enable-libvpx",
                "--disable-doc",
                "--disable-htmlpages",
                "--disable-manpages",
                "--disable-podpages",
                "--disable-txtpages",
            ])
            .output()
            .await?;

        if !configure_output.status.success() {
            let stderr = String::from_utf8_lossy(&configure_output.stderr);
            error!("Échec de la configuration FFmpeg: {}", stderr);
            return Err(DepsError::Build(format!("Configure failed: {}", stderr)));
        }

        info!("Compilation de FFmpeg (cela peut prendre 30-60 minutes)...");

        // Make
        let num_cores = self.get_num_cores();
        let make_output = tokio::process::Command::new("make")
            .current_dir(&source_dir)
            .args(["-j", &num_cores.to_string()])
            .output()
            .await?;

        if !make_output.status.success() {
            let stderr = String::from_utf8_lossy(&make_output.stderr);
            error!("Échec de la compilation FFmpeg: {}", stderr);
            return Err(DepsError::Build(format!("Make failed: {}", stderr)));
        }

        info!("Installation de FFmpeg...");

        // Make install
        let install_output = tokio::process::Command::new("make")
            .current_dir(&source_dir)
            .arg("install")
            .output()
            .await?;

        if !install_output.status.success() {
            let stderr = String::from_utf8_lossy(&install_output.stderr);
            error!("Échec de l'installation FFmpeg: {}", stderr);
            return Err(DepsError::Build(format!("Make install failed: {}", stderr)));
        }

        info!("FFmpeg installé avec succès");
        Ok(())
    }

    fn verify(&self, bin_dir: &Path) -> bool {
        let ffmpeg = bin_dir.join("ffmpeg");
        let ffprobe = bin_dir.join("ffprobe");

        ffmpeg.exists() && ffprobe.exists()
    }
}
