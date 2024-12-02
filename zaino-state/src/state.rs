//! Holds Wrapper functionality for Zebra's `ReadStateService`.

use std::net::SocketAddr;

use zebra_chain::parameters::Network;
use zebra_rpc::sync::init_read_state_with_syncer;
use zebra_state::{ChainTipChange, LatestChainTip, ReadStateService};

use crate::{
    error::StateServiceError,
    status::{AtomicStatus, StatusType},
};

/// Chain fetch service backed by Zebra's `ReadStateService` and `TrustedChainSync`.
pub struct StateService {
    /// `ReadeStateService` from Zebra-State.
    read_state_service: ReadStateService,
    /// Tracks the latest chain tip
    latest_chain_tip: LatestChainTip,
    /// Monitors changes in the chain tip
    chain_tip_change: ChainTipChange,
    /// Sync task handle
    sync_task_handle: tokio::task::JoinHandle<()>,
    /// Thread-safe status indicator
    status: AtomicStatus,
}

impl StateService {
    /// Initializes a new StateService instance
    pub async fn new(
        config: zebra_state::Config,
        network: &Network,
        rpc_address: SocketAddr,
    ) -> Result<Self, StateServiceError> {
        let (read_state_service, latest_chain_tip, chain_tip_change, sync_task) =
            init_read_state_with_syncer(config, network, rpc_address).await??;

        Ok(Self {
            read_state_service,
            latest_chain_tip,
            chain_tip_change,
            sync_task_handle: sync_task,
            status: AtomicStatus::new(StatusType::Spawning.into()),
        })
    }

    // /// Starts monitoring chain tip synchronization
    // pub async fn monitor_sync(&self) {
    //     while let Ok(tip_action) = self.chain_tip_change.next_tip().await {
    //         match tip_action {
    //             zebra_state::TipAction::Grow(_) | zebra_state::TipAction::Reset(_) => {
    //                 self.status.store(StatusType::Ready as usize);
    //             }
    //             _ => {
    //                 self.status.store(StatusType::CriticalError as usize);
    //             }
    //         }
    //     }
    // }

    // /// Fetches the current status
    // pub fn status(&self) -> StatusType {
    //     self.status.load().into()
    // }

    // /// Fetch a block by height
    // pub async fn get_block(
    //     &self,
    //     height: zebra_chain::block::Height,
    // ) -> Option<zebra_chain::block::Block> {
    //     self.read_state_service.block(height).await.ok()
    // }
}
