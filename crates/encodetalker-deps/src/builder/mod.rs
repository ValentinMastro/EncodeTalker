pub mod ffmpeg;
pub mod svt_av1;
pub mod aom;
pub mod mkvtoolnix;

pub use ffmpeg::*;
pub use svt_av1::*;
pub use aom::*;
pub use mkvtoolnix::*;

use std::path::PathBuf;
use crate::Result;

/// Trait pour un builder de dépendance
#[async_trait::async_trait]
pub trait DependencyBuilder: Send + Sync {
    /// Nom de la dépendance
    fn name(&self) -> &str;

    /// Télécharger les sources
    async fn download(&self) -> Result<PathBuf>;

    /// Compiler et installer
    async fn build(&self, source_dir: PathBuf, install_prefix: PathBuf) -> Result<()>;

    /// Vérifier que la compilation a réussi
    fn verify(&self, bin_dir: &PathBuf) -> bool;
}
