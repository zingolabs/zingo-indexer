//! Hold error types for the JsonRpcConnector and related functionality.

/// General error type for handling JsonRpcConnector errors.
#[derive(Debug, thiserror::Error)]
pub enum JsonRpcConnectorError {
    /// Type for errors without an underlying source.
    #[error("Error: {0}")]
    JsonRpcClientError(String),

    /// Serialization/Deserialization Errors.
    #[error("Error: Serialization/Deserialization Error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),

    /// Reqwest Based Errors.
    #[error("Error: HTTP Request Error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    /// Invalid URI Errors.
    #[error("Error: Invalid URI: {0}")]
    InvalidUriError(#[from] http::uri::InvalidUri),

    /// URL Parse Errors.
    #[error("Error: Invalid URL:{0}")]
    UrlParseError(#[from] url::ParseError),
}

impl JsonRpcConnectorError {
    /// Constructor for errors without an underlying source
    pub fn new(msg: impl Into<String>) -> Self {
        JsonRpcConnectorError::JsonRpcClientError(msg.into())
    }

    /// Converts JsonRpcConnectorError to tonic::Status
    ///
    /// TODO: This impl should be changed to return the correct status [https://github.com/zcash/lightwalletd/issues/497] before release,
    ///       however propagating the server error is useful durin development.
    pub fn to_grpc_status(&self) -> tonic::Status {
        // TODO: Hide server error from clients before release. Currently useful for dev purposes.
        tonic::Status::internal(format!("Error: JsonRPC Client Error: {}", self))
    }
}

impl From<JsonRpcConnectorError> for tonic::Status {
    fn from(err: JsonRpcConnectorError) -> Self {
        err.to_grpc_status()
    }
}
