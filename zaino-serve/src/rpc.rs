//! Lightwallet service RPC implementations.

use std::sync::{atomic::AtomicBool, Arc};

pub mod service;

#[derive(Debug, Clone)]
/// Configuration data for gRPC server.
pub struct GrpcClient {
    /// Zebrad uri.
    pub zebrad_rpc_uri: http::Uri,
    /// Represents the Online status of the gRPC server.
    pub online: Arc<AtomicBool>,
}
