//! Zaino config.

use std::{
    net::{IpAddr, SocketAddr, ToSocketAddrs},
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
    pub grpc_tls: bool,
    /// Path to the TLS certificate file.
    pub tls_cert_path: Option<String>,
    /// Path to the TLS private key file.
    pub tls_key_path: Option<String>,
    /// Full node / validator listen port.
    pub validator_listen_address: SocketAddr,
    /// Enable validator rpc cookie authentification.
    pub validator_cookie_auth: bool,
    /// Path to the validator cookie file.
    pub validator_cookie_path: Option<String>,
    /// Full node / validator Username.
    pub validator_user: Option<String>,
    /// full node / validator Password.
    pub validator_password: Option<String>,
    /// Capacity of the Dashmaps used for the Mempool.
    /// Also use by the BlockCache::NonFinalisedState when using the FetchService.
    pub map_capacity: Option<usize>,
    /// Number of shard used in the DashMap used for the Mempool.
    /// Also use by the BlockCache::NonFinalisedState when using the FetchService.
    ///
    /// shard_amount should greater than 0 and be a power of two.
    /// If a shard_amount which is not a power of two is provided, the function will panic.
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
    pub(crate) fn check_config(&self) -> Result<(), IndexerError> {
        // Check network type.
        if (self.network != "Regtest") && (self.network != "Testnet") && (self.network != "Mainnet")
        {
            return Err(IndexerError::ConfigError(
                "Incorrect network name given, must be one of (Mainnet, Testnet, Regtest)."
                    .to_string(),
            ));
        }

        // Check TLS settings.
        if self.grpc_tls {
            if let Some(ref cert_path) = self.tls_cert_path {
                if !std::path::Path::new(cert_path).exists() {
                    return Err(IndexerError::ConfigError(format!(
                        "TLS is enabled, but certificate path '{}' does not exist.",
                        cert_path
                    )));
                }
            } else {
                return Err(IndexerError::ConfigError(
                    "TLS is enabled, but no certificate path is provided.".to_string(),
                ));
            }

            if let Some(ref key_path) = self.tls_key_path {
                if !std::path::Path::new(key_path).exists() {
                    return Err(IndexerError::ConfigError(format!(
                        "TLS is enabled, but key path '{}' does not exist.",
                        key_path
                    )));
                }
            } else {
                return Err(IndexerError::ConfigError(
                    "TLS is enabled, but no key path is provided.".to_string(),
                ));
            }
        }

        // Check validator cookie authentication settings
        if self.validator_cookie_auth {
            if let Some(ref cookie_path) = self.validator_cookie_path {
                if !std::path::Path::new(cookie_path).exists() {
                    return Err(IndexerError::ConfigError(
                        format!("Validator cookie authentication is enabled, but cookie path '{}' does not exist.", cookie_path),
                    ));
                }
            } else {
                return Err(IndexerError::ConfigError(
                    "Validator cookie authentication is enabled, but no cookie path is provided."
                        .to_string(),
                ));
            }
        }

        // Ensure TLS is used when connecting to external addresses.
        if !is_loopback_listen_addr(&self.grpc_listen_address) && !self.grpc_tls {
            return Err(IndexerError::ConfigError(
                "TLS required when connecting to external addresses.".to_string(),
            ));
        }

        // Ensure validator listen address is private.
        if !is_private_listen_addr(&self.validator_listen_address) {
            return Err(IndexerError::ConfigError(
                "Zaino may only connect to Zebra with private IP addresses.".to_string(),
            ));
        }

        // Ensure validator rpc cookie authentication is used when connecting to non-loopback addresses.
        if !is_loopback_listen_addr(&self.validator_listen_address) && !self.validator_cookie_auth {
            return Err(IndexerError::ConfigError(
                "Validator listen address is not loopback, so cookie authentication must be enabled."
                    .to_string(),
            ));
        }

        Ok(())
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
            grpc_tls: false,
            tls_cert_path: None,
            tls_key_path: None,
            validator_listen_address: "127.0.0.1:18232".parse().unwrap(),
            validator_cookie_auth: false,
            validator_cookie_path: None,
            validator_user: Some("xxxxxx".to_string()),
            validator_password: Some("xxxxxx".to_string()),
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

/// Resolves a hostname to a SocketAddr.
fn fetch_socket_addr_from_hostname(address: &str) -> Result<SocketAddr, IndexerError> {
    address.parse::<SocketAddr>().or_else(|_| {
        address
            .to_socket_addrs()
            .map_err(|e| {
                IndexerError::ConfigError(format!("Invalid address '{}': {}", address, e))
            })?
            .find(|addr| addr.is_ipv4() || addr.is_ipv6())
            .ok_or_else(|| {
                IndexerError::ConfigError(format!("Unable to resolve address '{}'", address))
            })
    })
}

/// Validates that the configured `address` is either:
/// - An RFC1918 (private) IPv4 address, or
/// - An IPv6 Unique Local Address (ULA) (using `is_unique_local()`)
///
/// Returns `Ok(BindAddress)` if valid.
pub(crate) fn is_private_listen_addr(addr: &SocketAddr) -> bool {
    let ip = addr.ip();
    match ip {
        IpAddr::V4(ipv4) => {
            if ipv4.is_private() || ipv4.is_loopback() {
                true
            } else {
                false
            }
        }
        IpAddr::V6(ipv6) => {
            if ipv6.is_unique_local() || ip.is_loopback() {
                true
            } else {
                false
            }
        }
    }
}

/// Validates that the configured `address` is a loopback address.
///
/// Returns `Ok(BindAddress)` if valid.
pub(crate) fn is_loopback_listen_addr(addr: &SocketAddr) -> bool {
    let ip = addr.ip();
    match ip {
        IpAddr::V4(ipv4) => {
            if ipv4.is_loopback() {
                true
            } else {
                false
            }
        }
        IpAddr::V6(ipv6) => {
            if ipv6.is_loopback() {
                true
            } else {
                false
            }
        }
    }
}

/// Attempts to load config data from a toml file at the specified path else returns a default config.
///
/// Loads each variable individually to log all default values used and correctly parse hostnames.
pub fn load_config(file_path: &std::path::PathBuf) -> Result<IndexerConfig, IndexerError> {
    let default_config = IndexerConfig::default();

    if let Ok(contents) = std::fs::read_to_string(file_path) {
        let parsed_config: toml::Value = toml::from_str(&contents)
            .map_err(|e| IndexerError::ConfigError(format!("TOML parsing error: {}", e)))?;

        let grpc_listen_address = parsed_config
            .get("grpc_listen_address")
            .and_then(|v| v.as_str())
            .map(|addr| {
                fetch_socket_addr_from_hostname(addr).unwrap_or_else(|_| {
                    warn!("Invalid `grpc_listen_address`, using default.");
                    default_config.grpc_listen_address
                })
            })
            .unwrap_or_else(|| {
                warn!("Missing `grpc_listen_address`, using default.");
                default_config.grpc_listen_address
            });

        let tls = parsed_config
            .get("tls")
            .and_then(|v| v.as_bool())
            .unwrap_or_else(|| {
                warn!("Missing `tls`, using default.");
                default_config.grpc_tls
            });

        let tls_cert_path = parsed_config
            .get("tls_cert_path")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .or_else(|| {
                warn!("Missing `tls_cert_path`, using default.");
                default_config.tls_cert_path.clone()
            });

        let tls_key_path = parsed_config
            .get("tls_key_path")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .or_else(|| {
                warn!("Missing `tls_key_path`, using default.");
                default_config.tls_key_path.clone()
            });

        let validator_listen_address = parsed_config
            .get("validator_listen_address")
            .and_then(|v| v.as_str())
            .map(|addr| {
                fetch_socket_addr_from_hostname(addr).unwrap_or_else(|_| {
                    warn!("Invalid `grpc_listen_address`, using default.");
                    default_config.validator_listen_address
                })
            })
            .unwrap_or_else(|| {
                warn!("Missing `grpc_listen_address`, using default.");
                default_config.grpc_listen_address
            });

        let validator_cookie_auth = parsed_config
            .get("validator_cookie_auth")
            .and_then(|v| v.as_bool())
            .unwrap_or_else(|| {
                warn!("Missing `validator_cookie_auth`, using default.");
                default_config.validator_cookie_auth
            });

        let validator_cookie_path = parsed_config
            .get("validator_cookie_path")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .or_else(|| {
                warn!("Missing `validator_cookie_path`, using default.");
                default_config.validator_cookie_path.clone()
            });

        let node_user = parsed_config
            .get("node_user")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .or_else(|| {
                warn!("Missing `node_user`, using default.");
                default_config.validator_user.clone()
            });

        let node_password = parsed_config
            .get("node_password")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .or_else(|| {
                warn!("Missing `node_password`, using default.");
                default_config.validator_password.clone()
            });

        let map_capacity = parsed_config
            .get("map_capacity")
            .and_then(|v| v.as_integer().map(|n| n as usize))
            .or_else(|| {
                warn!("Missing `map_capacity`, using default.");
                default_config.map_capacity
            });

        let map_shard_amount = parsed_config
            .get("map_shard_amount")
            .and_then(|v| v.as_integer().map(|n| n as usize))
            .or_else(|| {
                warn!("Missing `map_shard_amount`, using default.");
                default_config.map_shard_amount
            });

        let db_path = parsed_config
            .get("db_path")
            .and_then(|v| v.as_str().map(PathBuf::from))
            .unwrap_or_else(|| {
                warn!("Missing `db_path`, using default.");
                default_config.db_path.clone()
            });

        let db_size = parsed_config
            .get("db_size")
            .and_then(|v| v.as_integer().map(|n| n as usize))
            .or_else(|| {
                warn!("Missing `db_size`, using default.");
                default_config.db_size
            });

        let network = parsed_config
            .get("network")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| {
                warn!("Missing `network`, using default.");
                default_config.network.clone()
            });

        let no_sync = parsed_config
            .get("no_sync")
            .and_then(|v| v.as_bool())
            .unwrap_or_else(|| {
                warn!("Missing `no_sync`, using default.");
                default_config.no_sync
            });

        let no_db = parsed_config
            .get("no_db")
            .and_then(|v| v.as_bool())
            .unwrap_or_else(|| {
                warn!("Missing `no_db`, using default.");
                default_config.no_db
            });

        let no_state = parsed_config
            .get("no_state")
            .and_then(|v| v.as_bool())
            .unwrap_or_else(|| {
                warn!("Missing `no_state`, using default.");
                default_config.no_state
            });

        let config = IndexerConfig {
            grpc_listen_address,
            grpc_tls: tls,
            tls_cert_path,
            tls_key_path,
            validator_listen_address,
            validator_cookie_auth,
            validator_cookie_path,
            validator_user: node_user,
            validator_password: node_password,
            map_capacity,
            map_shard_amount,
            db_path,
            db_size,
            network,
            no_sync,
            no_db,
            no_state,
        };

        config.check_config()?;
        Ok(config)
    } else {
        warn!("Could not find config file at given path, using default config.");
        Ok(default_config)
    }
}
