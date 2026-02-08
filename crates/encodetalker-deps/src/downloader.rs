use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use reqwest;
use tracing::info;
use crate::{Result, DepsError};

/// Téléchargeur de sources
pub struct Downloader {
    src_dir: PathBuf,
}

impl Downloader {
    pub fn new(src_dir: PathBuf) -> Self {
        Self { src_dir }
    }

    /// Télécharger une archive tar.xz
    pub async fn download_tarball(&self, url: &str, output_name: &str) -> Result<PathBuf> {
        let output_path = self.src_dir.join(output_name);

        if output_path.exists() {
            info!("Archive {} déjà téléchargée", output_name);
            return Ok(output_path);
        }

        info!("Téléchargement de {} depuis {}", output_name, url);

        let response = reqwest::get(url).await?;

        if !response.status().is_success() {
            return Err(DepsError::Download(format!(
                "Échec du téléchargement: status {}",
                response.status()
            )));
        }

        let bytes = response.bytes().await?;
        let mut file = File::create(&output_path).await?;
        file.write_all(&bytes).await?;

        info!("Archive {} téléchargée avec succès", output_name);
        Ok(output_path)
    }

    /// Cloner un dépôt git
    pub async fn clone_git(&self, url: &str, dir_name: &str) -> Result<PathBuf> {
        let output_path = self.src_dir.join(dir_name);

        if output_path.exists() {
            info!("Dépôt {} déjà cloné", dir_name);
            return Ok(output_path);
        }

        info!("Clonage de {} depuis {}", dir_name, url);

        let output = tokio::process::Command::new("git")
            .args(&["clone", url, output_path.to_str().unwrap()])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DepsError::Download(format!(
                "Échec du clonage git: {}",
                stderr
            )));
        }

        info!("Dépôt {} cloné avec succès", dir_name);
        Ok(output_path)
    }

    /// Extraire une archive tar.xz
    pub async fn extract_tarball(&self, archive_path: &Path, extract_name: &str) -> Result<PathBuf> {
        let extract_path = self.src_dir.join(extract_name);

        if extract_path.exists() {
            info!("Archive déjà extraite à {:?}", extract_path);
            return Ok(extract_path);
        }

        info!("Extraction de {:?}", archive_path);

        let output = tokio::process::Command::new("tar")
            .args(&[
                "xf",
                archive_path.to_str().unwrap(),
                "-C",
                self.src_dir.to_str().unwrap(),
            ])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DepsError::Download(format!(
                "Échec de l'extraction: {}",
                stderr
            )));
        }

        info!("Archive extraite avec succès");
        Ok(extract_path)
    }
}
