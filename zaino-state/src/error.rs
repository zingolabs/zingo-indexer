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

impl From<StateServiceError> for tonic::Status {
    fn from(error: StateServiceError) -> Self {
        match error {
            StateServiceError::Custom(message) => tonic::Status::internal(message),
            StateServiceError::JoinError(err) => {
                tonic::Status::internal(format!("Join error: {}", err))
            }
            StateServiceError::JsonRpcConnectorError(err) => {
                tonic::Status::internal(format!("JsonRpcConnector error: {}", err))
            }
            StateServiceError::RpcError(err) => {
                tonic::Status::internal(format!("RPC error: {:?}", err))
            }
            StateServiceError::TonicStatusError(err) => err,
            StateServiceError::SerializationError(err) => {
                tonic::Status::internal(format!("Serialization error: {}", err))
            }
            StateServiceError::TryFromIntError(err) => {
                tonic::Status::internal(format!("Integer conversion error: {}", err))
            }
            StateServiceError::IoError(err) => {
                tonic::Status::internal(format!("IO error: {}", err))
            }
            StateServiceError::Generic(err) => {
                tonic::Status::internal(format!("Generic error: {}", err))
            }
        }
    }
}

/// Errors related to the `StateService`.
#[derive(Debug, thiserror::Error)]
pub enum FetchServiceError {
    /// Custom Errors. *Remove before production.
    #[error("Custom error: {0}")]
    Custom(String),

    /// Error from a Tokio JoinHandle.
    #[error("Join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),

    /// Error from JsonRpcConnector.
    #[error("JsonRpcConnector error: {0}")]
    JsonRpcConnectorError(#[from] zaino_fetch::jsonrpc::error::JsonRpcConnectorError),

    /// Error from the mempool.
    #[error("Mempool error: {0}")]
    MempoolError(#[from] MempoolError),

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

    /// UTF-8 conversion error.
    #[error("UTF-8 conversion error: {0}")]
    Utf8Error(#[from] std::str::Utf8Error),

    /// Integer parsing error.
    #[error("Integer parsing error: {0}")]
    ParseIntError(#[from] std::num::ParseIntError),

    /// Chain parse error.
    #[error("Chain parse error: {0}")]
    ChainParseError(#[from] zaino_fetch::chain::error::ParseError),

    /// std::io::Error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// A generic boxed error.
    #[error("Generic error: {0}")]
    Generic(#[from] Box<dyn std::error::Error + Send + Sync>),
}

impl From<FetchServiceError> for tonic::Status {
    fn from(error: FetchServiceError) -> Self {
        match error {
            FetchServiceError::Custom(message) => tonic::Status::internal(message),
            FetchServiceError::JoinError(err) => {
                tonic::Status::internal(format!("Join error: {}", err))
            }
            FetchServiceError::JsonRpcConnectorError(err) => {
                tonic::Status::internal(format!("JsonRpcConnector error: {}", err))
            }
            FetchServiceError::MempoolError(err) => {
                tonic::Status::internal(format!("Mempool error: {}", err))
            }
            FetchServiceError::RpcError(err) => {
                tonic::Status::internal(format!("RPC error: {:?}", err))
            }
            FetchServiceError::TonicStatusError(err) => err,
            FetchServiceError::SerializationError(err) => {
                tonic::Status::internal(format!("Serialization error: {}", err))
            }
            FetchServiceError::TryFromIntError(err) => {
                tonic::Status::internal(format!("Integer conversion error: {}", err))
            }
            FetchServiceError::Utf8Error(err) => {
                tonic::Status::internal(format!("UTF-8 conversion error: {}", err))
            }
            FetchServiceError::ParseIntError(err) => {
                tonic::Status::internal(format!("Integer parsing error: {}", err))
            }
            FetchServiceError::ChainParseError(err) => {
                tonic::Status::internal(format!("Chain parse error: {}", err))
            }
            FetchServiceError::IoError(err) => {
                tonic::Status::internal(format!("IO error: {}", err))
            }
            FetchServiceError::Generic(err) => {
                tonic::Status::internal(format!("Generic error: {}", err))
            }
        }
    }
}

/// Errors related to the `StateService`.
#[derive(Debug, thiserror::Error)]
pub enum MempoolError {
    /// Custom Errors. *Remove before production.
    #[error("Custom error: {0}")]
    Custom(String),

    /// Error from a Tokio JoinHandle.
    #[error("Join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),

    /// Error from JsonRpcConnector.
    #[error("JsonRpcConnector error: {0}")]
    JsonRpcConnectorError(#[from] zaino_fetch::jsonrpc::error::JsonRpcConnectorError),

    /// Error from a Tokio Watch Receiver.
    #[error("Join error: {0}")]
    WatchRecvError(#[from] tokio::sync::watch::error::RecvError),

    /// Unexpected status-related error.
    #[error("Status error: {0:?}")]
    StatusError(StatusError),

    /// Error from sending to a Tokio MPSC channel.
    #[error("Send error: {0}")]
    SendError(
        #[from]
        tokio::sync::mpsc::error::SendError<
            Result<(crate::mempool::MempoolKey, crate::mempool::MempoolValue), StatusError>,
        >,
    ),

    /// A generic boxed error.
    #[error("Generic error: {0}")]
    Generic(#[from] Box<dyn std::error::Error + Send + Sync>),
}

/// A general error type to represent error StatusTypes.
#[derive(Debug, Clone, thiserror::Error)]
#[error("Unexpected status error: {0:?}")]
pub struct StatusError(pub crate::status::StatusType);
