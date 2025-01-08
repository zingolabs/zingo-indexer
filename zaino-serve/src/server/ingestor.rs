//! Holds the server ingestor (listener) implementations.

use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::net::TcpListener;

use crate::server::{
    error::{IngestorError, QueueError},
    queue::QueueSender,
    request::ZingoIndexerRequest,
};
use zaino_state::status::{AtomicStatus, StatusType};

/// Listens for incoming gRPC requests over HTTP.
pub(crate) struct TcpIngestor {
    /// Tcp Listener.
    ingestor: TcpListener,
    /// Used to send requests to the queue.
    queue: QueueSender<ZingoIndexerRequest>,
    /// Current status of the ingestor.
    status: AtomicStatus,
    /// Represents the Online status of the gRPC server.
    online: Arc<AtomicBool>,
}

impl TcpIngestor {
    /// Creates a Tcp Ingestor.
    pub(crate) async fn spawn(
        listen_addr: SocketAddr,
        queue: QueueSender<ZingoIndexerRequest>,
        status: AtomicStatus,
        online: Arc<AtomicBool>,
    ) -> Result<Self, IngestorError> {
        status.store(StatusType::Spawning.into());
        let listener = TcpListener::bind(listen_addr).await?;
        println!("TcpIngestor listening at: {}.", listen_addr);
        Ok(TcpIngestor {
            ingestor: listener,
            queue,
            online,
            status,
        })
    }

    /// Starts Tcp service.
    pub(crate) async fn serve(self) -> tokio::task::JoinHandle<Result<(), IngestorError>> {
        tokio::task::spawn(async move {
            // NOTE: This interval may need to be changed or removed / moved once scale testing begins.
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(50));
            // TODO Check blockcache sync status and wait on server / node if on hold.
            self.status.store(StatusType::Ready.into());
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if self.check_for_shutdown().await {
                            self.status.store(StatusType::Offline.into());
                            return Ok(());
                        }
                    }
                    incoming = self.ingestor.accept() => {
                        // NOTE: This may need to be removed / moved for scale use.
                        if self.check_for_shutdown().await {
                            self.status.store(StatusType::Offline.into());
                            return Ok(());
                        }
                        match incoming {
                            Ok((stream, _)) => {
                                match self.queue.try_send(ZingoIndexerRequest::new_from_grpc(stream)) {
                                    Ok(_) => {
                                        println!("[TEST] Requests in Queue: {}", self.queue.queue_length());
                                    }
                                    Err(QueueError::QueueFull(_request)) => {
                                        eprintln!("Queue Full.");
                                        // TODO: Return queue full tonic status over tcpstream and close (that TcpStream..).
                                    }
                                    Err(e) => {
                                        eprintln!("Queue Closed. Failed to send request to queue: {}", e);
                                        // TODO: Handle queue closed error here.
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to accept connection with client: {}", e);
                                // TODO: Handle failed connection errors here (count errors and restart ingestor / proxy or initiate shotdown?)
                            }
                        }
                    }
                }
            }
        })
    }

    /// Checks indexers online status and ingestors internal status for closure signal.
    pub(crate) async fn check_for_shutdown(&self) -> bool {
        if self.status() >= 4 {
            return true;
        }
        if !self.check_online() {
            return true;
        }
        false
    }

    /// Sets the ingestor to close gracefully.
    pub(crate) fn shutdown(&mut self) {
        self.status.store(StatusType::Closing.into())
    }

    /// Returns the ingestor current status usize.
    pub(crate) fn status(&self) -> usize {
        self.status.load()
    }

    /// Returns the ingestor current statustype.
    pub(crate) fn _statustype(&self) -> StatusType {
        StatusType::from(self.status())
    }

    fn check_online(&self) -> bool {
        self.online.load(Ordering::SeqCst)
    }
}

impl Drop for TcpIngestor {
    fn drop(&mut self) {
        self.shutdown()
    }
}
