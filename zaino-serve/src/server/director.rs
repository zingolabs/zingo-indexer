//! Zingo-Indexer gRPC server.

use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};

use crate::server::{
    error::{IngestorError, ServerError, WorkerError},
    ingestor::TcpIngestor,
    queue::Queue,
    request::ZingoIndexerRequest,
    worker::{WorkerPool, WorkerPoolStatus},
};
use zaino_state::{
    fetch::FetchServiceSubscriber,
    indexer::IndexerSubscriber,
    status::{AtomicStatus, StatusType},
};

/// Holds the status of the server and all its components.
#[derive(Debug, Clone)]
pub struct ServerStatus {
    /// Status of the Server.
    pub server_status: AtomicStatus,
    /// Status of the chain fetch service.
    pub service_status: AtomicStatus,
    tcp_ingestor_status: AtomicStatus,
    workerpool_status: WorkerPoolStatus,
    request_queue_status: Arc<AtomicUsize>,
}

impl ServerStatus {
    /// Creates a ServerStatus.
    pub fn new(max_workers: u16) -> Self {
        ServerStatus {
            server_status: AtomicStatus::new(StatusType::Offline.into()),
            service_status: AtomicStatus::new(StatusType::Offline.into()),
            tcp_ingestor_status: AtomicStatus::new(StatusType::Offline.into()),
            workerpool_status: WorkerPoolStatus::new(max_workers),
            request_queue_status: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Returns the ServerStatus.
    pub fn load(&self) -> ServerStatus {
        self.server_status.load();
        self.tcp_ingestor_status.load();
        self.workerpool_status.load();
        self.request_queue_status.load(Ordering::SeqCst);
        self.clone()
    }
}

/// LightWallet server capable of servicing clients over TCP.
pub struct Server {
    /// Listens for incoming gRPC requests over HTTP.
    tcp_ingestor: Option<TcpIngestor>,
    /// Chain fetch service subscriber.
    _service_subscriber: IndexerSubscriber<FetchServiceSubscriber>,
    /// Dynamically sized pool of workers.
    worker_pool: WorkerPool,
    /// Request queue.
    request_queue: Queue<ZingoIndexerRequest>,
    /// Servers current status.
    status: ServerStatus,
    /// Represents the Online status of the Server.
    pub online: Arc<AtomicBool>,
}

impl Server {
    /// Spawns a new Server.
    #[allow(clippy::too_many_arguments)]
    pub async fn spawn(
        service_subscriber: IndexerSubscriber<FetchServiceSubscriber>,
        tcp_active: bool,
        tcp_ingestor_listen_addr: Option<SocketAddr>,
        max_queue_size: u16,
        max_worker_pool_size: u16,
        idle_worker_pool_size: u16,
        status: ServerStatus,
        online: Arc<AtomicBool>,
    ) -> Result<Self, ServerError> {
        if !tcp_active {
            return Err(ServerError::ServerConfigError(
                "Cannot start server with no ingestors selected.".to_string(),
            ));
        }
        if tcp_active && tcp_ingestor_listen_addr.is_none() {
            return Err(ServerError::ServerConfigError(
                "TCP is active but no address provided.".to_string(),
            ));
        }
        println!("Launching Server..");
        status.server_status.store(StatusType::Spawning.into());
        let request_queue: Queue<ZingoIndexerRequest> =
            Queue::new(max_queue_size as usize, status.request_queue_status.clone());
        status.request_queue_status.store(0, Ordering::SeqCst);
        let tcp_ingestor = if tcp_active {
            println!("Launching TcpIngestor..");
            Some(
                TcpIngestor::spawn(
                    tcp_ingestor_listen_addr
                        .expect("tcp_ingestor_listen_addr returned none when used."),
                    request_queue.tx().clone(),
                    status.tcp_ingestor_status.clone(),
                    online.clone(),
                )
                .await?,
            )
        } else {
            None
        };
        println!("Launching WorkerPool..");
        let worker_pool = WorkerPool::spawn(
            service_subscriber.clone(),
            status.service_status.clone(),
            max_worker_pool_size,
            idle_worker_pool_size,
            request_queue.rx().clone(),
            request_queue.tx().clone(),
            status.workerpool_status.clone(),
            online.clone(),
        )
        .await;
        Ok(Server {
            _service_subscriber: service_subscriber,
            tcp_ingestor,
            worker_pool,
            request_queue,
            status: status.clone(),
            online,
        })
    }

    /// Starts the gRPC service.
    ///
    /// Launches all components then enters command loop:
    /// - Checks request queue and workerpool to spawn / despawn workers as required.
    /// - Updates the ServerStatus.
    /// - Checks for shutdown signal, shutting down server if received.
    pub async fn serve(mut self) -> tokio::task::JoinHandle<Result<(), ServerError>> {
        tokio::task::spawn(async move {
            // NOTE: This interval may need to be reduced or removed / moved once scale testing begins.
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(50));
            let mut tcp_ingestor_handle = None;
            let mut worker_handles;
            if let Some(ingestor) = self.tcp_ingestor.take() {
                tcp_ingestor_handle = Some(ingestor.serve().await);
            }
            worker_handles = self.worker_pool.clone().serve().await;
            self.status.server_status.store(StatusType::Ready.into());
            loop {
                if self.request_queue.queue_length() >= (self.request_queue.max_length() / 4)
                    && (self.worker_pool.workers() < self.worker_pool.max_size() as usize)
                {
                    match self.worker_pool.push_worker().await {
                        Ok(handle) => {
                            worker_handles.push(handle);
                        }
                        Err(_e) => {
                            eprintln!("WorkerPool at capacity");
                        }
                    }
                } else if (self.request_queue.queue_length() <= 1)
                    && (self.worker_pool.workers() > self.worker_pool.idle_size() as usize)
                {
                    let worker_index = self.worker_pool.workers() - 1;
                    let worker_handle = worker_handles.remove(worker_index);
                    match self.worker_pool.pop_worker(worker_handle).await {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Failed to pop worker from pool: {}", e);
                            // TODO: Handle this error.
                        }
                    }
                }
                self.statuses();
                // TODO: Implement check_statuses() and run here.
                if self.check_for_shutdown().await {
                    self.status.server_status.store(StatusType::Closing.into());
                    let worker_handle_options: Vec<
                        Option<tokio::task::JoinHandle<Result<(), WorkerError>>>,
                    > = worker_handles.into_iter().map(Some).collect();
                    self.shutdown_components(tcp_ingestor_handle, worker_handle_options)
                        .await;
                    self.status.server_status.store(StatusType::Offline.into());
                    return Ok(());
                }
                interval.tick().await;
            }
        })
    }

    /// Checks indexers online status and servers internal status for closure signal.
    pub async fn check_for_shutdown(&self) -> bool {
        if self.status() >= 4 {
            return true;
        }
        if !self.check_online() {
            return true;
        }
        false
    }

    /// Sets the servers to close gracefully.
    pub async fn shutdown(&mut self) {
        self.status.server_status.store(StatusType::Closing.into())
    }

    /// Sets the server's components to close gracefully.
    async fn shutdown_components(
        &mut self,
        tcp_ingestor_handle: Option<tokio::task::JoinHandle<Result<(), IngestorError>>>,
        mut worker_handles: Vec<Option<tokio::task::JoinHandle<Result<(), WorkerError>>>>,
    ) {
        if let Some(handle) = tcp_ingestor_handle {
            self.status
                .tcp_ingestor_status
                .store(StatusType::Closing.into());
            handle.await.ok();
        }
        self.worker_pool.shutdown(&mut worker_handles).await;
    }

    /// Returns the servers current status usize.
    pub fn status(&self) -> usize {
        self.status.server_status.load()
    }

    /// Returns the servers current statustype.
    pub fn statustype(&self) -> StatusType {
        StatusType::from(self.status())
    }

    /// Updates and returns the status of the server and its parts.
    pub fn statuses(&mut self) -> ServerStatus {
        self.status.server_status.load();
        self.status.tcp_ingestor_status.load();
        self.status
            .request_queue_status
            .store(self.request_queue.queue_length(), Ordering::SeqCst);
        self.worker_pool.status();
        self.status.clone()
    }

    /// Checks statuses, handling errors.
    pub async fn check_statuses(&mut self) {
        todo!()
    }

    /// Check the online status on the indexer.
    fn check_online(&self) -> bool {
        self.online.load(Ordering::SeqCst)
    }
}
