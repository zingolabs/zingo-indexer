//! Lightwallet service RPC implementations.

use std::sync::{atomic::AtomicBool, Arc};

use zaino_state::{fetch::FetchServiceSubscriber, indexer::IndexerSubscriber};

pub mod service;

#[derive(Clone)]
/// Configuration data for gRPC server.
pub struct GrpcClient {
    /// Zebrad uri.
    pub zebrad_rpc_uri: http::Uri,
    /// Chain fetch service subscriber.
    pub service_subscriber: IndexerSubscriber<FetchServiceSubscriber>,
    /// Represents the Online status of the gRPC server.
    pub online: Arc<AtomicBool>,
}
