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

    /// Serialization error.
    #[error("Serialization error: {0}")]
    SerializationError(#[from] zebra_chain::serialization::SerializationError),

    /// RPC error in compatibility with zcashd.
    #[error("RPC error: {0:?}")]
    RpcError(#[from] zaino_fetch::jsonrpc::connector::RpcError),

    /// A generic boxed error.
    #[error("Generic error: {0}")]
    Generic(#[from] Box<dyn std::error::Error + Send + Sync>),
}
