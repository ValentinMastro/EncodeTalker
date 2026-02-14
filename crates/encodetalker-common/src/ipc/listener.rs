use super::IpcStream;
use anyhow::{Context, Result};
use std::path::Path;

/// Listener IPC cross-platform (Unix Socket ou Named Pipe Server)
pub struct IpcListener {
    #[cfg(unix)]
    inner: tokio::net::UnixListener,
    #[cfg(windows)]
    pipe_name: std::ffi::OsString,
}

impl IpcListener {
    /// Créer un listener sur le chemin spécifié
    pub fn bind(path: impl AsRef<Path>) -> Result<Self> {
        #[cfg(unix)]
        {
            let listener = tokio::net::UnixListener::bind(path.as_ref())
                .context("Impossible de créer le socket Unix")?;
            Ok(IpcListener { inner: listener })
        }

        #[cfg(windows)]
        {
            let pipe_name = path.as_ref().as_os_str().to_owned();
            Ok(IpcListener { pipe_name })
        }
    }

    /// Nettoyer le chemin avant de créer le listener (Unix: supprimer fichier, Windows: no-op)
    pub fn cleanup(path: impl AsRef<Path>) {
        #[cfg(unix)]
        {
            if path.as_ref().exists() {
                let _ = std::fs::remove_file(path.as_ref());
            }
        }

        #[cfg(windows)]
        {
            // Les Named Pipes Windows n'ont pas besoin de nettoyage
            let _ = path;
        }
    }

    /// Accepter une connexion cliente
    pub async fn accept(&self) -> Result<IpcStream> {
        #[cfg(unix)]
        {
            let (stream, _) = self
                .inner
                .accept()
                .await
                .context("Erreur lors de l'acceptation de connexion")?;
            Ok(IpcStream::Unix(stream))
        }

        #[cfg(windows)]
        {
            use tokio::net::windows::named_pipe::{PipeMode, ServerOptions};

            // Créer une nouvelle instance du Named Pipe pour ce client
            let pipe_name = self
                .pipe_name
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Nom de pipe invalide"))?;

            let server = ServerOptions::new()
                .pipe_mode(PipeMode::Byte)
                .first_pipe_instance(false)
                .create(pipe_name)
                .context("Impossible de créer une instance de Named Pipe")?;

            // Attendre qu'un client se connecte
            server
                .connect()
                .await
                .context("Erreur lors de l'attente de connexion au pipe")?;

            Ok(IpcStream::PipeServer(server))
        }
    }
}

#[cfg(windows)]
impl IpcListener {
    /// Créer la première instance du Named Pipe (pour Windows)
    pub async fn create_first_instance(path: impl AsRef<Path>) -> Result<()> {
        use tokio::net::windows::named_pipe::{PipeMode, ServerOptions};

        let pipe_name = path
            .as_ref()
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Nom de pipe invalide"))?;

        // Créer la première instance pour "réserver" le nom du pipe
        let _first = ServerOptions::new()
            .pipe_mode(PipeMode::Byte)
            .first_pipe_instance(true)
            .create(pipe_name)
            .context("Impossible de créer la première instance du Named Pipe")?;

        // On laisse cette instance en vie (elle sera utilisée par le premier accept)
        // Pour Windows, on doit garder au moins une instance active
        Ok(())
    }
}
