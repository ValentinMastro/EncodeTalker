#[cfg(windows)]
use crate::{DependencyBuilder, DepsError, Downloader, Result};
#[cfg(windows)]
use std::path::{Path, PathBuf};
#[cfg(windows)]
use tracing::{error, info};

#[cfg(windows)]
const FFMPEG_WIN_URL: &str = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip";

#[cfg(windows)]
pub struct PrecompiledFFmpegBuilder {
    downloader: Downloader,
}

#[cfg(windows)]
impl PrecompiledFFmpegBuilder {
    pub fn new(src_dir: PathBuf) -> Self {
        Self {
            downloader: Downloader::new(src_dir),
        }
    }
}

#[cfg(windows)]
#[async_trait::async_trait]
impl DependencyBuilder for PrecompiledFFmpegBuilder {
    fn name(&self) -> &str {
        "ffmpeg-precompiled"
    }

    async fn download(&self) -> Result<PathBuf> {
        let archive = self
            .downloader
            .download_tarball(FFMPEG_WIN_URL, "ffmpeg-win64.zip")
            .await?;

        // Extraire le zip
        self.extract_zip(&archive).await
    }

    async fn build(&self, source_dir: PathBuf, install_prefix: PathBuf) -> Result<()> {
        info!("Installation des binaires FFmpeg pré-compilés...");

        // Les binaires FFmpeg Windows sont dans ffmpeg-xxx/bin/
        let bin_source = source_dir.join("bin");
        let bin_dest = install_prefix.join("bin");

        tokio::fs::create_dir_all(&bin_dest).await?;

        // Copier ffmpeg.exe et ffprobe.exe
        for binary in &["ffmpeg.exe", "ffprobe.exe"] {
            let src = bin_source.join(binary);
            let dst = bin_dest.join(binary);

            if src.exists() {
                tokio::fs::copy(&src, &dst).await?;
                info!("Copié: {} -> {}", src.display(), dst.display());
            } else {
                error!("Binaire non trouvé: {}", src.display());
                return Err(DepsError::Build(format!(
                    "Binaire {} non trouvé dans l'archive",
                    binary
                )));
            }
        }

        info!("FFmpeg pré-compilé installé avec succès");
        Ok(())
    }

    fn verify(&self, bin_dir: &Path) -> bool {
        use encodetalker_common::binary_name;
        let ffmpeg = bin_dir.join(binary_name("ffmpeg"));
        let ffprobe = bin_dir.join(binary_name("ffprobe"));
        ffmpeg.exists() && ffprobe.exists()
    }
}

#[cfg(windows)]
impl PrecompiledFFmpegBuilder {
    async fn extract_zip(&self, archive_path: &Path) -> Result<PathBuf> {
        use std::io;
        use zip::ZipArchive;

        let extract_dir = self.downloader.src_dir().join("ffmpeg-win64");

        if extract_dir.exists() {
            info!("Archive déjà extraite à {:?}", extract_dir);
            return Ok(extract_dir);
        }

        info!("Extraction de {:?}", archive_path);

        // Ouvrir le fichier zip de manière synchrone
        let file = std::fs::File::open(archive_path)
            .map_err(|e| DepsError::Io(io::Error::new(io::ErrorKind::Other, e)))?;

        let mut archive = ZipArchive::new(file)
            .map_err(|e| DepsError::Build(format!("Échec de lecture du zip: {}", e)))?;

        // Extraire tous les fichiers
        archive
            .extract(self.downloader.src_dir())
            .map_err(|e| DepsError::Build(format!("Échec de l'extraction: {}", e)))?;

        // L'archive contient probablement un dossier avec un nom spécifique
        // On cherche le premier dossier qui contient "ffmpeg"
        let entries = std::fs::read_dir(self.downloader.src_dir())?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir()
                && path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .contains("ffmpeg")
            {
                info!("Archive extraite avec succès vers {:?}", path);
                return Ok(path);
            }
        }

        Err(DepsError::Build(
            "Impossible de trouver le dossier FFmpeg extrait".to_string(),
        ))
    }
}

// Builders pour SVT-AV1 et aomenc pré-compilés peuvent être ajoutés ici de manière similaire
// Pour l'instant, sur Windows, on peut se contenter de FFmpeg
