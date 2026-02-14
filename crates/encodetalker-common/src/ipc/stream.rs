use anyhow::{Context, Result};
use std::path::Path;
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// Stream IPC cross-platform (Unix Socket ou Named Pipe)
pub enum IpcStream {
    #[cfg(unix)]
    Unix(tokio::net::UnixStream),
    #[cfg(windows)]
    PipeServer(tokio::net::windows::named_pipe::NamedPipeServer),
    #[cfg(windows)]
    PipeClient(tokio::net::windows::named_pipe::NamedPipeClient),
}

impl IpcStream {
    /// Se connecter au daemon (client)
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self> {
        #[cfg(unix)]
        {
            let stream = tokio::net::UnixStream::connect(path.as_ref())
                .await
                .context("Échec de connexion au socket Unix")?;
            Ok(IpcStream::Unix(stream))
        }

        #[cfg(windows)]
        {
            use tokio::net::windows::named_pipe::ClientOptions;

            let pipe_name = path
                .as_ref()
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Chemin invalide pour Named Pipe"))?;

            let client = ClientOptions::new()
                .open(pipe_name)
                .context("Échec de connexion au Named Pipe")?;

            Ok(IpcStream::PipeClient(client))
        }
    }

    /// Vérifier si un serveur écoute sur ce chemin
    pub fn server_exists(path: impl AsRef<Path>) -> bool {
        #[cfg(unix)]
        {
            path.as_ref().exists()
        }

        #[cfg(windows)]
        {
            use tokio::net::windows::named_pipe::ClientOptions;

            if let Some(pipe_name) = path.as_ref().to_str() {
                // Tenter d'ouvrir le pipe pour voir s'il existe
                ClientOptions::new().open(pipe_name).is_ok()
            } else {
                false
            }
        }
    }
}

// Implémentation de AsyncRead par délégation
impl AsyncRead for IpcStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            #[cfg(unix)]
            IpcStream::Unix(stream) => Pin::new(stream).poll_read(cx, buf),
            #[cfg(windows)]
            IpcStream::PipeServer(pipe) => Pin::new(pipe).poll_read(cx, buf),
            #[cfg(windows)]
            IpcStream::PipeClient(pipe) => Pin::new(pipe).poll_read(cx, buf),
        }
    }
}

// Implémentation de AsyncWrite par délégation
impl AsyncWrite for IpcStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match &mut *self {
            #[cfg(unix)]
            IpcStream::Unix(stream) => Pin::new(stream).poll_write(cx, buf),
            #[cfg(windows)]
            IpcStream::PipeServer(pipe) => Pin::new(pipe).poll_write(cx, buf),
            #[cfg(windows)]
            IpcStream::PipeClient(pipe) => Pin::new(pipe).poll_write(cx, buf),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<std::io::Result<()>> {
        match &mut *self {
            #[cfg(unix)]
            IpcStream::Unix(stream) => Pin::new(stream).poll_flush(cx),
            #[cfg(windows)]
            IpcStream::PipeServer(pipe) => Pin::new(pipe).poll_flush(cx),
            #[cfg(windows)]
            IpcStream::PipeClient(pipe) => Pin::new(pipe).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            #[cfg(unix)]
            IpcStream::Unix(stream) => Pin::new(stream).poll_shutdown(cx),
            #[cfg(windows)]
            IpcStream::PipeServer(pipe) => Pin::new(pipe).poll_shutdown(cx),
            #[cfg(windows)]
            IpcStream::PipeClient(pipe) => Pin::new(pipe).poll_shutdown(cx),
        }
    }
}
