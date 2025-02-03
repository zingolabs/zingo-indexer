//! Server configuration data.

use std::net::{IpAddr, SocketAddr};

use tonic::transport::{Identity, ServerTlsConfig};

use super::error::ServerError;

/// Configuration data for Zaino's gRPC server.
pub struct GrpcConfig {
    /// gRPC server bind addr.
    pub grpc_listen_address: SocketAddr,
    /// Enables TLS.
    pub tls: bool,
    /// Path to the TLS certificate file in PEM format.
    pub tls_cert_path: Option<String>,
    /// Path to the TLS private key file in PEM format.
    pub tls_key_path: Option<String>,
}

impl GrpcConfig {
    /// Validates that the configured `bind_address` is either:
    /// - An RFC1918 (private) IPv4 address, or
    /// - An IPv6 Unique Local Address (ULA) (using `is_unique_local()`)
    ///
    /// Returns `Ok(BindAddress)` if valid.
    pub fn is_private_listen_addr(&self) -> Result<SocketAddr, ServerError> {
        let ip = self.grpc_listen_address.ip();
        match ip {
            IpAddr::V4(ipv4) => {
                if ipv4.is_private() {
                    Ok(self.grpc_listen_address)
                } else {
                    Err(ServerError::ServerConfigError(format!(
                        "{} is not an RFC1918 IPv4 address",
                        ipv4
                    )))
                }
            }
            IpAddr::V6(ipv6) => {
                if ipv6.is_unique_local() {
                    Ok(self.grpc_listen_address)
                } else {
                    Err(ServerError::ServerConfigError(format!(
                        "{} is not a unique local IPv6 address",
                        ipv6
                    )))
                }
            }
        }
    }

    /// If TLS is enabled, reads the certificate and key files and returns a valid
    /// `ServerTlsConfig`. If TLS is not enabled, returns `Ok(None)`.
    pub async fn get_valid_tls(&self) -> Result<Option<ServerTlsConfig>, ServerError> {
        if self.tls {
            // Ensure the certificate and key paths are provided.
            let cert_path = self.tls_cert_path.as_ref().ok_or_else(|| {
                ServerError::ServerConfigError("TLS enabled but tls_cert_path not provided".into())
            })?;
            let key_path = self.tls_key_path.as_ref().ok_or_else(|| {
                ServerError::ServerConfigError("TLS enabled but tls_key_path not provided".into())
            })?;
            // Read the certificate and key files asynchronously.
            let cert = tokio::fs::read(cert_path).await.map_err(|e| {
                ServerError::ServerConfigError(format!("Failed to read TLS certificate: {}", e))
            })?;
            let key = tokio::fs::read(key_path).await.map_err(|e| {
                ServerError::ServerConfigError(format!("Failed to read TLS key: {}", e))
            })?;
            // Build the identity and TLS configuration.
            let identity = Identity::from_pem(cert, key);
            let tls_config = ServerTlsConfig::new().identity(identity);
            Ok(Some(tls_config))
        } else {
            Ok(None)
        }
    }
}
