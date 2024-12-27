//! Holds Wrapper functionality for Zebra's `ReadStateService`.

use chrono::Utc;
use hex::ToHex;
use indexmap::IndexMap;
use std::io::Cursor;
use std::{future::poll_fn, pin::pin};
use tokio::time::timeout;
use tower::Service;
use zaino_proto::proto::service::BlockRange;
use zebra_chain::parameters::Network;

use zebra_chain::{
    chain_tip::{ChainTip, NetworkChainTipHeightEstimator},
    parameters::{ConsensusBranchId, NetworkUpgrade},
    serialization::{ZcashDeserialize, ZcashSerialize},
    transaction::Transaction,
};
use zebra_rpc::{
    constants::{INVALID_ADDRESS_OR_KEY_ERROR_CODE, MISSING_BLOCK_ERROR_CODE},
    methods::{
        hex_data::HexData, types::ValuePoolBalance, ConsensusBranchIdHex, GetBlock,
        GetBlockChainInfo, GetBlockHash, GetBlockHeader, GetBlockHeaderObject, GetBlockTrees,
        GetInfo, NetworkUpgradeInfo, NetworkUpgradeStatus, TipConsensusBranch,
    },
    sync::init_read_state_with_syncer,
};
use zebra_state::{ChainTipChange, HashOrHeight, LatestChainTip, ReadStateService};

use crate::{
    config::StateServiceConfig,
    error::StateServiceError,
    get_build_info,
    status::{AtomicStatus, StatusType},
    stream::CompactBlockStream,
    ServiceMetadata,
};
use zaino_fetch::jsonrpc::connector::{test_node_and_return_uri, JsonRpcConnector};
use zaino_proto::proto::compact_formats::{
    ChainMetadata, CompactBlock, CompactOrchardAction, CompactSaplingOutput, CompactSaplingSpend,
    CompactTx,
};

/// Chain fetch service backed by Zebra's `ReadStateService` and `TrustedChainSync`.
#[derive(Debug)]
pub struct StateService {
    /// `ReadeStateService` from Zebra-State.
    read_state_service: ReadStateService,
    /// Tracks the latest chain tip.
    latest_chain_tip: LatestChainTip,
    /// Monitors changes in the chain tip.
    _chain_tip_change: ChainTipChange,
    /// Sync task handle.
    sync_task_handle: tokio::task::JoinHandle<()>,
    /// JsonRPC Client.
    _rpc_client: JsonRpcConnector,
    /// Service metadata.
    data: ServiceMetadata,
    /// StateService config data.
    config: StateServiceConfig,
    /// Thread-safe status indicator.
    status: AtomicStatus,
}

impl StateService {
    /// Initializes a new StateService instance and starts sync process.
    pub async fn spawn(config: StateServiceConfig) -> Result<Self, StateServiceError> {
        let rpc_uri = test_node_and_return_uri(
            &config.validator_rpc_address.port(),
            Some(config.validator_rpc_user.clone()),
            Some(config.validator_rpc_password.clone()),
        )
        .await?;

        let (read_state_service, latest_chain_tip, chain_tip_change, sync_task_handle) =
            init_read_state_with_syncer(
                config.validator_config.clone(),
                &config.network,
                config.validator_rpc_address,
            )
            .await??;

        let rpc_client = JsonRpcConnector::new(
            rpc_uri,
            Some(config.validator_rpc_user.clone()),
            Some(config.validator_rpc_password.clone()),
        )
        .await
        .unwrap();

        let zebra_build_data = rpc_client.get_info().await.unwrap();

        let data = ServiceMetadata {
            build_info: get_build_info(),
            network: config.network.clone(),
            zebra_build: zebra_build_data.build,
            zebra_subversion: zebra_build_data.subversion,
        };

        let mut state_service = Self {
            read_state_service,
            latest_chain_tip,
            _chain_tip_change: chain_tip_change,
            sync_task_handle,
            _rpc_client: rpc_client,
            data,
            config,
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
        &self,
        req: zebra_state::ReadRequest,
    ) -> Result<zebra_state::ReadResponse, StateServiceError> {
        let mut read_state_service = self.read_state_service.clone();
        poll_fn(|cx| read_state_service.poll_ready(cx)).await?;
        read_state_service
            .call(req)
            .await
            .map_err(StateServiceError::from)
    }

    /// A combined function that checks readiness using `poll_ready` and then performs the request.
    /// If the service is busy, it waits until ready. If there's an error, it returns the error.
    ///
    /// Avoides taking `Self`.
    pub(crate) async fn checked_call_decoupled(
        mut read_state_service: ReadStateService,
        req: zebra_state::ReadRequest,
    ) -> Result<zebra_state::ReadResponse, StateServiceError> {
        // let mut read_state_service = self.read_state_service.clone();
        poll_fn(|cx| read_state_service.poll_ready(cx)).await?;
        read_state_service
            .call(req)
            .await
            .map_err(StateServiceError::from)
    }

    /// Uses poll_ready to update the status of the `ReadStateService`.
    #[allow(dead_code)]
    pub(crate) async fn fetch_status_from_validator(&self) -> StatusType {
        let mut read_state_service = self.read_state_service.clone();
        poll_fn(|cx| match read_state_service.poll_ready(cx) {
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

/// This impl will hold the Zcash RPC method implementations for StateService.
///
/// Doc comments are taken from Zebra for consistency.
///
/// TODO: Update this to be `impl ZcashIndexer for StateService` once rpc methods are implemented and tested (or implement separately).
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
            self.data.zebra_build(),
            self.data.zebra_subversion(),
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
    pub async fn get_blockchain_info(&self) -> Result<GetBlockChainInfo, StateServiceError> {
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

    /// Returns the requested block by hash or height, as a [`GetBlock`] JSON string.
    /// If the block is not in Zebra's state, returns
    /// [error code `-8`.](https://github.com/zcash/zcash/issues/5758) if a height was
    /// passed or -5 if a hash was passed.
    ///
    /// zcashd reference: [`getblock`](https://zcash.github.io/rpc/getblock.html)
    /// method: post
    /// tags: blockchain
    ///
    /// # Parameters
    ///
    /// - `hash_or_height`: (string, required, example="1") The hash or height for the block to be returned.
    /// - `verbosity`: (number, optional, default=1, example=1) 0 for hex encoded data, 1 for a json object, and 2 for json object with transaction data.
    ///
    /// # Notes
    ///
    /// Zebra previously partially supported verbosity=1 by returning only the
    /// fields required by lightwalletd ([`lightwalletd` only reads the `tx`
    /// field of the result](https://github.com/zcash/lightwalletd/blob/dfac02093d85fb31fb9a8475b884dd6abca966c7/common/common.go#L152)).
    /// That verbosity level was migrated to "3"; so while lightwalletd will
    /// still work by using verbosity=1, it will sync faster if it is changed to
    /// use verbosity=3.
    ///
    /// The undocumented `chainwork` field is not returned.
    pub async fn get_block(
        &self,
        hash_or_height: String,
        verbosity: Option<u8>,
    ) -> Result<GetBlock, StateServiceError> {
        // From <https://zcash.github.io/rpc/getblock.html>
        const DEFAULT_GETBLOCK_VERBOSITY: u8 = 1;

        let verbosity = verbosity.unwrap_or(DEFAULT_GETBLOCK_VERBOSITY);
        let network = self.data.network.clone();
        let original_hash_or_height = hash_or_height.clone();

        // If verbosity requires a call to `get_block_header`, resolve it here
        let get_block_header_future = if matches!(verbosity, 1 | 2) {
            Some(self.get_block_header(original_hash_or_height.clone(), Some(true)))
        } else {
            None
        };

        let hash_or_height: HashOrHeight = hash_or_height.parse()?;

        if verbosity == 0 {
            // # Performance
            //
            // This RPC is used in `lightwalletd`'s initial sync of 2 million blocks,
            // so it needs to load block data very efficiently.
            match self
                .checked_call(zebra_state::ReadRequest::Block(hash_or_height))
                .await?
            {
                zebra_state::ReadResponse::Block(Some(block)) => Ok(GetBlock::Raw(block.into())),
                zebra_state::ReadResponse::Block(None) => Err(StateServiceError::RpcError(
                    zaino_fetch::jsonrpc::connector::RpcError {
                        code: MISSING_BLOCK_ERROR_CODE.code(),
                        message: "Block not found".to_string(),
                        data: None,
                    },
                )),
                _ => unreachable!("unmatched response to a block request"),
            }
        } else if let Some(get_block_header_future) = get_block_header_future {
            let GetBlockHeader::Object(block_header) = get_block_header_future.await? else {
                return Err(StateServiceError::Custom(
                    "Unexpected response type for BlockHeader".into(),
                ));
            };
            let GetBlockHeaderObject {
                hash,
                confirmations,
                height,
                version,
                merkle_root,
                final_sapling_root,
                sapling_tree_size,
                time,
                nonce,
                solution,
                bits,
                difficulty,
                previous_block_hash,
                next_block_hash,
            } = *block_header;

            // # Concurrency
            //
            // We look up by block hash so the hash, transaction IDs, and confirmations
            // are consistent.
            let hash_or_height = hash.0.into();

            let mut txids_future = pin!(self.checked_call(
                zebra_state::ReadRequest::TransactionIdsForBlock(hash_or_height)
            ));
            let mut orchard_tree_future =
                pin!(self.checked_call(zebra_state::ReadRequest::OrchardTree(hash_or_height)));

            let mut txids = None;
            let mut orchard_trees = None;
            let mut final_orchard_root = None;

            while txids.is_none() || orchard_trees.is_none() {
                tokio::select! {
                    response = &mut txids_future, if txids.is_none() => {
                        let tx_ids_response = response?;
                        let tx_ids = match tx_ids_response {
                            zebra_state::ReadResponse::TransactionIdsForBlock(Some(tx_ids)) => tx_ids,
                            zebra_state::ReadResponse::TransactionIdsForBlock(None) => {
                                return Err(StateServiceError::RpcError(zaino_fetch::jsonrpc::connector::RpcError {
                                    code: if hash_or_height.hash().is_some() {
                                        INVALID_ADDRESS_OR_KEY_ERROR_CODE.code()
                                    } else {
                                        MISSING_BLOCK_ERROR_CODE.code()
                                    },
                                    message: "Block not found".to_string(),
                                    data: None,
                                }));
                            }
                            _ => unreachable!("Unexpected response type for TransactionIdsForBlock"),
                        };

                        txids = Some(tx_ids.iter().map(|tx_id| tx_id.encode_hex()).collect::<Vec<String>>());
                    }
                    response = &mut orchard_tree_future, if orchard_trees.is_none() => {
                        let orchard_tree_response = response?;
                        let orchard_tree = match orchard_tree_response {
                            zebra_state::ReadResponse::OrchardTree(Some(tree)) => tree,
                            zebra_state::ReadResponse::OrchardTree(None) => {
                                return Err(StateServiceError::RpcError(zaino_fetch::jsonrpc::connector::RpcError {
                                    code: if hash_or_height.hash().is_some() {
                                        INVALID_ADDRESS_OR_KEY_ERROR_CODE.code()
                                    } else {
                                        MISSING_BLOCK_ERROR_CODE.code()
                                    },
                                    message: "Missing orchard tree for block.".to_string(),
                                    data: None,
                                }));
                            }
                            _ => unreachable!("Unexpected response type for OrchardTree"),
                        };

                        let orchard_tree_size = orchard_tree.count();
                        let nu5_activation = NetworkUpgrade::Nu5.activation_height(&network);


                        // ---

                        final_orchard_root = match nu5_activation {
                            Some(activation_height) if height >= activation_height => {
                                Some(orchard_tree.root().into())
                            }
                            _ => None,
                        };

                        orchard_trees = Some(GetBlockTrees::new(sapling_tree_size, orchard_tree_size));
                    }
                }
            }
            let tx = txids.unwrap();
            let trees = orchard_trees.unwrap();

            Ok(GetBlock::Object {
                hash,
                confirmations,
                height: Some(height),
                version: Some(version),
                merkle_root: Some(merkle_root),
                time: Some(time),
                nonce: Some(nonce),
                solution: Some(solution),
                bits: Some(bits),
                difficulty: Some(difficulty),
                tx,
                trees,
                size: None,
                final_sapling_root: Some(final_sapling_root),
                final_orchard_root,
                previous_block_hash: Some(previous_block_hash),
                next_block_hash,
            })
        } else {
            Err(StateServiceError::RpcError(
                zaino_fetch::jsonrpc::connector::RpcError {
                    code: jsonrpc_core::ErrorCode::InvalidParams.code(),
                    message: "Invalid verbosity value".to_string(),
                    data: None,
                },
            ))
        }
    }

    /// Returns the requested block header by hash or height, as a [`GetBlockHeader`] JSON string.
    /// If the block is not in Zebra's state,
    /// returns [error code `-8`.](https://github.com/zcash/zcash/issues/5758)
    /// if a height was passed or -5 if a hash was passed.
    ///
    /// zcashd reference: [`getblockheader`](https://zcash.github.io/rpc/getblockheader.html)
    /// method: post
    /// tags: blockchain
    ///
    /// # Parameters
    ///
    /// - `hash_or_height`: (string, required, example="1") The hash or height for the block to be returned.
    /// - `verbose`: (bool, optional, default=false, example=true) false for hex encoded data, true for a json object
    ///
    /// # Notes
    ///
    /// The undocumented `chainwork` field is not returned.
    ///
    /// This rpc is used by get_block(verbose), there is currently no plan to offer this RPC publicly.
    async fn get_block_header(
        &self,
        hash_or_height: String,
        verbose: Option<bool>,
    ) -> Result<GetBlockHeader, StateServiceError> {
        let verbose = verbose.unwrap_or(true);
        let network = self.data.network.clone();

        let hash_or_height: HashOrHeight = hash_or_height.parse()?;

        let zebra_state::ReadResponse::BlockHeader {
            header,
            hash,
            height,
            next_block_hash,
        } = self
            .checked_call(zebra_state::ReadRequest::BlockHeader(hash_or_height))
            .await
            .map_err(|_| {
                StateServiceError::RpcError(zaino_fetch::jsonrpc::connector::RpcError {
                    // Compatibility with zcashd. Note that since this function
                    // is reused by getblock(), we return the errors expected
                    // by it (they differ whether a hash or a height was passed)
                    code: if hash_or_height.hash().is_some() {
                        INVALID_ADDRESS_OR_KEY_ERROR_CODE.code()
                    } else {
                        MISSING_BLOCK_ERROR_CODE.code()
                    },
                    message: "block height not in best chain".to_string(),
                    data: None,
                })
            })?
        else {
            return Err(StateServiceError::Custom(
                "Unexpected response to BlockHeader request".to_string(),
            ));
        };

        let response = if !verbose {
            GetBlockHeader::Raw(HexData(header.zcash_serialize_to_vec().unwrap()))
        } else {
            let zebra_state::ReadResponse::SaplingTree(sapling_tree) = self
                .checked_call(zebra_state::ReadRequest::SaplingTree(hash_or_height))
                .await?
            else {
                return Err(StateServiceError::Custom(
                    "Unexpected response to SaplingTree request".to_string(),
                ));
            };

            // This could be `None` if there's a chain reorg between state queries.
            let sapling_tree = sapling_tree.ok_or_else(|| {
                StateServiceError::RpcError(zaino_fetch::jsonrpc::connector::RpcError {
                    code: MISSING_BLOCK_ERROR_CODE.code(),
                    message: "missing sapling tree for block".to_string(),
                    data: None,
                })
            })?;

            let zebra_state::ReadResponse::Depth(depth) = self
                .checked_call(zebra_state::ReadRequest::Depth(hash))
                .await?
            else {
                return Err(StateServiceError::Custom(
                    "Unexpected response to Depth request".to_string(),
                ));
            };

            // From <https://zcash.github.io/rpc/getblock.html>
            // TODO: Deduplicate const definition, consider refactoring this to avoid duplicate logic
            const NOT_IN_BEST_CHAIN_CONFIRMATIONS: i64 = -1;

            // Confirmations are one more than the depth.
            // Depth is limited by height, so it will never overflow an i64.
            let confirmations = depth
                .map(|depth| i64::from(depth) + 1)
                .unwrap_or(NOT_IN_BEST_CHAIN_CONFIRMATIONS);

            let mut nonce = *header.nonce;
            nonce.reverse();

            let sapling_activation = NetworkUpgrade::Sapling.activation_height(&network);
            let sapling_tree_size = sapling_tree.count();
            let final_sapling_root: [u8; 32] =
                if sapling_activation.is_some() && height >= sapling_activation.unwrap() {
                    let mut root: [u8; 32] = sapling_tree.root().into();
                    root.reverse();
                    root
                } else {
                    [0; 32]
                };

            let difficulty = header.difficulty_threshold.relative_to_network(&network);

            let block_header = GetBlockHeaderObject {
                hash: GetBlockHash(hash),
                confirmations,
                height,
                version: header.version,
                merkle_root: header.merkle_root,
                final_sapling_root,
                sapling_tree_size,
                time: header.time.timestamp(),
                nonce,
                solution: header.solution,
                bits: header.difficulty_threshold,
                difficulty,
                previous_block_hash: GetBlockHash(header.previous_block_hash),
                next_block_hash: next_block_hash.map(GetBlockHash),
            };

            GetBlockHeader::Object(Box::new(block_header))
        };

        Ok(response)
    }
}

/// This impl will hold the Lightwallet RPC method implementations for StateService.
///
/// TODO: Update this to be `impl LightWalletIndexer for StateService` once rpc methods are implemented and tested (or implement separately).
impl StateService {
    /// Return a list of consecutive compact blocks.
    pub async fn get_block_range(
        &self,
        blockrange: BlockRange,
    ) -> Result<CompactBlockStream, StateServiceError> {
        let mut start: u32 = match blockrange.start {
            Some(block_id) => match block_id.height.try_into() {
                Ok(height) => height,
                Err(_) => {
                    return Err(StateServiceError::TonicStatusError(
                        tonic::Status::invalid_argument(
                            "Error: Start height out of range. Failed to convert to u32.",
                        ),
                    ));
                }
            },
            None => {
                return Err(StateServiceError::TonicStatusError(
                    tonic::Status::invalid_argument("Error: No start height given."),
                ));
            }
        };
        let mut end: u32 = match blockrange.end {
            Some(block_id) => match block_id.height.try_into() {
                Ok(height) => height,
                Err(_) => {
                    return Err(StateServiceError::TonicStatusError(
                        tonic::Status::invalid_argument(
                            "Error: End height out of range. Failed to convert to u32.",
                        ),
                    ));
                }
            },
            None => {
                return Err(StateServiceError::TonicStatusError(
                    tonic::Status::invalid_argument("Error: No start height given."),
                ));
            }
        };
        let rev_order = if start > end {
            (start, end) = (end, start);
            true
        } else {
            false
        };

        let cloned_read_state_service = self.read_state_service.clone();
        let network = self.config.network.clone();
        let service_channel_size = self.config.service_channel_size;
        let service_timeout = self.config.service_timeout;
        let latest_chain_tip = self.latest_chain_tip.clone();
        let (channel_tx, channel_rx) = tokio::sync::mpsc::channel(service_channel_size as usize);
        tokio::spawn(async move {
            let timeout = timeout(
                std::time::Duration::from_secs(service_timeout as u64),
                async {
                    for height in start..=end {
                        let height = if rev_order {
                            end - (height - start)
                        } else {
                            height
                        };
                        match StateService::get_compact_block(
                            &cloned_read_state_service,
                            height.to_string(),
                            &network,
                        ).await {
                            Ok(block) => {
                                if channel_tx.send(Ok(block)).await.is_err() {
                                    break;
                                };
                            }
                            Err(e) => {
                                let chain_height = latest_chain_tip.best_tip_height().unwrap().0;
                                if height >= chain_height {
                                    match channel_tx
                                        .send(Err(tonic::Status::out_of_range(format!(
                                            "Error: Height out of range [{}]. Height requested is greater than the best chain tip [{}].",
                                            height, chain_height,
                                        ))))
                                        .await

                                    {
                                        Ok(_) => break,
                                        Err(e) => {
                                            eprintln!("Error: Channel closed unexpectedly: {}", e);
                                            break;
                                        }
                                    }
                                } else {
                                    // TODO: Hide server error from clients before release. Currently useful for dev purposes.
                                    if channel_tx
                                        .send(Err(tonic::Status::unknown(e.to_string())))
                                        .await
                                        .is_err()
                                    {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                },
            )
            .await;
            match timeout {
                Ok(_) => {}
                Err(_) => {
                    channel_tx
                        .send(Err(tonic::Status::deadline_exceeded(
                            "Error: get_block_range gRPC request timed out.",
                        )))
                        .await
                        .ok();
                }
            }
        });

        Ok(CompactBlockStream::new(channel_rx))
    }

    /// Returns a [`zaino_proto::proto::compact_formats::CompactTx`].
    ///
    /// Notes:
    ///
    /// Written to avoid taking [`Self`] to simplify use in [`get_block_range`].
    ///
    /// This function is used by get_block_range, there is currently no plan to offer this RPC publicly.
    ///
    /// LightWalletD doesnt return a compact block header, however this could be used to return data if useful.
    ///
    /// This impl is still slow, either CompactBl,ocks should be returned directly from the [`ReadStateService`] or Zaino should hold an internal compact block cache.
    async fn get_compact_block(
        read_state_service: &ReadStateService,
        hash_or_height: String,
        network: &Network,
    ) -> Result<CompactBlock, StateServiceError> {
        let hash_or_height: HashOrHeight = hash_or_height.parse()?;
        let cloned_read_state_service = read_state_service.clone();
        let cloned_network = network.clone();
        let get_block_header_future = tokio::spawn(async move {
            let zebra_state::ReadResponse::BlockHeader {
                header,
                hash,
                height,
                next_block_hash,
            } = StateService::checked_call_decoupled(
                cloned_read_state_service.clone(),
                zebra_state::ReadRequest::BlockHeader(hash_or_height),
            )
            .await
            .map_err(|_| {
                StateServiceError::RpcError(zaino_fetch::jsonrpc::connector::RpcError {
                    // Compatibility with zcashd. Note that since this function
                    // is reused by getblock(), we return the errors expected
                    // by it (they differ whether a hash or a height was passed)
                    code: if hash_or_height.hash().is_some() {
                        INVALID_ADDRESS_OR_KEY_ERROR_CODE.code()
                    } else {
                        MISSING_BLOCK_ERROR_CODE.code()
                    },
                    message: "block height not in best chain".to_string(),
                    data: None,
                })
            })?
            else {
                return Err(StateServiceError::Custom(
                    "Unexpected response to BlockHeader request".to_string(),
                ));
            };

            let zebra_state::ReadResponse::SaplingTree(sapling_tree) =
                StateService::checked_call_decoupled(
                    cloned_read_state_service.clone(),
                    zebra_state::ReadRequest::SaplingTree(hash_or_height),
                )
                .await?
            else {
                return Err(StateServiceError::Custom(
                    "Unexpected response to SaplingTree request".to_string(),
                ));
            };

            // This could be `None` if there's a chain reorg between state queries.
            let sapling_tree = sapling_tree.ok_or_else(|| {
                StateServiceError::RpcError(zaino_fetch::jsonrpc::connector::RpcError {
                    code: MISSING_BLOCK_ERROR_CODE.code(),
                    message: "missing sapling tree for block".to_string(),
                    data: None,
                })
            })?;

            let zebra_state::ReadResponse::Depth(depth) = StateService::checked_call_decoupled(
                cloned_read_state_service,
                zebra_state::ReadRequest::Depth(hash),
            )
            .await?
            else {
                return Err(StateServiceError::Custom(
                    "Unexpected response to Depth request".to_string(),
                ));
            };

            // From <https://zcash.github.io/rpc/getblock.html>
            // TODO: Deduplicate const definition, consider refactoring this to avoid duplicate logic
            const NOT_IN_BEST_CHAIN_CONFIRMATIONS: i64 = -1;

            // Confirmations are one more than the depth.
            // Depth is limited by height, so it will never overflow an i64.
            let confirmations = depth
                .map(|depth| i64::from(depth) + 1)
                .unwrap_or(NOT_IN_BEST_CHAIN_CONFIRMATIONS);

            let mut nonce = *header.nonce;
            nonce.reverse();

            let sapling_activation = NetworkUpgrade::Sapling.activation_height(&cloned_network);
            let sapling_tree_size = sapling_tree.count();
            let final_sapling_root: [u8; 32] =
                if sapling_activation.is_some() && height >= sapling_activation.unwrap() {
                    let mut root: [u8; 32] = sapling_tree.root().into();
                    root.reverse();
                    root
                } else {
                    [0; 32]
                };

            let difficulty = header
                .difficulty_threshold
                .relative_to_network(&cloned_network);

            Ok(GetBlockHeaderObject {
                hash: GetBlockHash(hash),
                confirmations,
                height,
                version: header.version,
                merkle_root: header.merkle_root,
                final_sapling_root,
                sapling_tree_size,
                time: header.time.timestamp(),
                nonce,
                solution: header.solution,
                bits: header.difficulty_threshold,
                difficulty,
                previous_block_hash: GetBlockHash(header.previous_block_hash),
                next_block_hash: next_block_hash.map(GetBlockHash),
            })
        });

        let get_orchard_trees_future = StateService::checked_call_decoupled(
            read_state_service.clone(),
            zebra_state::ReadRequest::OrchardTree(hash_or_height),
        );

        let zebra_state::ReadResponse::Block(Some(block_raw)) =
            StateService::checked_call_decoupled(
                read_state_service.clone(),
                zebra_state::ReadRequest::Block(hash_or_height),
            )
            .await?
        else {
            return Err(StateServiceError::RpcError(
                zaino_fetch::jsonrpc::connector::RpcError {
                    code: MISSING_BLOCK_ERROR_CODE.code(),
                    message: "Block not found".to_string(),
                    data: None,
                },
            ));
        };

        let block_bytes = block_raw.zcash_serialize_to_vec().map_err(|e| {
            StateServiceError::Custom(format!("Failed to serialize block: {:#?}", e))
        })?;
        let mut cursor = Cursor::new(block_bytes);
        let block = zebra_chain::block::Block::zcash_deserialize(&mut cursor).map_err(|e| {
            StateServiceError::Custom(format!("Failed to deserialize block bytes: {:#?}", e))
        })?;
        let vtx = block
            .transactions
            .into_iter()
            .enumerate()
            .filter_map(|(index, tx)| {
                if tx.has_shielded_inputs() || tx.has_shielded_outputs() {
                    Some(tx_to_compact(tx, index as u64))
                } else {
                    None
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        let block_header = get_block_header_future.await??;
        let zebra_state::ReadResponse::OrchardTree(Some(orchard_trees)) =
            get_orchard_trees_future.await?
        else {
            return Err(StateServiceError::Custom(
                "Unexpected response type for OrchardTrees".into(),
            ));
        };
        let chain_metadata = Some(ChainMetadata {
            sapling_commitment_tree_size: block_header.sapling_tree_size.try_into()?,
            orchard_commitment_tree_size: orchard_trees.count().try_into()?,
        });

        let compact_block = CompactBlock {
            proto_version: block.header.version,
            height: block_header.height.0 as u64,
            hash: block_header.hash.0 .0.to_vec(),
            prev_hash: block_header.previous_block_hash.0 .0.to_vec(),
            time: block_header.time.try_into()?,
            header: Vec::new(),
            vtx,
            chain_metadata,
        };

        Ok(compact_block)
    }
}

/// Converts a [`zebra_chain::transaction::Transaction`] into a [`zaino_proto::proto::compact_formats::CompactTx`].
///
/// Notes:
///
/// Currently only supports V4 and V5 transactions.
///
/// LightWalletD currently does not return a fee and is not currently priority here. Please open an Issue or PR at the Zingo-Indexer github (https://github.com/zingolabs/zingo-indexer) if you require this functionality.
fn tx_to_compact(
    transaction: std::sync::Arc<Transaction>,
    index: u64,
) -> Result<CompactTx, StateServiceError> {
    let (spends, outputs) = if transaction.has_sapling_shielded_data() {
        (
            transaction
                .sapling_nullifiers()
                .map(|nullifier| CompactSaplingSpend {
                    nf: nullifier.0.to_vec(),
                })
                .collect(),
            transaction
                .sapling_outputs()
                .map(|output| CompactSaplingOutput {
                    cmu: output.cm_u.to_bytes().to_vec(),
                    ephemeral_key: <[u8; 32]>::from(output.ephemeral_key).to_vec(),
                    ciphertext: output.enc_ciphertext.zcash_serialize_to_vec().unwrap(),
                })
                .collect(),
        )
    } else {
        (Vec::new(), Vec::new())
    };

    let actions = if transaction.has_orchard_shielded_data() {
        transaction
            .orchard_actions()
            .map(|action| CompactOrchardAction {
                nullifier: <[u8; 32]>::from(action.nullifier).to_vec(),
                cmx: <[u8; 32]>::from(action.cm_x).to_vec(),
                ephemeral_key: <[u8; 32]>::from(action.ephemeral_key).to_vec(),
                ciphertext: action.enc_ciphertext.zcash_serialize_to_vec().unwrap(),
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(CompactTx {
        index,
        hash: transaction.hash().0.to_vec(),
        fee: 0,
        spends,
        outputs,
        actions,
    })
}

/// !!! NOTE / TODO: This code should be retested before continued development, once zebra regtest is fully operational.
#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use super::*;
    use futures::stream::StreamExt;
    use zaino_proto::proto::service::{compact_tx_streamer_server::CompactTxStreamer, BlockId};
    use zaino_testutils::{TestManager, ZEBRAD_CHAIN_CACHE_BIN, ZEBRAD_TESTNET_CACHE_BIN};
    use zcash_local_net::validator::Validator;
    use zebra_chain::parameters::Network;

    #[tokio::test]
    async fn launch_state_regtest_service_no_cache() {
        let mut test_manager = TestManager::launch("zebrad", None, None, false, false)
            .await
            .unwrap();

        let state_service = StateService::spawn(StateServiceConfig::new(
            zebra_state::Config {
                cache_dir: test_manager.data_dir.clone(),
                ephemeral: false,
                delete_old_database: true,
                debug_stop_at_height: None,
                debug_validity_check_interval: None,
            },
            SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                test_manager.zebrad_rpc_listen_port,
            ),
            None,
            None,
            None,
            None,
            Network::new_regtest(Some(1), Some(1)),
        ))
        .await
        .unwrap();

        assert_eq!(
            state_service.fetch_status_from_validator().await,
            StatusType::Ready
        );

        test_manager.close().await;
    }

    #[tokio::test]
    async fn launch_state_regtest_service_with_cache() {
        let mut test_manager =
            TestManager::launch("zebrad", None, ZEBRAD_CHAIN_CACHE_BIN.clone(), false, false)
                .await
                .unwrap();

        let state_service = StateService::spawn(StateServiceConfig::new(
            zebra_state::Config {
                cache_dir: test_manager.data_dir.clone(),
                ephemeral: false,
                delete_old_database: true,
                debug_stop_at_height: None,
                debug_validity_check_interval: None,
            },
            SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                test_manager.zebrad_rpc_listen_port,
            ),
            None,
            None,
            None,
            None,
            Network::new_regtest(Some(1), Some(1)),
        ))
        .await
        .unwrap();

        assert_eq!(
            state_service.fetch_status_from_validator().await,
            StatusType::Ready
        );

        test_manager.close().await;
    }

    #[tokio::test]
    async fn state_service_regtest_get_info() {
        let mut test_manager =
            TestManager::launch("zebrad", None, ZEBRAD_CHAIN_CACHE_BIN.clone(), false, false)
                .await
                .unwrap();

        let state_service = StateService::spawn(StateServiceConfig::new(
            zebra_state::Config {
                cache_dir: test_manager.data_dir.clone(),
                ephemeral: false,
                delete_old_database: true,
                debug_stop_at_height: None,
                debug_validity_check_interval: None,
            },
            SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                test_manager.zebrad_rpc_listen_port,
            ),
            None,
            None,
            None,
            None,
            Network::new_regtest(Some(1), Some(1)),
        ))
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
        let state_service_get_info = state_service.get_info().await.unwrap();
        let state_service_duration = state_start.elapsed();

        let fetch_start = tokio::time::Instant::now();
        let fetch_service_get_info = fetch_service.get_info().await.unwrap();
        let fetch_service_duration = fetch_start.elapsed();

        assert_eq!(state_service_get_info, fetch_service_get_info.into());

        println!("GetInfo responses correct. State-Service processing time: {:?} - fetch-Service processing time: {:?}.", state_service_duration, fetch_service_duration);

        test_manager.close().await;
    }

    #[tokio::test]
    async fn state_service_regtest_get_blockchain_info() {
        let mut test_manager =
            TestManager::launch("zebrad", None, ZEBRAD_CHAIN_CACHE_BIN.clone(), false, false)
                .await
                .unwrap();

        let state_service = StateService::spawn(StateServiceConfig::new(
            zebra_state::Config {
                cache_dir: test_manager.data_dir.clone(),
                ephemeral: false,
                delete_old_database: true,
                debug_stop_at_height: None,
                debug_validity_check_interval: None,
            },
            SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                test_manager.zebrad_rpc_listen_port,
            ),
            None,
            None,
            None,
            None,
            Network::new_regtest(Some(1), Some(1)),
        ))
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
        let fetch_service_get_blockchain_info: GetBlockChainInfo =
            fetch_service_get_blockchain_info.into();

        // Zaino-Fetch does not return value_pools, ingnore this field.
        assert_eq!(
            (
                state_service_get_blockchain_info.chain(),
                state_service_get_blockchain_info.blocks(),
                state_service_get_blockchain_info.best_block_hash(),
                state_service_get_blockchain_info.estimated_height(),
                state_service_get_blockchain_info.upgrades(),
                state_service_get_blockchain_info.consensus(),
            ),
            (
                fetch_service_get_blockchain_info.chain(),
                fetch_service_get_blockchain_info.blocks(),
                fetch_service_get_blockchain_info.best_block_hash(),
                fetch_service_get_blockchain_info.estimated_height(),
                fetch_service_get_blockchain_info.upgrades(),
                fetch_service_get_blockchain_info.consensus(),
            )
        );

        println!("GetBlockChainInfo responses correct. State-Service processing time: {:?} - fetch-Service processing time: {:?}.", state_service_duration, fetch_service_duration);

        test_manager.close().await;
    }

    /// Bug documented in https://github.com/zingolabs/zaino/issues/146.
    #[tokio::test]
    async fn state_service_get_blockchain_info_no_cache() {
        let mut test_manager = TestManager::launch("zebrad", None, None, false, false)
            .await
            .unwrap();
        test_manager.local_net.generate_blocks(1).await.unwrap();

        let state_service = StateService::spawn(StateServiceConfig::new(
            zebra_state::Config {
                cache_dir: test_manager.data_dir.clone(),
                ephemeral: false,
                delete_old_database: true,
                debug_stop_at_height: None,
                debug_validity_check_interval: None,
            },
            SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                test_manager.zebrad_rpc_listen_port,
            ),
            None,
            None,
            None,
            None,
            Network::new_regtest(Some(1), Some(1)),
        ))
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
        let fetch_service_get_blockchain_info: GetBlockChainInfo =
            fetch_service_get_blockchain_info.into();

        println!(
            "Fetch Service Chain Height: {}",
            fetch_service_get_blockchain_info.blocks().0
        );
        println!(
            "State Service Chain Height: {}",
            state_service_get_blockchain_info.blocks().0
        );

        test_manager.local_net.print_stdout();

        // Zaino-Fetch does not return value_pools, ingnore this field.
        assert_eq!(
            (
                state_service_get_blockchain_info.chain(),
                state_service_get_blockchain_info.blocks(),
                state_service_get_blockchain_info.best_block_hash(),
                state_service_get_blockchain_info.estimated_height(),
                state_service_get_blockchain_info.upgrades(),
                state_service_get_blockchain_info.consensus(),
            ),
            (
                fetch_service_get_blockchain_info.chain(),
                fetch_service_get_blockchain_info.blocks(),
                fetch_service_get_blockchain_info.best_block_hash(),
                fetch_service_get_blockchain_info.estimated_height(),
                fetch_service_get_blockchain_info.upgrades(),
                fetch_service_get_blockchain_info.consensus(),
            )
        );

        println!("GetBlockChainInfo responses correct. State-Service processing time: {:?} - fetch-Service processing time: {:?}.", state_service_duration, fetch_service_duration);

        test_manager.close().await;
    }

    #[tokio::test]
    async fn state_service_regtest_get_block_raw() {
        let mut test_manager =
            TestManager::launch("zebrad", None, ZEBRAD_CHAIN_CACHE_BIN.clone(), false, false)
                .await
                .unwrap();

        let state_service = StateService::spawn(StateServiceConfig::new(
            zebra_state::Config {
                cache_dir: test_manager.data_dir.clone(),
                ephemeral: false,
                delete_old_database: true,
                debug_stop_at_height: None,
                debug_validity_check_interval: None,
            },
            SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                test_manager.zebrad_rpc_listen_port,
            ),
            None,
            None,
            None,
            None,
            Network::new_regtest(Some(1), Some(1)),
        ))
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
        let state_service_get_blockchain_info = state_service
            .get_block("1".to_string(), Some(0))
            .await
            .unwrap();
        let state_service_duration = state_start.elapsed();

        let fetch_start = tokio::time::Instant::now();
        let fetch_service_get_blockchain_info = fetch_service
            .get_block("1".to_string(), Some(0))
            .await
            .unwrap();
        let fetch_service_duration = fetch_start.elapsed();

        assert_eq!(
            state_service_get_blockchain_info,
            fetch_service_get_blockchain_info.into()
        );

        println!("GetBlock(raw) responses correct. State-Service processing time: {:?} - fetch-Service processing time: {:?}.", state_service_duration, fetch_service_duration);

        test_manager.close().await;
    }

    #[tokio::test]
    async fn state_service_regtest_get_block_object() {
        let mut test_manager =
            TestManager::launch("zebrad", None, ZEBRAD_CHAIN_CACHE_BIN.clone(), false, false)
                .await
                .unwrap();

        let state_service = StateService::spawn(StateServiceConfig::new(
            zebra_state::Config {
                cache_dir: test_manager.data_dir.clone(),
                ephemeral: false,
                delete_old_database: true,
                debug_stop_at_height: None,
                debug_validity_check_interval: None,
            },
            SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                test_manager.zebrad_rpc_listen_port,
            ),
            None,
            None,
            None,
            None,
            Network::new_regtest(Some(1), Some(1)),
        ))
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
        let state_service_get_blockchain_info = state_service
            .get_block("1".to_string(), Some(1))
            .await
            .unwrap();
        let state_service_duration = state_start.elapsed();

        let fetch_start = tokio::time::Instant::now();
        let fetch_service_get_blockchain_info = fetch_service
            .get_block("1".to_string(), Some(1))
            .await
            .unwrap();
        let fetch_service_duration = fetch_start.elapsed();

        // Zaino-fetch only returns fields that are required by the lightwallet services. Check those fields match and ignore the others.
        match (
            state_service_get_blockchain_info,
            fetch_service_get_blockchain_info.into(),
        ) {
            (
                zebra_rpc::methods::GetBlock::Object {
                    hash: state_hash,
                    confirmations: state_confirmations,
                    height: state_height,
                    time: state_time,
                    tx: state_tx,
                    trees: state_trees,
                    ..
                },
                zebra_rpc::methods::GetBlock::Object {
                    hash: fetch_hash,
                    confirmations: fetch_confirmations,
                    height: fetch_height,
                    time: fetch_time,
                    tx: fetch_tx,
                    trees: fetch_trees,
                    ..
                },
            ) => {
                assert_eq!(state_hash, fetch_hash);
                assert_eq!(state_confirmations, fetch_confirmations);
                assert_eq!(state_height, fetch_height);
                assert_eq!(state_time, fetch_time);
                assert_eq!(state_tx, fetch_tx);
                assert_eq!(state_trees, fetch_trees);
            }
            _ => panic!("Mismatched variants or unexpected types in block response"),
        }

        println!("GetBlock(object) responses correct. State-Service processing time: {:?} - fetch-Service processing time: {:?}.", state_service_duration, fetch_service_duration);

        test_manager.close().await;
    }

    #[tokio::test]
    async fn state_service_regtest_get_block_compact() {
        let mut test_manager =
            TestManager::launch("zebrad", None, ZEBRAD_CHAIN_CACHE_BIN.clone(), false, false)
                .await
                .unwrap();
        let zebra_uri = format!("http://127.0.0.1:{}", test_manager.zebrad_rpc_listen_port)
            .parse::<http::Uri>()
            .expect("Failed to convert URL to URI");

        let state_service = StateService::spawn(StateServiceConfig::new(
            zebra_state::Config {
                cache_dir: test_manager.data_dir.clone(),
                ephemeral: false,
                delete_old_database: true,
                debug_stop_at_height: None,
                debug_validity_check_interval: None,
            },
            SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                test_manager.zebrad_rpc_listen_port,
            ),
            None,
            None,
            None,
            None,
            Network::new_regtest(Some(1), Some(1)),
        ))
        .await
        .unwrap();

        let state_start = tokio::time::Instant::now();
        let state_service_get_compact_block = StateService::get_compact_block(
            &state_service.read_state_service,
            "1".to_string(),
            &state_service.config.network,
        )
        .await
        .unwrap();
        let state_service_duration = state_start.elapsed();

        let fetch_start = tokio::time::Instant::now();
        let fetch_service_get_compact_block = zaino_fetch::chain::block::get_block_from_node(
            &zebra_uri,
            &1,
            Some("xxxxxx".to_string()),
            Some("xxxxxx".to_string()),
        )
        .await
        .unwrap();
        let fetch_service_duration = fetch_start.elapsed();

        assert_eq!(
            state_service_get_compact_block,
            fetch_service_get_compact_block,
        );

        println!("GetCompactBlock responses correct. State-Service processing time: {:?} - fetch-Service processing time: {:?}.", state_service_duration, fetch_service_duration);

        test_manager.close().await;
    }

    #[tokio::test]
    async fn state_service_regtest_get_block_range() {
        let mut test_manager =
            TestManager::launch("zebrad", None, ZEBRAD_CHAIN_CACHE_BIN.clone(), false, false)
                .await
                .unwrap();
        let zebra_uri = format!("http://127.0.0.1:{}", test_manager.zebrad_rpc_listen_port)
            .parse::<http::Uri>()
            .expect("Failed to convert URL to URI");

        let block_range = BlockRange {
            start: Some(BlockId {
                height: 50,
                hash: Vec::new(),
            }),
            end: Some(BlockId {
                height: 1,
                hash: Vec::new(),
            }),
        };
        let state_service = StateService::spawn(StateServiceConfig::new(
            zebra_state::Config {
                cache_dir: test_manager.data_dir.clone(),
                ephemeral: false,
                delete_old_database: true,
                debug_stop_at_height: None,
                debug_validity_check_interval: None,
            },
            SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                test_manager.zebrad_rpc_listen_port,
            ),
            None,
            None,
            None,
            None,
            Network::new_regtest(Some(1), Some(1)),
        ))
        .await
        .unwrap();
        let grpc_service = zaino_serve::rpc::GrpcClient {
            zebrad_rpc_uri: zebra_uri,
            online: test_manager.online.clone(),
        };

        let state_start = tokio::time::Instant::now();
        let state_service_stream = state_service
            .get_block_range(block_range.clone())
            .await
            .unwrap();
        let state_service_compact_blocks: Vec<_> = state_service_stream.collect().await;
        let state_service_duration = state_start.elapsed();

        let fetch_start = tokio::time::Instant::now();
        let fetch_service_stream = grpc_service
            .get_block_range(tonic::Request::new(block_range))
            .await
            .unwrap()
            .into_inner();
        let fetch_service_compact_blocks: Vec<_> = fetch_service_stream.collect().await;
        let fetch_service_duration = fetch_start.elapsed();

        // Extract only the successful `CompactBlock` results
        let state_blocks: Vec<_> = state_service_compact_blocks
            .into_iter()
            .filter_map(|result| result.ok())
            .collect();
        let fetch_blocks: Vec<_> = fetch_service_compact_blocks
            .into_iter()
            .filter_map(|result| result.ok())
            .collect();

        assert_eq!(state_blocks, fetch_blocks);

        println!("GetBlockRange responses correct. State-Service processing time: {:?} - fetch-Service processing time: {:?}.", state_service_duration, fetch_service_duration);

        test_manager.close().await;
    }

    #[tokio::test]
    async fn state_service_testnet_get_block_range_large() {
        let mut test_manager = TestManager::launch(
            "zebrad",
            Some(zcash_local_net::network::Network::Testnet),
            ZEBRAD_TESTNET_CACHE_BIN.clone(),
            false,
            false,
        )
        .await
        .unwrap();

        let block_range = BlockRange {
            start: Some(BlockId {
                height: 2000000,
                hash: Vec::new(),
            }),
            end: Some(BlockId {
                height: 3000000,
                hash: Vec::new(),
            }),
        };

        let state_service = StateService::spawn(StateServiceConfig::new(
            zebra_state::Config {
                cache_dir: test_manager.data_dir.clone(),
                ephemeral: false,
                delete_old_database: true,
                debug_stop_at_height: None,
                debug_validity_check_interval: None,
            },
            SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                test_manager.zebrad_rpc_listen_port,
            ),
            None,
            None,
            None,
            None,
            Network::new_default_testnet(),
        ))
        .await
        .unwrap();

        let num_blocks =
            block_range.clone().end.unwrap().height - block_range.clone().start.unwrap().height;
        println!("Fetching {} blocks in range: {:?}", num_blocks, block_range);

        let state_start = tokio::time::Instant::now();
        let state_service_stream = state_service
            .get_block_range(block_range.clone())
            .await
            .unwrap();
        let state_service_compact_blocks: Vec<_> = state_service_stream.collect().await;
        let state_service_duration = state_start.elapsed();

        let state_blocks: Vec<_> = state_service_compact_blocks
            .into_iter()
            .filter_map(|result| result.ok())
            .collect();

        println!("First block in range: {:?}", state_blocks.first());
        println!("Last block in range: {:?}", state_blocks.last());
        println!("GetBlockRange response received. State-Service fetch 1,000,000 blocks in processing time: {:?}.", state_service_duration);

        test_manager.close().await;
    }
}
