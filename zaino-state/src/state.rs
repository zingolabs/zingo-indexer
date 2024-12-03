//! Holds Wrapper functionality for Zebra's `ReadStateService`.

use std::future::poll_fn;
use std::net::SocketAddr;
use tower::Service;

use zebra_chain::parameters::Network;
use zebra_rpc::{
    methods::{GetBlockChainInfo, GetInfo},
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
}

/// Zcash RPC method implementations.
///
/// Doc comments taken from Zebra for consistency.
///
/// TODO: Update this to be `impl Indexer for StateService` once rpc methods are implemented and tested.
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
        // let response = GetInfo {
        //     build: self.data.version(),
        //     subversion: self.data.user(),
        // };

        // Ok(response)
        todo!()
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
    pub async fn get_blockchain_info(&self) -> Result<GetBlockChainInfo, StateServiceError> {
        todo!()
    }
}
