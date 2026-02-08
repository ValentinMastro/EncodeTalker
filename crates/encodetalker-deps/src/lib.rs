pub mod detector;
pub mod downloader;
pub mod builder;
pub mod manager;

// N'exporter que les types publics nécessaires, pas Result pour éviter conflits
pub use manager::DependencyManager;
pub use detector::{DependencyDetector, DependencyStatus};
pub use downloader::Downloader;
pub use builder::{
    DependencyBuilder, FFmpegBuilder, SvtAv1Builder, AomBuilder, MkvtoolnixBuilder,
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DepsError {
    #[error("Erreur de téléchargement: {0}")]
    Download(String),

    #[error("Erreur de compilation: {0}")]
    Build(String),

    #[error("Dépendance non trouvée: {0}")]
    NotFound(String),

    #[error("Erreur d'I/O: {0}")]
    Io(#[from] std::io::Error),

    #[error("Erreur HTTP: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Autre erreur: {0}")]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, DepsError>;
