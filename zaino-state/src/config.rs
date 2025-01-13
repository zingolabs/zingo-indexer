//! Holds config data for Zaino-State services.

/// Holds config data for [`StateService`].
#[derive(Debug, Clone)]
pub struct StateServiceConfig {
    /// Zebra [`ReadStateService`] config data
    pub validator_config: zebra_state::Config,
    /// Validator JsonRPC address.
    pub validator_rpc_address: std::net::SocketAddr,
    /// Validator JsonRPC user.
    pub validator_rpc_user: String,
    /// Validator JsonRPC password.
    pub validator_rpc_password: String,
    /// StateService RPC timeout
    pub service_timeout: u32,
    /// StateService RPC max channel size.
    pub service_channel_size: u32,
    /// StateService network type.
    pub network: zebra_chain::parameters::Network,
}

impl StateServiceConfig {
    /// Returns a new instance of [`StateServiceConfig`].
    pub fn new(
        validator_config: zebra_state::Config,
        validator_rpc_address: std::net::SocketAddr,
        validator_rpc_user: Option<String>,
        validator_rpc_password: Option<String>,
        service_timeout: Option<u32>,
        service_channel_size: Option<u32>,
        network: zebra_chain::parameters::Network,
    ) -> Self {
        StateServiceConfig {
            validator_config,
            validator_rpc_address,
            validator_rpc_user: validator_rpc_user.unwrap_or("xxxxxx".to_string()),
            validator_rpc_password: validator_rpc_password.unwrap_or("xxxxxx".to_string()),
            service_timeout: service_timeout.unwrap_or(30),
            service_channel_size: service_channel_size.unwrap_or(32),
            network,
        }
    }
}

/// Holds config data for [`FetchService`].
#[derive(Debug, Clone)]
pub struct FetchServiceConfig {
    /// Validator JsonRPC address.
    pub validator_rpc_address: std::net::SocketAddr,
    /// Validator JsonRPC user.
    pub validator_rpc_user: String,
    /// Validator JsonRPC password.
    pub validator_rpc_password: String,
    /// StateService RPC timeout
    pub service_timeout: u32,
    /// StateService RPC max channel size.
    pub service_channel_size: u32,
    /// StateService network type.
    pub network: zebra_chain::parameters::Network,
}

impl FetchServiceConfig {
    /// Returns a new instance of [`FetchServiceConfig`].
    pub fn new(
        validator_rpc_address: std::net::SocketAddr,
        validator_rpc_user: Option<String>,
        validator_rpc_password: Option<String>,
        service_timeout: Option<u32>,
        service_channel_size: Option<u32>,
        network: zebra_chain::parameters::Network,
    ) -> Self {
        FetchServiceConfig {
            validator_rpc_address,
            validator_rpc_user: validator_rpc_user.unwrap_or("xxxxxx".to_string()),
            validator_rpc_password: validator_rpc_password.unwrap_or("xxxxxx".to_string()),
            // NOTE: This timeout is currently long to ease development but should be reduced before production.
            service_timeout: service_timeout.unwrap_or(60),
            service_channel_size: service_channel_size.unwrap_or(32),
            network,
        }
    }
}
