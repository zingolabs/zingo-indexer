//! Zaino config.

use crate::error::IndexerError;

/// Config information required for Zaino.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct IndexerConfig {
    /// Sets the TcpIngestor's status.
    pub tcp_active: bool,
    /// TcpIngestors listen port
    pub listen_port: Option<u16>,
    /// LightWalletD listen port [DEPRECATED].
    /// Used by zingo-testutils.
    pub lightwalletd_port: u16,
    /// Full node / validator listen port.
    pub zebrad_port: u16,
    /// Full node Username.
    pub node_user: Option<String>,
    /// full node Password.
    pub node_password: Option<String>,
    /// Maximum requests allowed in the request queue.
    pub max_queue_size: u16,
    /// Maximum workers allowed in the worker pool
    pub max_worker_pool_size: u16,
    /// Minimum number of workers held in the workerpool when idle.
    pub idle_worker_pool_size: u16,
}

impl IndexerConfig {
    /// Performs checks on config data.
    ///
    /// - Checks that at least 1 ingestor is active.
    /// - Checks listen port is given is tcp is active.
    pub(crate) fn check_config(&self) -> Result<(), IndexerError> {
        if !self.tcp_active {
            return Err(IndexerError::ConfigError(
                "Cannot start server with no ingestors selected.".to_string(),
            ));
        }
        if self.tcp_active && self.listen_port.is_none() {
            return Err(IndexerError::ConfigError(
                "TCP is active but no address provided.".to_string(),
            ));
        }
        Ok(())
    }
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            tcp_active: true,
            listen_port: Some(8080),
            lightwalletd_port: 9067,
            zebrad_port: 18232,
            node_user: Some("xxxxxx".to_string()),
            node_password: Some("xxxxxx".to_string()),
            max_queue_size: 1024,
            max_worker_pool_size: 32,
            idle_worker_pool_size: 4,
        }
    }
}

/// Attempts to load config data from a toml file at the specified path.
pub fn load_config(file_path: &std::path::PathBuf) -> IndexerConfig {
    let mut config = IndexerConfig::default();

    if let Ok(contents) = std::fs::read_to_string(file_path) {
        if let Ok(parsed_config) = toml::from_str::<IndexerConfig>(&contents) {
            config = IndexerConfig {
                tcp_active: parsed_config.tcp_active,
                listen_port: parsed_config.listen_port.or(config.listen_port),
                lightwalletd_port: parsed_config.lightwalletd_port,
                zebrad_port: parsed_config.zebrad_port,
                node_user: parsed_config.node_user.or(config.node_user),
                node_password: parsed_config.node_password.or(config.node_password),
                max_queue_size: parsed_config.max_queue_size,
                max_worker_pool_size: parsed_config.max_worker_pool_size,
                idle_worker_pool_size: parsed_config.idle_worker_pool_size,
            };
        }
    }

    config
}
