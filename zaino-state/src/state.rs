//! Holds Wrapper functionality for Zebra's `ReadStateService`.

use chrono::Utc;
use indexmap::IndexMap;
use std::future::poll_fn;
use std::net::SocketAddr;
use tower::Service;

use zebra_chain::{
    chain_tip::NetworkChainTipHeightEstimator,
    parameters::{ConsensusBranchId, Network, NetworkUpgrade},
};
use zebra_rpc::{
    methods::{
        types::ValuePoolBalance, ConsensusBranchIdHex, GetBlockChainInfo, GetInfo,
        NetworkUpgradeInfo, NetworkUpgradeStatus, TipConsensusBranch,
    },
    sync::init_read_state_with_syncer,
};
use zebra_state::{ChainTipChange, LatestChainTip, ReadStateService};

use crate::{
    error::StateServiceError,
    get_build_info,
    status::{AtomicStatus, StatusType},
    ServiceMetadata,
};

/// Chain fetch service backed by Zebra's `ReadStateService` and `TrustedChainSync`.
#[derive(Debug)]
pub struct StateService {
    /// `ReadeStateService` from Zebra-State.
    read_state_service: ReadStateService,
    /// Tracks the latest chain tip.
    latest_chain_tip: LatestChainTip,
    /// Monitors changes in the chain tip.
    chain_tip_change: ChainTipChange,
    /// Sync task handle.
    sync_task_handle: tokio::task::JoinHandle<()>,
    /// Service metadata.
    data: ServiceMetadata,
    /// Thread-safe status indicator
    status: AtomicStatus,
}

impl StateService {
    /// Initializes a new StateService instance and starts sync process.
    pub async fn spawn(
        config: zebra_state::Config,
        network: &Network,
        rpc_address: SocketAddr,
    ) -> Result<Self, StateServiceError> {
        let (read_state_service, latest_chain_tip, chain_tip_change, sync_task_handle) =
            init_read_state_with_syncer(config, network, rpc_address).await??;

        let data = ServiceMetadata {
            build_info: get_build_info(),
            network: network.clone(),
        };

        let mut state_service = Self {
            read_state_service,
            latest_chain_tip,
            chain_tip_change,
            sync_task_handle,
            data,
            status: AtomicStatus::new(StatusType::Spawning.into()),
        };

        state_service.status.store(StatusType::Syncing.into());

        poll_fn(|cx| state_service.read_state_service.poll_ready(cx)).await?;

        state_service.status.store(StatusType::Ready.into());

        Ok(state_service)
    }

    /// A combined function that checks readiness using `poll_ready` and then performs the request.
    /// If the service is busy, it waits until ready. If there's an error, it returns the error.
    pub(crate) async fn checked_call(
        &mut self,
        req: zebra_state::ReadRequest,
    ) -> Result<zebra_state::ReadResponse, StateServiceError> {
        poll_fn(|cx| self.read_state_service.poll_ready(cx)).await?;
        self.read_state_service
            .call(req)
            .await
            .map_err(StateServiceError::from)
    }

    /// Uses poll_ready to update the status of the `ReadStateService`.
    pub(crate) async fn fetch_status_from_validator(&mut self) -> StatusType {
        poll_fn(|cx| match self.read_state_service.poll_ready(cx) {
            std::task::Poll::Ready(Ok(())) => {
                self.status.store(StatusType::Ready.into());
                std::task::Poll::Ready(StatusType::Ready)
            }
            std::task::Poll::Ready(Err(e)) => {
                eprintln!("Service readiness error: {:?}", e);
                self.status.store(StatusType::CriticalError.into());
                std::task::Poll::Ready(StatusType::CriticalError)
            }
            std::task::Poll::Pending => {
                self.status.store(StatusType::Busy.into());
                std::task::Poll::Pending
            }
        })
        .await
    }

    /// Fetches the current status
    pub fn status(&self) -> StatusType {
        self.status.load().into()
    }

    /// Shuts down the StateService.
    pub fn close(&mut self) {
        self.sync_task_handle.abort();
    }
}

impl Drop for StateService {
    fn drop(&mut self) {
        self.close()
    }
}

/// Zcash RPC method implementations.
///
/// Doc comments taken from Zebra for consistency.
///
/// TODO: Update this to be `impl Indexer for StateService` once rpc methods are implemented and tested (or implement separately).
impl StateService {
    /// Returns software information from the RPC server, as a [`GetInfo`] JSON struct.
    ///
    /// zcashd reference: [`getinfo`](https://zcash.github.io/rpc/getinfo.html)
    /// method: post
    /// tags: control
    ///
    /// # Notes
    ///
    /// [The zcashd reference](https://zcash.github.io/rpc/getinfo.html) might not show some fields
    /// in Zebra's [`GetInfo`]. Zebra uses the field names and formats from the
    /// [zcashd code](https://github.com/zcash/zcash/blob/v4.6.0-1/src/rpc/misc.cpp#L86-L87).
    ///
    /// Some fields from the zcashd reference are missing from Zebra's [`GetInfo`]. It only contains the fields
    /// [required for lightwalletd support.](https://github.com/zcash/lightwalletd/blob/v0.4.9/common/common.go#L91-L95)
    pub async fn get_info(&self) -> Result<GetInfo, StateServiceError> {
        Ok(GetInfo::from_parts(
            self.data.build_info().version(),
            self.data.build_info().build_user(),
        ))
    }

    /// Returns blockchain state information, as a [`GetBlockChainInfo`] JSON struct.
    ///
    /// zcashd reference: [`getblockchaininfo`](https://zcash.github.io/rpc/getblockchaininfo.html)
    /// method: post
    /// tags: blockchain
    ///
    /// # Notes
    ///
    /// Some fields from the zcashd reference are missing from Zebra's [`GetBlockChainInfo`]. It only contains the fields
    /// [required for lightwalletd support.](https://github.com/zcash/lightwalletd/blob/v0.4.9/common/common.go#L72-L89)
    pub async fn get_blockchain_info(&mut self) -> Result<GetBlockChainInfo, StateServiceError> {
        let network = self.data.network();
        let chain = network.bip70_network_name();

        // Fetch Pool Values
        let pool_values = self
            .checked_call(zebra_state::ReadRequest::TipPoolValues)
            .await?;
        let zebra_state::ReadResponse::TipPoolValues {
            tip_height,
            tip_hash,
            value_balance,
        } = pool_values
        else {
            return Err(StateServiceError::Custom(
                "Unexpected response type for TipPoolValues".into(),
            ));
        };

        // Calculate Estimated height
        let block_header = self
            .checked_call(zebra_state::ReadRequest::BlockHeader(tip_hash.into()))
            .await?;
        let zebra_state::ReadResponse::BlockHeader { header, .. } = block_header else {
            return Err(StateServiceError::Custom(
                "Unexpected response type for BlockHeader".into(),
            ));
        };
        let tip_block_time = header.time;
        let now = Utc::now();
        let zebra_estimated_height =
            NetworkChainTipHeightEstimator::new(tip_block_time, tip_height, &network)
                .estimate_height_at(now);
        let estimated_height = if tip_block_time > now || zebra_estimated_height < tip_height {
            tip_height
        } else {
            zebra_estimated_height
        };

        // Create `upgrades` object
        //
        // Get the network upgrades in height order, like `zebra` `zcashd`.
        let mut upgrades = IndexMap::new();
        for (activation_height, network_upgrade) in network.full_activation_list() {
            // Zebra defines network upgrades based on incompatible consensus rule changes,
            // but zcashd defines them based on ZIPs.
            //
            // All the network upgrades with a consensus branch ID are the same in Zebra and zcashd.
            if let Some(branch_id) = network_upgrade.branch_id() {
                // zcashd's RPC seems to ignore Disabled network upgrades, so Zaino does too.
                let status = if tip_height >= activation_height {
                    NetworkUpgradeStatus::Active
                } else {
                    NetworkUpgradeStatus::Pending
                };

                let upgrade =
                    NetworkUpgradeInfo::from_parts(network_upgrade, activation_height, status);
                upgrades.insert(ConsensusBranchIdHex::new(branch_id.into()), upgrade);
            }
        }

        // Create `consensus` object
        let next_block_height =
            (tip_height + 1).expect("valid chain tips are a lot less than Height::MAX");
        let consensus = TipConsensusBranch::from_parts(
            NetworkUpgrade::current(&network, tip_height)
                .branch_id()
                .unwrap_or(ConsensusBranchId::RPC_MISSING_ID)
                .into(),
            NetworkUpgrade::current(&network, next_block_height)
                .branch_id()
                .unwrap_or(ConsensusBranchId::RPC_MISSING_ID)
                .into(),
        );

        let response = GetBlockChainInfo::new(
            chain,
            tip_height,
            tip_hash,
            estimated_height,
            ValuePoolBalance::from_value_balance(value_balance),
            upgrades,
            consensus,
        );

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zaino_testutils::{TestManager, ZEBRAD_CHAIN_CACHE_BIN};

    #[tokio::test]
    async fn launch_state_service_no_cache() {
        let mut test_manager = TestManager::launch("zebrad", None, false, false)
            .await
            .unwrap();

        let config = zebra_state::Config {
            cache_dir: test_manager.data_dir.clone(),
            ephemeral: false,
            delete_old_database: true,
            debug_stop_at_height: None,
            debug_validity_check_interval: None,
        };

        let mut state_service = StateService::spawn(
            config,
            &Network::new_regtest(Some(1), Some(1)),
            SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                test_manager.zebrad_rpc_listen_port,
            ),
        )
        .await
        .unwrap();

        assert_eq!(
            state_service.fetch_status_from_validator().await,
            StatusType::Ready
        );

        test_manager.close().await;
    }

    #[tokio::test]
    async fn launch_state_service_with_cache() {
        let mut test_manager =
            TestManager::launch("zebrad", ZEBRAD_CHAIN_CACHE_BIN.clone(), false, false)
                .await
                .unwrap();

        let config = zebra_state::Config {
            cache_dir: test_manager.data_dir.clone(),
            ephemeral: false,
            delete_old_database: true,
            debug_stop_at_height: None,
            debug_validity_check_interval: None,
        };

        let mut state_service = StateService::spawn(
            config,
            &Network::new_regtest(Some(1), Some(1)),
            SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                test_manager.zebrad_rpc_listen_port,
            ),
        )
        .await
        .unwrap();

        assert_eq!(
            state_service.fetch_status_from_validator().await,
            StatusType::Ready
        );

        test_manager.close().await;
    }

    #[tokio::test]
    async fn state_service_get_info() {
        let mut test_manager =
            TestManager::launch("zebrad", ZEBRAD_CHAIN_CACHE_BIN.clone(), false, false)
                .await
                .unwrap();

        let config = zebra_state::Config {
            cache_dir: test_manager.data_dir.clone(),
            ephemeral: false,
            delete_old_database: true,
            debug_stop_at_height: None,
            debug_validity_check_interval: None,
        };

        let mut state_service = StateService::spawn(
            config,
            &Network::new_regtest(Some(1), Some(1)),
            SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                test_manager.zebrad_rpc_listen_port,
            ),
        )
        .await
        .unwrap();

        dbg!(state_service.get_info().await.unwrap());

        test_manager.close().await;
    }

    #[tokio::test]
    async fn state_service_get_blockchain_info() {
        let mut test_manager =
            TestManager::launch("zebrad", ZEBRAD_CHAIN_CACHE_BIN.clone(), false, false)
                .await
                .unwrap();

        let mut state_service = StateService::spawn(
            zebra_state::Config {
                cache_dir: test_manager.data_dir.clone(),
                ephemeral: false,
                delete_old_database: true,
                debug_stop_at_height: None,
                debug_validity_check_interval: None,
            },
            &Network::new_regtest(Some(1), Some(1)),
            SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                test_manager.zebrad_rpc_listen_port,
            ),
        )
        .await
        .unwrap();
        let fetch_service = zaino_fetch::jsonrpc::connector::JsonRpcConnector::new(
            url::Url::parse(&format!(
                "http://127.0.0.1:{}",
                test_manager.zebrad_rpc_listen_port
            ))
            .expect("Failed to construct URL")
            .as_str()
            .try_into()
            .expect("Failed to convert URL to URI"),
            Some("xxxxxx".to_string()),
            Some("xxxxxx".to_string()),
        )
        .await
        .unwrap();

        let state_start = tokio::time::Instant::now();
        let state_service_get_blockchain_info = state_service.get_blockchain_info().await.unwrap();
        let state_service_duration = state_start.elapsed();

        let fetch_start = tokio::time::Instant::now();
        let fetch_service_get_blockchain_info = fetch_service.get_blockchain_info().await.unwrap();
        let fetch_service_duration = fetch_start.elapsed();

        assert_eq!(
            state_service_get_blockchain_info,
            fetch_service_get_blockchain_info.into()
        );

        println!("GetBlockChainInfo responses correct. State-Service processing time: {:?} - fetch-Service processing time: {:?}.", state_service_duration, fetch_service_duration);

        test_manager.close().await;
    }
}
