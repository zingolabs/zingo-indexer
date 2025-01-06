//! Holds the server worker implementation.

use http::Uri;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use tonic::transport::Server;

use crate::{
    rpc::GrpcClient,
    server::{
        error::WorkerError,
        queue::{QueueReceiver, QueueSender},
        request::ZingoIndexerRequest,
    },
};
use zaino_proto::proto::service::compact_tx_streamer_server::CompactTxStreamerServer;
use zaino_state::{
    fetch::FetchServiceSubscriber,
    indexer::IndexerSubscriber,
    status::{AtomicStatus, StatusType},
};

/// A queue working is the entity that takes requests from the queue and processes them.
///
/// TODO: - Add JsonRpcConnector to worker and pass to underlying RPC services.
///       - Currently a new JsonRpcConnector is spawned for every new RPC serviced.
#[derive(Clone)]
pub(crate) struct Worker {
    /// Worker ID.
    _worker_id: usize,
    /// Used to pop requests from the queue
    queue: QueueReceiver<ZingoIndexerRequest>,
    /// Used to requeue requests.
    requeue: QueueSender<ZingoIndexerRequest>,
    /// Chain fetch service subscriber.
    _service_subscriber: IndexerSubscriber<FetchServiceSubscriber>,
    /// Service status.
    _service_status: AtomicStatus,
    /// gRPC client used for processing requests received over http.
    grpc_client: GrpcClient,
    /// Thread safe worker status.
    atomic_status: AtomicStatus,
    /// Represents the Online status of the Worker.
    pub online: Arc<AtomicBool>,
}

impl Worker {
    /// Creates a new queue worker.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn spawn(
        _worker_id: usize,
        queue: QueueReceiver<ZingoIndexerRequest>,
        requeue: QueueSender<ZingoIndexerRequest>,
        service_subscriber: IndexerSubscriber<FetchServiceSubscriber>,
        service_status: AtomicStatus,
        zebrad_uri: Uri,
        atomic_status: AtomicStatus,
        online: Arc<AtomicBool>,
    ) -> Self {
        let grpc_client = GrpcClient {
            service_subscriber: service_subscriber.clone(),
            zebrad_rpc_uri: zebrad_uri,
            online: online.clone(),
        };
        Worker {
            _worker_id,
            queue,
            requeue,
            _service_subscriber: service_subscriber,
            _service_status: service_status,
            grpc_client,
            atomic_status,
            online,
        }
    }

    /// Starts queue worker service routine.
    ///
    /// TODO: Add requeue logic for node errors.
    pub(crate) async fn serve(self) -> tokio::task::JoinHandle<Result<(), WorkerError>> {
        tokio::task::spawn(async move {
            // NOTE: This interval may need to be reduced or removed / moved once scale testing begins.
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));
            let svc = CompactTxStreamerServer::new(self.grpc_client.clone());
            // TODO: create tonic server here for use within loop.
            self.atomic_status.store(StatusType::Ready.into());
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if self.check_for_shutdown().await {
                            return Ok(());
                        }
                    }
                    incoming = self.queue.listen() => {
                        match incoming {
                            Ok(request) => {
                                self.atomic_status.store(StatusType::Busy.into());
                                    match request {
                                        ZingoIndexerRequest::TcpServerRequest(request) => {
                                            Server::builder().add_service(svc.clone())
                                                .serve_with_incoming( async_stream::stream! {
                                                    yield Ok::<_, std::io::Error>(
                                                        request.get_request().get_stream()
                                                    );
                                                }
                                            )
                                            .await?;
                                        }
                                    }
                                // NOTE: This may need to be removed for scale use.
                                if self.check_for_shutdown().await {
                                    self.atomic_status.store(StatusType::Offline.into());
                                    return Ok(());
                                } else {
                                    self.atomic_status.store(StatusType::Ready.into());
                                }
                            }
                            Err(_e) => {
                                self.atomic_status.store(StatusType::Offline.into());
                                eprintln!("Queue closed, worker shutting down.");
                                // TODO: Handle queue closed error here. (return correct error / undate status to correct err code.)
                                return Ok(());
                            }
                        }
                    }
                }
            }
        })
    }

    /// Checks for closure signals.
    ///
    /// Checks AtomicStatus for closure signal.
    /// Checks (online) AtomicBool for fatal error signal.
    pub(crate) async fn check_for_shutdown(&self) -> bool {
        if self.atomic_status() >= 4 {
            return true;
        }
        if !self.check_online() {
            return true;
        }
        false
    }

    /// Sets the worker to close gracefully.
    pub(crate) async fn shutdown(&mut self) {
        self.atomic_status.store(StatusType::Closing.into())
    }

    /// Returns the worker's ID.
    pub(crate) fn _id(&self) -> usize {
        self._worker_id
    }

    /// Loads the workers current atomic status.
    pub(crate) fn atomic_status(&self) -> usize {
        self.atomic_status.load()
    }

    /// Check the online status on the server.
    fn check_online(&self) -> bool {
        self.online.load(Ordering::SeqCst)
    }
}

/// Holds the status of the worker pool and its workers.
#[derive(Debug, Clone)]
pub struct WorkerPoolStatus {
    workers: Arc<AtomicUsize>,
    statuses: Vec<AtomicStatus>,
}

impl WorkerPoolStatus {
    /// Creates a WorkerPoolStatus.
    pub(crate) fn new(max_workers: u16) -> Self {
        WorkerPoolStatus {
            workers: Arc::new(AtomicUsize::new(0)),
            statuses: vec![AtomicStatus::new(StatusType::Offline.into()); max_workers as usize],
        }
    }

    /// Returns the WorkerPoolStatus.
    pub(crate) fn load(&self) -> WorkerPoolStatus {
        self.workers.load(Ordering::SeqCst);
        for i in 0..self.statuses.len() {
            self.statuses[i].load();
        }
        self.clone()
    }
}

/// Dynamically sized pool of workers.
#[derive(Clone)]
pub(crate) struct WorkerPool {
    /// Chain fetch service subscriber.
    service_subscriber: IndexerSubscriber<FetchServiceSubscriber>,
    /// Service status.
    service_status: AtomicStatus,
    /// Maximun number of concurrent workers allowed.
    max_size: u16,
    /// Minimum number of workers kept running on stanby.
    idle_size: u16,
    /// Workers currently in the pool
    workers: Vec<Worker>,
    /// Status of the workerpool and its workers.
    status: WorkerPoolStatus,
    /// Represents the Online status of the WorkerPool.
    pub online: Arc<AtomicBool>,
}

impl WorkerPool {
    /// Creates a new worker pool containing [idle_workers] workers.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn spawn(
        service_subscriber: IndexerSubscriber<FetchServiceSubscriber>,
        service_status: AtomicStatus,
        max_size: u16,
        idle_size: u16,
        queue: QueueReceiver<ZingoIndexerRequest>,
        _requeue: QueueSender<ZingoIndexerRequest>,
        zebrad_uri: Uri,
        status: WorkerPoolStatus,
        online: Arc<AtomicBool>,
    ) -> Self {
        let mut workers: Vec<Worker> = Vec::with_capacity(max_size as usize);
        for _ in 0..idle_size {
            workers.push(
                Worker::spawn(
                    workers.len(),
                    queue.clone(),
                    _requeue.clone(),
                    service_subscriber.clone(),
                    service_status.clone(),
                    zebrad_uri.clone(),
                    status.statuses[workers.len()].clone(),
                    online.clone(),
                )
                .await,
            );
        }
        status.workers.store(idle_size as usize, Ordering::SeqCst);
        WorkerPool {
            service_subscriber,
            service_status,
            max_size,
            idle_size,
            workers,
            status,
            online,
        }
    }

    /// Sets workers in the worker pool to start servicing the queue.
    pub(crate) async fn serve(self) -> Vec<tokio::task::JoinHandle<Result<(), WorkerError>>> {
        let mut worker_handles = Vec::new();
        for worker in self.workers {
            worker_handles.push(worker.serve().await);
        }
        worker_handles
    }

    /// Adds a worker to the worker pool, returns error if the pool is already at max size.
    pub(crate) async fn push_worker(
        &mut self,
    ) -> Result<tokio::task::JoinHandle<Result<(), WorkerError>>, WorkerError> {
        if self.workers.len() >= self.max_size as usize {
            Err(WorkerError::WorkerPoolFull)
        } else {
            let worker_index = self.workers();
            self.workers.push(
                Worker::spawn(
                    worker_index,
                    self.workers[0].queue.clone(),
                    self.workers[0].requeue.clone(),
                    self.service_subscriber.clone(),
                    self.service_status.clone(),
                    self.workers[0].grpc_client.zebrad_rpc_uri.clone(),
                    self.status.statuses[worker_index].clone(),
                    self.online.clone(),
                )
                .await,
            );
            self.status.workers.fetch_add(1, Ordering::SeqCst);
            Ok(self.workers[worker_index].clone().serve().await)
        }
    }

    /// Removes a worker from the worker pool, returns error if the pool is already at idle size.
    pub(crate) async fn pop_worker(
        &mut self,
        worker_handle: tokio::task::JoinHandle<Result<(), WorkerError>>,
    ) -> Result<(), WorkerError> {
        if self.workers.len() <= self.idle_size as usize {
            Err(WorkerError::WorkerPoolIdle)
        } else {
            let worker_index = self.workers.len() - 1;
            self.workers[worker_index].shutdown().await;
            match worker_handle.await {
                Ok(worker) => match worker {
                    Ok(()) => {
                        self.status.statuses[worker_index].store(5);
                        self.workers.pop();
                        self.status.workers.fetch_sub(1, Ordering::SeqCst);
                        Ok(())
                    }
                    Err(e) => {
                        self.status.statuses[worker_index].store(6);
                        eprintln!("Worker returned error on shutdown: {}", e);
                        // TODO: Handle the inner WorkerError. Return error.
                        self.status.workers.fetch_sub(1, Ordering::SeqCst);
                        Ok(())
                    }
                },
                Err(e) => {
                    self.status.statuses[worker_index].store(6);
                    eprintln!("Worker returned error on shutdown: {}", e);
                    // TODO: Handle the JoinError. Return error.
                    self.status.workers.fetch_sub(1, Ordering::SeqCst);
                    Ok(())
                }
            }
        }
    }

    /// Returns the max size of the pool
    pub(crate) fn max_size(&self) -> u16 {
        self.max_size
    }

    /// Returns the idle size of the pool
    pub(crate) fn idle_size(&self) -> u16 {
        self.idle_size
    }

    /// Returns the current number of workers in the pool.
    pub(crate) fn workers(&self) -> usize {
        self.workers.len()
    }

    /// Fetches and returns the status of the workerpool and its workers.
    pub(crate) fn status(&self) -> WorkerPoolStatus {
        self.status.workers.load(Ordering::SeqCst);
        for i in 0..self.workers() {
            self.status.statuses[i].load();
        }
        self.status.clone()
    }

    /// Shuts down all the workers in the pool.
    pub(crate) async fn shutdown(
        &mut self,
        worker_handles: &mut [Option<tokio::task::JoinHandle<Result<(), WorkerError>>>],
    ) {
        for i in (0..self.workers.len()).rev() {
            self.workers[i].shutdown().await;
            if let Some(worker_handle) = worker_handles[i].take() {
                match worker_handle.await {
                    Ok(worker) => match worker {
                        Ok(()) => {
                            self.status.statuses[i].store(5);
                            self.workers.pop();
                            self.status.workers.fetch_sub(1, Ordering::SeqCst);
                        }
                        Err(e) => {
                            self.status.statuses[i].store(6);
                            eprintln!("Worker returned error on shutdown: {}", e);
                            // TODO: Handle the inner WorkerError
                            self.status.workers.fetch_sub(1, Ordering::SeqCst);
                        }
                    },
                    Err(e) => {
                        self.status.statuses[i].store(6);
                        eprintln!("Worker returned error on shutdown: {}", e);
                        // TODO: Handle the JoinError
                        self.status.workers.fetch_sub(1, Ordering::SeqCst);
                    }
                };
            }
        }
    }
}
