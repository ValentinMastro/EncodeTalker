use crate::{
    AomBuilder, DependencyBuilder, DependencyDetector, DependencyStatus, DepsError, FFmpegBuilder,
    Result, SvtAv1Builder,
};
use encodetalker_common::AppPaths;
use std::path::PathBuf;
use tracing::{error, info};

/// Gestionnaire de dépendances - coordonne téléchargement, compilation et vérification
pub struct DependencyManager {
    paths: AppPaths,
    detector: DependencyDetector,
}

impl DependencyManager {
    pub fn new(paths: AppPaths) -> Self {
        let detector = DependencyDetector::new(paths.deps_bin_dir.clone());
        Self { paths, detector }
    }

    /// Vérifier l'état des dépendances
    pub fn check_status(&self) -> DependencyStatus {
        self.detector.check_all()
    }

    /// S'assurer que toutes les dépendances sont présentes, sinon les compiler
    pub async fn ensure_all_deps(&self) -> Result<()> {
        // Vérifier les dépendances système avant de commencer
        let missing_sys_deps = DependencyDetector::check_system_deps();
        if !missing_sys_deps.is_empty() {
            error!("Dépendances système manquantes: {:?}", missing_sys_deps);
            error!("Installez-les avec: sudo pacman -S base-devel cmake git nasm");
            return Err(DepsError::Build(format!(
                "Dépendances système manquantes: {}",
                missing_sys_deps.join(", ")
            )));
        }

        let status = self.check_status();

        if status.all_present() {
            info!("Toutes les dépendances sont déjà présentes");
            return Ok(());
        }

        let missing = status.missing();
        info!("Dépendances manquantes: {:?}", missing);

        // Compiler les dépendances manquantes
        if !status.ffmpeg || !status.ffprobe {
            self.ensure_ffmpeg().await?;
        }

        if !status.svt_av1 {
            self.ensure_svt_av1().await?;
        }

        if !status.aomenc {
            self.ensure_aom().await?;
        }

        // Vérification finale
        let final_status = self.check_status();
        if !final_status.all_present() {
            error!(
                "Certaines dépendances n'ont pas pu être compilées: {:?}",
                final_status.missing()
            );
            return Err(DepsError::Build("Compilation incomplète".to_string()));
        }

        info!("Toutes les dépendances sont maintenant présentes !");
        Ok(())
    }

    async fn ensure_ffmpeg(&self) -> Result<()> {
        info!("=== Installation de FFmpeg ===");
        let builder = FFmpegBuilder::new(self.paths.deps_src_dir.clone());
        self.build_dependency(&builder).await
    }

    async fn ensure_svt_av1(&self) -> Result<()> {
        info!("=== Installation de SVT-AV1-psy ===");
        let builder = SvtAv1Builder::new(self.paths.deps_src_dir.clone());
        self.build_dependency(&builder).await
    }

    async fn ensure_aom(&self) -> Result<()> {
        info!("=== Installation de libaom ===");
        let builder = AomBuilder::new(self.paths.deps_src_dir.clone());
        self.build_dependency(&builder).await
    }

    async fn build_dependency(&self, builder: &dyn DependencyBuilder) -> Result<()> {
        info!("Téléchargement de {}...", builder.name());
        let source_dir = builder.download().await?;

        info!("Compilation de {}...", builder.name());
        builder
            .build(source_dir, self.paths.deps_dir.clone())
            .await?;

        if builder.verify(&self.paths.deps_bin_dir) {
            info!("{} installé et vérifié avec succès", builder.name());
            Ok(())
        } else {
            error!("{} compilé mais vérification échouée", builder.name());
            Err(DepsError::Build(format!(
                "{} non trouvé après installation",
                builder.name()
            )))
        }
    }

    /// Obtenir le chemin d'un binaire de dépendance
    pub fn get_binary_path(&self, name: &str) -> PathBuf {
        self.paths.deps_bin_dir.join(name)
    }
}
