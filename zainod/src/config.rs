//! Zaino config.

use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

use tracing::warn;

use crate::error::IndexerError;

/// Config information required for Zaino.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct IndexerConfig {
    /// gRPC server bind addr.
    pub grpc_listen_address: SocketAddr,
    /// Enables TLS.
    pub tls: bool,
    /// Path to the TLS certificate file in PEM format.
    pub tls_cert_path: Option<String>,
    /// Path to the TLS private key file in PEM format.
    pub tls_key_path: Option<String>,
    /// Full node / validator listen port.
    pub zebrad_port: u16,
    /// Full node / validator Username.
    pub node_user: Option<String>,
    /// full node / validator Password.
    pub node_password: Option<String>,
    /// Capacity of the Dashmaps used for the Mempool.
    /// Also use by the BlockCache::NonFinalisedState when using the FetchService.
    ///
    /// NOTE: map_capacity and shard map must both be set for either to be used.
    pub map_capacity: Option<usize>,
    /// Number of shard used in the DashMap used for the Mempool.
    /// Also use by the BlockCache::NonFinalisedState when using the FetchService.
    ///
    /// shard_amount should greater than 0 and be a power of two.
    /// If a shard_amount which is not a power of two is provided, the function will panic.
    ///
    /// NOTE: map_capacity and shard map must both be set for either to be used.
    pub map_shard_amount: Option<usize>,
    /// Block Cache database file path.
    ///
    /// This is Zaino's Compact Block Cache db if using the FetchService or Zebra's RocksDB if using the StateService.
    pub db_path: PathBuf,
    /// Block Cache database maximum size in gb.
    ///
    /// Only used by the FetchService.
    pub db_size: Option<usize>,
    /// Network chain type (Mainnet, Testnet, Regtest).
    pub network: String,
    /// Disables internal sync and stops zaino waiting on server sync.
    /// Used for testing.
    pub no_sync: bool,
    /// Disables FinalisedState.
    /// Used for testing.
    pub no_db: bool,
    /// Disables internal mempool and blockcache.
    ///
    /// For use by lightweight wallets that do not want to run any extra processes.
    ///
    /// NOTE: Currently unimplemented as will require either a Tonic backend or a JsonRPC server.
    pub no_state: bool,
}

impl IndexerConfig {
    /// Performs checks on config data.
    ///
    /// - Checks that at least 1 ingestor is active.
    pub(crate) fn check_config(&self) -> Result<(), IndexerError> {
        if (self.network != "Regtest") && (self.network != "Testnet") && (self.network != "Mainnet")
        {
            return Err(IndexerError::ConfigError(
                "Incorrect network name given.".to_string(),
            ));
        }

        if self.loopback_listen_addr().is_err() && !self.tls {
            return Err(IndexerError::ConfigError(
                "TLS required when connecting to external addresses.".to_string(),
            ));
        }

        Ok(())
    }

    /// Validates that the configured `bind_address` is either:
    /// - An RFC1918 (private) IPv4 address, or
    /// - An IPv6 Unique Local Address (ULA) (using `is_unique_local()`)
    ///
    /// Returns `Ok(BindAddress)` if valid.
    pub(crate) fn private_listen_addr(&self) -> Result<SocketAddr, IndexerError> {
        let ip = self.grpc_listen_address.ip();
        match ip {
            IpAddr::V4(ipv4) => {
                if ipv4.is_private() {
                    Ok(self.grpc_listen_address)
                } else {
                    Err(IndexerError::ConfigError(format!(
                        "{} is not an RFC1918 IPv4 address",
                        ipv4
                    )))
                }
            }
            IpAddr::V6(ipv6) => {
                if ipv6.is_unique_local() {
                    Ok(self.grpc_listen_address)
                } else {
                    Err(IndexerError::ConfigError(format!(
                        "{} is not a unique local IPv6 address",
                        ipv6
                    )))
                }
            }
        }
    }

    /// Validates that the configured `bind_address` is a loopback address.
    ///
    /// Returns `Ok(BindAddress)` if valid.
    pub(crate) fn loopback_listen_addr(&self) -> Result<SocketAddr, IndexerError> {
        let ip = self.grpc_listen_address.ip();
        match ip {
            IpAddr::V4(ipv4) => {
                if ipv4.is_loopback() {
                    Ok(self.grpc_listen_address)
                } else {
                    Err(IndexerError::ConfigError(format!(
                        "{} is not an RFC1918 IPv4 address",
                        ipv4
                    )))
                }
            }
            IpAddr::V6(ipv6) => {
                if ipv6.is_loopback() {
                    Ok(self.grpc_listen_address)
                } else {
                    Err(IndexerError::ConfigError(format!(
                        "{} is not a unique local IPv6 address",
                        ipv6
                    )))
                }
            }
        }
    }

    /// Returns the network type currently being used by the server.
    pub fn get_network(&self) -> Result<zebra_chain::parameters::Network, IndexerError> {
        match self.network.as_str() {
            "Regtest" => Ok(zebra_chain::parameters::Network::new_regtest(
                Some(1),
                Some(1),
            )),
            "Testnet" => Ok(zebra_chain::parameters::Network::new_default_testnet()),
            "Mainnet" => Ok(zebra_chain::parameters::Network::Mainnet),
            _ => Err(IndexerError::ConfigError(
                "Incorrect network name given.".to_string(),
            )),
        }
    }
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            grpc_listen_address: "127.0.0.1:8137".parse().unwrap(),
            tls: false,
            tls_cert_path: None,
            tls_key_path: None,
            zebrad_port: 18232,
            node_user: Some("xxxxxx".to_string()),
            node_password: Some("xxxxxx".to_string()),
            map_capacity: None,
            map_shard_amount: None,
            db_path: default_db_path(),
            db_size: None,
            network: "Testnet".to_string(),
            no_sync: false,
            no_db: false,
            no_state: false,
        }
    }
}

/// Loads the default file path for zaino's local db.
fn default_db_path() -> PathBuf {
    match std::env::var("HOME") {
        Ok(home) => PathBuf::from(home).join(".cache").join("zaino"),
        Err(_) => PathBuf::from("/tmp").join("zaino"),
    }
}

/// Attempts to load config data from a toml file at the specified path else returns a default config.
pub fn load_config(file_path: &std::path::PathBuf) -> IndexerConfig {
    if let Ok(contents) = std::fs::read_to_string(file_path) {
        toml::from_str::<IndexerConfig>(&contents).unwrap_or_default()
    } else {
        warn!("Could not find config file at given path, using default config.");
        IndexerConfig::default()
    }
}
