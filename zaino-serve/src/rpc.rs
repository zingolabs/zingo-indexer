//! Lightwallet service RPC implementations.

use zaino_state::{fetch::FetchServiceSubscriber, indexer::IndexerSubscriber};

pub mod service;

#[derive(Clone)]
/// Zaino gRPC service.
pub struct GrpcClient {
    /// Chain fetch service subscriber.
    pub service_subscriber: IndexerSubscriber<FetchServiceSubscriber>,
}
