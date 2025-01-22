//! Zaino config.

use std::path::PathBuf;

use crate::error::IndexerError;

/// Config information required for Zaino.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct IndexerConfig {
    /// TcpIngestors listen port
    pub listen_port: u16,
    /// Full node / validator listen port.
    pub zebrad_port: u16,
    /// Full node / validator Username.
    pub node_user: Option<String>,
    /// full node / validator Password.
    pub node_password: Option<String>,
    /// Maximum requests allowed in the request queue.
    pub max_queue_size: u16,
    /// Maximum workers allowed in the worker pool
    pub max_worker_pool_size: u16,
    /// Minimum number of workers held in the workerpool when idle.
    pub idle_worker_pool_size: u16,
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
    /// - Checks listen port is given is tcp is active.
    pub(crate) fn check_config(&self) -> Result<(), IndexerError> {
        if (self.network != "Regtest") && (self.network != "Testnet") && (self.network != "Mainnet")
        {
            return Err(IndexerError::ConfigError(
                "Incorrect network name given.".to_string(),
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
            listen_port: 8137,
            zebrad_port: 18232,
            node_user: Some("xxxxxx".to_string()),
            node_password: Some("xxxxxx".to_string()),
            max_queue_size: 1024,
            max_worker_pool_size: 32,
            idle_worker_pool_size: 4,
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
        eprintln!("Could not find config file at given path, using default config.");
        IndexerConfig::default()
    }
}
