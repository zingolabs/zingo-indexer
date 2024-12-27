//! Holds error types for Zaino-state.

/// Errors related to the `StateService`.
#[derive(Debug, thiserror::Error)]
pub enum StateServiceError {
    /// Custom Errors. *Remove before production.
    #[error("Custom error: {0}")]
    Custom(String),

    /// Error from a Tokio JoinHandle.
    #[error("Join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),

    /// Error from JsonRpcConnector.
    #[error("JsonRpcConnector error: {0}")]
    JsonRpcConnectorError(#[from] zaino_fetch::jsonrpc::error::JsonRpcConnectorError),

    /// RPC error in compatibility with zcashd.
    #[error("RPC error: {0:?}")]
    RpcError(#[from] zaino_fetch::jsonrpc::connector::RpcError),

    /// Tonic gRPC error.
    #[error("Tonic status error: {0}")]
    TonicStatusError(#[from] tonic::Status),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    SerializationError(#[from] zebra_chain::serialization::SerializationError),

    /// Integer conversion error.
    #[error("Integer conversion error: {0}")]
    TryFromIntError(#[from] std::num::TryFromIntError),

    /// std::io::Error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// A generic boxed error.
    #[error("Generic error: {0}")]
    Generic(#[from] Box<dyn std::error::Error + Send + Sync>),
}
