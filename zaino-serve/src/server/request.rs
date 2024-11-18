//! Request types.

use crate::server::error::RequestError;
use std::time::SystemTime;
use tokio::net::TcpStream;

/// Requests queuing metadata.
#[derive(Debug, Clone)]
struct QueueData {
    // / Exclusive request id.
    // request_id: u64, // TODO: implement with request queue (implement exlusive request_id generator in queue object).
    /// Time which the request was received.
    #[allow(dead_code)]
    time_received: SystemTime,
    /// Number of times the request has been requeued.
    #[allow(dead_code)]
    requeue_attempts: u32,
}

impl QueueData {
    /// Returns a new instance of QueueData.
    fn new() -> Self {
        QueueData {
            time_received: SystemTime::now(),
            requeue_attempts: 0,
        }
    }

    /// Increases the requeue attempts for the request.
    #[allow(dead_code)]
    pub(crate) fn increase_requeues(&mut self) {
        self.requeue_attempts += 1;
    }

    /// Returns the duration sunce the request was received.
    #[allow(dead_code)]
    fn duration(&self) -> Result<std::time::Duration, RequestError> {
        self.time_received.elapsed().map_err(RequestError::from)
    }

    /// Returns the number of times the request has been requeued.
    #[allow(dead_code)]
    fn requeues(&self) -> u32 {
        self.requeue_attempts
    }
}

/// TcpStream holing an incoming gRPC request.
#[derive(Debug)]
pub(crate) struct TcpRequest(TcpStream);

impl TcpRequest {
    /// Returns the underlying TcpStream help by the request
    pub(crate) fn get_stream(self) -> TcpStream {
        self.0
    }
}

/// Requests originating from the Tcp server.
#[derive(Debug)]
pub struct TcpServerRequest {
    #[allow(dead_code)]
    queuedata: QueueData,
    request: TcpRequest,
}

impl TcpServerRequest {
    /// Returns the underlying request.
    pub(crate) fn get_request(self) -> TcpRequest {
        self.request
    }
}

/// Zingo-Indexer request, used by request queue.
#[derive(Debug)]
pub enum ZingoIndexerRequest {
    /// Requests originating from the gRPC server.
    TcpServerRequest(TcpServerRequest),
}

impl ZingoIndexerRequest {
    /// Creates a ZingoIndexerRequest from a gRPC service call, recieved by the gRPC server.
    ///
    /// TODO: implement proper functionality along with queue.
    pub(crate) fn new_from_grpc(stream: TcpStream) -> Self {
        ZingoIndexerRequest::TcpServerRequest(TcpServerRequest {
            queuedata: QueueData::new(),
            request: TcpRequest(stream),
        })
    }

    /// Increases the requeue attempts for the request.
    #[allow(dead_code)]
    pub(crate) fn increase_requeues(&mut self) {
        match self {
            ZingoIndexerRequest::TcpServerRequest(ref mut req) => req.queuedata.increase_requeues(),
        }
    }

    /// Returns the duration sunce the request was received.
    #[allow(dead_code)]
    pub(crate) fn duration(&self) -> Result<std::time::Duration, RequestError> {
        match self {
            ZingoIndexerRequest::TcpServerRequest(ref req) => req.queuedata.duration(),
        }
    }

    /// Returns the number of times the request has been requeued.
    #[allow(dead_code)]
    pub(crate) fn requeues(&self) -> u32 {
        match self {
            ZingoIndexerRequest::TcpServerRequest(ref req) => req.queuedata.requeues(),
        }
    }
}
