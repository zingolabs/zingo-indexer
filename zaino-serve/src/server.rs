//! Zaino's gRPC server implementation.

pub mod director;
pub mod error;
pub(crate) mod ingestor;
pub(crate) mod queue;
pub mod request;
pub(crate) mod worker;
