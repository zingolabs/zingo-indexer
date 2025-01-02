//! Zcash chain fetch and tx submission service backed by zcashds JsonRPC service.

use crate::{
    config::FetchServiceConfig,
    error::FetchServiceError,
    get_build_info,
    indexer::{LightWalletIndexer, ZcashIndexer},
    status::{AtomicStatus, StatusType},
    stream::{
        AddressStream, CompactBlockStream, CompactTransactionStream, RawTransactionStream,
        SubtreeRootReplyStream, UtxoReplyStream,
    },
    ServiceMetadata,
};
use tonic::async_trait;
use zaino_fetch::jsonrpc::connector::{test_node_and_return_uri, JsonRpcConnector};
use zaino_proto::proto::{
    compact_formats::CompactBlock,
    service::{
        AddressList, Balance, BlockId, BlockRange, ChainSpec, Duration, Exclude,
        GetAddressUtxosArg, GetAddressUtxosReplyList, GetSubtreeRootsArg, LightdInfo, PingResponse,
        RawTransaction, SendResponse, TransparentAddressBlockFilter, TreeState, TxFilter,
    },
};
use zebra_chain::subtree::NoteCommitmentSubtreeIndex;
use zebra_rpc::methods::{
    trees::{GetSubtrees, GetTreestate},
    AddressBalance, AddressStrings, GetAddressTxIdsRequest, GetAddressUtxos, GetBlock,
    GetBlockChainInfo, GetInfo, GetRawTransaction, SentTransactionHash,
};

/// Chain fetch service backed by Zcashds JsonRPC service.
#[derive(Debug)]
pub struct FetchService {
    /// JsonRPC Client.
    fetcher: JsonRpcConnector,
    // TODO: Add Internal Non-Finalised State
    /// Sync task handle.
    // sync_task_handle: tokio::task::JoinHandle<()>,
    /// Service metadata.
    data: ServiceMetadata,
    /// StateService config data.
    _config: FetchServiceConfig,
    /// Thread-safe status indicator.
    status: AtomicStatus,
}

impl FetchService {
    /// Initializes a new StateService instance and starts sync process.
    pub async fn spawn(config: FetchServiceConfig) -> Result<Self, FetchServiceError> {
        let rpc_uri = test_node_and_return_uri(
            &config.validator_rpc_address.port(),
            Some(config.validator_rpc_user.clone()),
            Some(config.validator_rpc_password.clone()),
        )
        .await?;

        let fetcher = JsonRpcConnector::new(
            rpc_uri,
            Some(config.validator_rpc_user.clone()),
            Some(config.validator_rpc_password.clone()),
        )
        .await?;

        let zebra_build_data = fetcher.get_info().await?;

        let data = ServiceMetadata {
            build_info: get_build_info(),
            network: config.network.clone(),
            zebra_build: zebra_build_data.build,
            zebra_subversion: zebra_build_data.subversion,
        };

        let state_service = Self {
            fetcher,
            data,
            _config: config,
            status: AtomicStatus::new(StatusType::Spawning.into()),
        };

        state_service.status.store(StatusType::Syncing.into());

        // TODO: Wait for Non-Finalised state to sync or for mempool to come online.

        state_service.status.store(StatusType::Ready.into());

        Ok(state_service)
    }

    /// Fetches the current status
    pub fn status(&self) -> StatusType {
        self.status.load().into()
    }

    /// Shuts down the StateService.
    pub fn close(&mut self) {
        // self.sync_task_handle.abort();
    }
}

impl Drop for FetchService {
    fn drop(&mut self) {
        self.close()
    }
}

#[async_trait]
impl ZcashIndexer for FetchService {
    type Error = FetchServiceError;

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
    async fn get_info(&self) -> Result<GetInfo, Self::Error> {
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
    async fn get_blockchain_info(&self) -> Result<GetBlockChainInfo, Self::Error> {
        Ok(self.fetcher.get_blockchain_info().await?.into())
    }

    /// Returns the total balance of a provided `addresses` in an [`AddressBalance`] instance.
    ///
    /// zcashd reference: [`getaddressbalance`](https://zcash.github.io/rpc/getaddressbalance.html)
    /// method: post
    /// tags: address
    ///
    /// # Parameters
    ///
    /// - `address_strings`: (object, example={"addresses": ["tmYXBYJj1K7vhejSec5osXK2QsGa5MTisUQ"]}) A JSON map with a single entry
    ///     - `addresses`: (array of strings) A list of base-58 encoded addresses.
    ///
    /// # Notes
    ///
    /// zcashd also accepts a single string parameter instead of an array of strings, but Zebra
    /// doesn't because lightwalletd always calls this RPC with an array of addresses.
    ///
    /// zcashd also returns the total amount of Zatoshis received by the addresses, but Zebra
    /// doesn't because lightwalletd doesn't use that information.
    ///
    /// The RPC documentation says that the returned object has a string `balance` field, but
    /// zcashd actually [returns an
    /// integer](https://github.com/zcash/lightwalletd/blob/bdaac63f3ee0dbef62bde04f6817a9f90d483b00/common/common.go#L128-L130).
    async fn z_get_address_balance(
        &self,
        address_strings: AddressStrings,
    ) -> Result<AddressBalance, Self::Error> {
        Ok(self
            .fetcher
            .get_address_balance(address_strings.valid_address_strings().map_err(|code| {
                FetchServiceError::RpcError(zaino_fetch::jsonrpc::connector::RpcError {
                    code: code as i32 as i64,
                    message: "Invalid address provided".to_string(),
                    data: None,
                })
            })?)
            .await?
            .into())
    }

    /// Sends the raw bytes of a signed transaction to the local node's mempool, if the transaction is valid.
    /// Returns the [`SentTransactionHash`] for the transaction, as a JSON string.
    ///
    /// zcashd reference: [`sendrawtransaction`](https://zcash.github.io/rpc/sendrawtransaction.html)
    /// method: post
    /// tags: transaction
    ///
    /// # Parameters
    ///
    /// - `raw_transaction_hex`: (string, required, example="signedhex") The hex-encoded raw transaction bytes.
    ///
    /// # Notes
    ///
    /// zcashd accepts an optional `allowhighfees` parameter. Zebra doesn't support this parameter,
    /// because lightwalletd doesn't use it.
    async fn send_raw_transaction(
        &self,
        raw_transaction_hex: String,
    ) -> Result<SentTransactionHash, Self::Error> {
        Ok(self
            .fetcher
            .send_raw_transaction(raw_transaction_hex)
            .await?
            .into())
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
    async fn z_get_block(
        &self,
        hash_or_height: String,
        verbosity: Option<u8>,
    ) -> Result<GetBlock, Self::Error> {
        Ok(self
            .fetcher
            .get_block(hash_or_height, verbosity)
            .await?
            .try_into()?)
    }

    /// Returns all transaction ids in the memory pool, as a JSON array.
    ///
    /// zcashd reference: [`getrawmempool`](https://zcash.github.io/rpc/getrawmempool.html)
    /// method: post
    /// tags: blockchain
    async fn get_raw_mempool(&self) -> Result<Vec<String>, Self::Error> {
        Ok(self.fetcher.get_raw_mempool().await?.transactions)
    }

    /// Returns information about the given block's Sapling & Orchard tree state.
    ///
    /// zcashd reference: [`z_gettreestate`](https://zcash.github.io/rpc/z_gettreestate.html)
    /// method: post
    /// tags: blockchain
    ///
    /// # Parameters
    ///
    /// - `hash | height`: (string, required, example="00000000febc373a1da2bd9f887b105ad79ddc26ac26c2b28652d64e5207c5b5") The block hash or height.
    ///
    /// # Notes
    ///
    /// The zcashd doc reference above says that the parameter "`height` can be
    /// negative where -1 is the last known valid block". On the other hand,
    /// `lightwalletd` only uses positive heights, so Zebra does not support
    /// negative heights.
    async fn z_get_treestate(&self, hash_or_height: String) -> Result<GetTreestate, Self::Error> {
        Ok(self
            .fetcher
            .get_treestate(hash_or_height)
            .await?
            .try_into()?)
    }

    /// Returns information about a range of Sapling or Orchard subtrees.
    ///
    /// zcashd reference: [`z_getsubtreesbyindex`](https://zcash.github.io/rpc/z_getsubtreesbyindex.html) - TODO: fix link
    /// method: post
    /// tags: blockchain
    ///
    /// # Parameters
    ///
    /// - `pool`: (string, required) The pool from which subtrees should be returned. Either "sapling" or "orchard".
    /// - `start_index`: (number, required) The index of the first 2^16-leaf subtree to return.
    /// - `limit`: (number, optional) The maximum number of subtree values to return.
    ///
    /// # Notes
    ///
    /// While Zebra is doing its initial subtree index rebuild, subtrees will become available
    /// starting at the chain tip. This RPC will return an empty list if the `start_index` subtree
    /// exists, but has not been rebuilt yet. This matches `zcashd`'s behaviour when subtrees aren't
    /// available yet. (But `zcashd` does its rebuild before syncing any blocks.)
    async fn z_get_subtrees_by_index(
        &self,
        pool: String,
        start_index: NoteCommitmentSubtreeIndex,
        limit: Option<NoteCommitmentSubtreeIndex>,
    ) -> Result<GetSubtrees, Self::Error> {
        Ok(self
            .fetcher
            .get_subtrees_by_index(
                pool,
                start_index.0,
                limit.and_then(|limit_index| Some(limit_index.0)),
            )
            .await?
            .into())
    }

    /// Returns the raw transaction data, as a [`GetRawTransaction`] JSON string or structure.
    ///
    /// zcashd reference: [`getrawtransaction`](https://zcash.github.io/rpc/getrawtransaction.html)
    /// method: post
    /// tags: transaction
    ///
    /// # Parameters
    ///
    /// - `txid`: (string, required, example="mytxid") The transaction ID of the transaction to be returned.
    /// - `verbose`: (number, optional, default=0, example=1) If 0, return a string of hex-encoded data, otherwise return a JSON object.
    ///
    /// # Notes
    ///
    /// We don't currently support the `blockhash` parameter since lightwalletd does not
    /// use it.
    ///
    /// In verbose mode, we only expose the `hex` and `height` fields since
    /// lightwalletd uses only those:
    /// <https://github.com/zcash/lightwalletd/blob/631bb16404e3d8b045e74a7c5489db626790b2f6/common/common.go#L119>
    async fn get_raw_transaction(
        &self,
        txid_hex: String,
        verbose: Option<u8>,
    ) -> Result<GetRawTransaction, Self::Error> {
        Ok(self
            .fetcher
            .get_raw_transaction(txid_hex, verbose)
            .await?
            .into())
    }

    /// Returns the transaction ids made by the provided transparent addresses.
    ///
    /// zcashd reference: [`getaddresstxids`](https://zcash.github.io/rpc/getaddresstxids.html)
    /// method: post
    /// tags: address
    ///
    /// # Parameters
    ///
    /// - `request`: (object, required, example={\"addresses\": [\"tmYXBYJj1K7vhejSec5osXK2QsGa5MTisUQ\"], \"start\": 1000, \"end\": 2000}) A struct with the following named fields:
    ///     - `addresses`: (json array of string, required) The addresses to get transactions from.
    ///     - `start`: (numeric, required) The lower height to start looking for transactions (inclusive).
    ///     - `end`: (numeric, required) The top height to stop looking for transactions (inclusive).
    ///
    /// # Notes
    ///
    /// Only the multi-argument format is used by lightwalletd and this is what we currently support:
    /// <https://github.com/zcash/lightwalletd/blob/631bb16404e3d8b045e74a7c5489db626790b2f6/common/common.go#L97-L102>
    async fn get_address_tx_ids(
        &self,
        request: GetAddressTxIdsRequest,
    ) -> Result<Vec<String>, Self::Error> {
        let (addresses, start, end) = request.into_parts();
        Ok(self
            .fetcher
            .get_address_txids(addresses, start, end)
            .await?
            .transactions)
    }

    /// Returns all unspent outputs for a list of addresses.
    ///
    /// zcashd reference: [`getaddressutxos`](https://zcash.github.io/rpc/getaddressutxos.html)
    /// method: post
    /// tags: address
    ///
    /// # Parameters
    ///
    /// - `addresses`: (array, required, example={\"addresses\": [\"tmYXBYJj1K7vhejSec5osXK2QsGa5MTisUQ\"]}) The addresses to get outputs from.
    ///
    /// # Notes
    ///
    /// lightwalletd always uses the multi-address request, without chaininfo:
    /// <https://github.com/zcash/lightwalletd/blob/master/frontend/service.go#L402>
    async fn z_get_address_utxos(
        &self,
        address_strings: AddressStrings,
    ) -> Result<Vec<GetAddressUtxos>, Self::Error> {
        Ok(self
            .fetcher
            .get_address_utxos(address_strings.valid_address_strings().map_err(|code| {
                FetchServiceError::RpcError(zaino_fetch::jsonrpc::connector::RpcError {
                    code: code as i32 as i64,
                    message: "Invalid address provided".to_string(),
                    data: None,
                })
            })?)
            .await?
            .into_iter()
            .map(|utxos| utxos.into())
            .collect())
    }
}

#[async_trait]
impl LightWalletIndexer for FetchService {
    type Error = FetchServiceError;

    /// Return the height of the tip of the best chain
    async fn get_latest_block(&self, request: ChainSpec) -> Result<BlockId, Self::Error> {
        unimplemented!()
    }

    /// Return the compact block corresponding to the given block identifier
    async fn get_block(&self, request: BlockId) -> Result<CompactBlock, Self::Error> {
        unimplemented!()
    }

    /// Same as GetBlock except actions contain only nullifiers
    async fn get_block_nullifiers(&self, request: BlockId) -> Result<CompactBlock, Self::Error> {
        unimplemented!()
    }

    /// Return a list of consecutive compact blocks
    async fn get_block_range(
        &self,
        request: BlockRange,
    ) -> Result<CompactBlockStream, Self::Error> {
        unimplemented!()
    }

    /// Same as GetBlockRange except actions contain only nullifiers
    async fn get_block_range_nullifiers(
        &self,
        request: BlockRange,
    ) -> Result<CompactBlockStream, Self::Error> {
        unimplemented!()
    }

    /// Return the requested full (not compact) transaction (as from zcashd)
    async fn get_transaction(&self, request: TxFilter) -> Result<RawTransaction, Self::Error> {
        unimplemented!()
    }

    /// Submit the given transaction to the Zcash network
    async fn send_transaction(&self, request: RawTransaction) -> Result<SendResponse, Self::Error> {
        unimplemented!()
    }

    /// Return the txids corresponding to the given t-address within the given block range
    async fn get_taddress_txids(
        &self,
        request: TransparentAddressBlockFilter,
    ) -> Result<RawTransactionStream, Self::Error> {
        unimplemented!()
    }

    /// Returns the total balance for a list of taddrs
    async fn get_taddress_balance(&self, request: AddressList) -> Result<Balance, Self::Error> {
        unimplemented!()
    }

    /// Returns the total balance for a list of taddrs
    ///
    /// TODO: Update input type.
    async fn get_taddress_balance_stream(
        &self,
        request: AddressStream,
    ) -> Result<Balance, Self::Error> {
        unimplemented!()
    }

    /// Return the compact transactions currently in the mempool; the results
    /// can be a few seconds out of date. If the Exclude list is empty, return
    /// all transactions; otherwise return all *except* those in the Exclude list
    /// (if any); this allows the client to avoid receiving transactions that it
    /// already has (from an earlier call to this rpc). The transaction IDs in the
    /// Exclude list can be shortened to any number of bytes to make the request
    /// more bandwidth-efficient; if two or more transactions in the mempool
    /// match a shortened txid, they are all sent (none is excluded). Transactions
    /// in the exclude list that don't exist in the mempool are ignored.
    async fn get_mempool_tx(
        &self,
        request: Exclude,
    ) -> Result<CompactTransactionStream, Self::Error> {
        unimplemented!()
    }

    /// Return a stream of current Mempool transactions. This will keep the output stream open while
    /// there are mempool transactions. It will close the returned stream when a new block is mined.
    async fn get_mempool_stream(&self) -> Result<RawTransactionStream, Self::Error> {
        unimplemented!()
    }

    /// GetTreeState returns the note commitment tree state corresponding to the given block.
    /// See section 3.7 of the Zcash protocol specification. It returns several other useful
    /// values also (even though they can be obtained using GetBlock).
    /// The block can be specified by either height or hash.
    async fn get_tree_state(&self, request: BlockId) -> Result<TreeState, Self::Error> {
        unimplemented!()
    }

    /// GetLatestTreeState returns the note commitment tree state corresponding to the chain tip.
    async fn get_latest_tree_state(&self) -> Result<TreeState, Self::Error> {
        unimplemented!()
    }

    /// Returns a stream of information about roots of subtrees of the Sapling and Orchard
    /// note commitment trees.
    async fn get_subtree_roots(
        &self,
        request: GetSubtreeRootsArg,
    ) -> Result<SubtreeRootReplyStream, Self::Error> {
        unimplemented!()
    }

    /// Returns all unspent outputs for a list of addresses.
    ///
    /// Ignores all utxos below block height [GetAddressUtxosArg.start_height].
    /// Returns max [GetAddressUtxosArg.max_entries] utxos, or unrestricted if [GetAddressUtxosArg.max_entries] = 0.
    /// Utxos are collected and returned as a single Vec.
    async fn get_address_utxos(
        &self,
        request: GetAddressUtxosArg,
    ) -> Result<GetAddressUtxosReplyList, Self::Error> {
        unimplemented!()
    }

    /// Returns all unspent outputs for a list of addresses.
    ///
    /// Ignores all utxos below block height [GetAddressUtxosArg.start_height].
    /// Returns max [GetAddressUtxosArg.max_entries] utxos, or unrestricted if [GetAddressUtxosArg.max_entries] = 0.
    /// Utxos are returned in a stream.
    async fn get_address_utxos_stream(
        &self,
        request: GetAddressUtxosArg,
    ) -> Result<UtxoReplyStream, Self::Error> {
        unimplemented!()
    }

    /// Return information about this lightwalletd instance and the blockchain
    async fn get_lightd_info(&self) -> Result<LightdInfo, Self::Error> {
        unimplemented!()
    }

    /// Testing-only, requires lightwalletd --ping-very-insecure (do not enable in production)
    async fn ping(&self, request: Duration) -> Result<PingResponse, Self::Error> {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use super::*;
    use zaino_testutils::{TestManager, ZCASHD_CHAIN_CACHE_BIN, ZEBRAD_CHAIN_CACHE_BIN};
    use zcash_local_net::validator::Validator;
    use zebra_chain::parameters::Network;

    #[tokio::test]
    async fn launch_fetch_service_zcashd_regtest_no_cache() {
        launch_fetch_service("zcashd", None).await;
    }

    #[tokio::test]
    async fn launch_fetch_service_zcashd_regtest_with_cache() {
        launch_fetch_service("zcashd", ZCASHD_CHAIN_CACHE_BIN.clone()).await;
    }

    #[tokio::test]
    async fn launch_fetch_service_zebrad_regtest_no_cache() {
        launch_fetch_service("zebrad", None).await;
    }

    #[tokio::test]
    async fn launch_fetch_service_zebrad_regtest_with_cache() {
        launch_fetch_service("zebrad", ZEBRAD_CHAIN_CACHE_BIN.clone()).await;
    }

    async fn launch_fetch_service(validator: &str, chain_cache: Option<std::path::PathBuf>) {
        let mut test_manager = TestManager::launch(validator, None, chain_cache, false, false)
            .await
            .unwrap();

        let fetch_service = FetchService::spawn(FetchServiceConfig::new(
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

        assert_eq!(fetch_service.status(), StatusType::Ready);
        dbg!(fetch_service.data.clone());
        dbg!(fetch_service.get_info().await.unwrap());
        dbg!(fetch_service.get_blockchain_info().await.unwrap().blocks());

        test_manager.close().await;
    }

    #[tokio::test]
    async fn fetch_service_get_address_balance_zcashd() {
        fetch_service_get_address_balance("zcashd").await;
    }

    async fn fetch_service_get_address_balance(validator: &str) {
        let mut test_manager = TestManager::launch(validator, None, None, true, true)
            .await
            .unwrap();

        let clients = test_manager
            .clients
            .as_ref()
            .expect("Clients are not initialized");
        let recipient_address = clients.get_recipient_address("transparent").await;

        clients.faucet.do_sync(true).await.unwrap();
        zingolib::testutils::lightclient::from_inputs::quick_send(
            &clients.faucet,
            vec![(recipient_address.as_str(), 250_000, None)],
        )
        .await
        .unwrap();
        test_manager.local_net.generate_blocks(1).await.unwrap();
        clients.recipient.do_sync(true).await.unwrap();
        let recipient_balance = clients.recipient.do_balance().await;

        let fetch_service = FetchService::spawn(FetchServiceConfig::new(
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

        let fetch_service_balance = fetch_service
            .z_get_address_balance(AddressStrings::new_valid(vec![recipient_address]).unwrap())
            .await
            .unwrap();

        dbg!(recipient_balance.clone());
        dbg!(fetch_service_balance.clone());

        assert_eq!(recipient_balance.transparent_balance.unwrap(), 250_000,);
        assert_eq!(
            recipient_balance.transparent_balance.unwrap(),
            fetch_service_balance.balance,
        );

        test_manager.close().await;
    }

    #[tokio::test]
    async fn fetch_service_get_block_raw_zcashd() {
        fetch_service_get_block_raw("zcashd").await;
    }

    async fn fetch_service_get_block_raw(validator: &str) {
        let mut test_manager = TestManager::launch(validator, None, None, false, false)
            .await
            .unwrap();

        let fetch_service = FetchService::spawn(FetchServiceConfig::new(
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

        dbg!(fetch_service
            .z_get_block("1".to_string(), Some(0))
            .await
            .unwrap());

        test_manager.close().await;
    }

    #[tokio::test]
    async fn fetch_service_get_block_object_zcashd() {
        fetch_service_get_block_object("zcashd").await;
    }

    async fn fetch_service_get_block_object(validator: &str) {
        let mut test_manager = TestManager::launch(validator, None, None, false, false)
            .await
            .unwrap();

        let fetch_service = FetchService::spawn(FetchServiceConfig::new(
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

        dbg!(fetch_service
            .z_get_block("1".to_string(), Some(1))
            .await
            .unwrap());

        test_manager.close().await;
    }

    #[tokio::test]
    async fn fetch_service_get_raw_mempool_zcashd() {
        fetch_service_get_raw_mempool("zcashd").await;
    }

    async fn fetch_service_get_raw_mempool(validator: &str) {
        let mut test_manager = TestManager::launch(validator, None, None, true, true)
            .await
            .unwrap();

        let clients = test_manager
            .clients
            .as_ref()
            .expect("Clients are not initialized");

        test_manager.local_net.generate_blocks(1).await.unwrap();
        clients.faucet.do_sync(true).await.unwrap();

        let tx_1 = zingolib::testutils::lightclient::from_inputs::quick_send(
            &clients.faucet,
            vec![(
                &clients.get_recipient_address("transparent").await,
                250_000,
                None,
            )],
        )
        .await
        .unwrap();
        let tx_2 = zingolib::testutils::lightclient::from_inputs::quick_send(
            &clients.faucet,
            vec![(
                &clients.get_recipient_address("unified").await,
                250_000,
                None,
            )],
        )
        .await
        .unwrap();

        let fetch_service = FetchService::spawn(FetchServiceConfig::new(
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

        let fetch_service_mempool = fetch_service.get_raw_mempool().await.unwrap();

        dbg!(&tx_1);
        dbg!(&tx_2);
        dbg!(&fetch_service_mempool);

        assert_eq!(tx_1.first().to_string(), fetch_service_mempool[0]);
        assert_eq!(tx_2.first().to_string(), fetch_service_mempool[1]);

        test_manager.close().await;
    }

    #[tokio::test]
    async fn fetch_service_z_get_treestate_zcashd() {
        fetch_service_z_get_treestate("zcashd").await;
    }

    async fn fetch_service_z_get_treestate(validator: &str) {
        let mut test_manager = TestManager::launch(validator, None, None, true, true)
            .await
            .unwrap();

        let clients = test_manager
            .clients
            .as_ref()
            .expect("Clients are not initialized");

        clients.faucet.do_sync(true).await.unwrap();
        zingolib::testutils::lightclient::from_inputs::quick_send(
            &clients.faucet,
            vec![(
                &clients.get_recipient_address("unified").await,
                250_000,
                None,
            )],
        )
        .await
        .unwrap();

        test_manager.local_net.generate_blocks(1).await.unwrap();

        let fetch_service = FetchService::spawn(FetchServiceConfig::new(
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

        dbg!(fetch_service
            .z_get_treestate("2".to_string())
            .await
            .unwrap());

        test_manager.close().await;
    }

    #[tokio::test]
    async fn fetch_service_z_get_subtrees_by_index_zcashd() {
        fetch_service_z_get_subtrees_by_index("zcashd").await;
    }

    async fn fetch_service_z_get_subtrees_by_index(validator: &str) {
        let mut test_manager = TestManager::launch(validator, None, None, true, true)
            .await
            .unwrap();

        let clients = test_manager
            .clients
            .as_ref()
            .expect("Clients are not initialized");

        clients.faucet.do_sync(true).await.unwrap();
        zingolib::testutils::lightclient::from_inputs::quick_send(
            &clients.faucet,
            vec![(
                &clients.get_recipient_address("unified").await,
                250_000,
                None,
            )],
        )
        .await
        .unwrap();

        test_manager.local_net.generate_blocks(1).await.unwrap();

        let fetch_service = FetchService::spawn(FetchServiceConfig::new(
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

        dbg!(fetch_service
            .z_get_subtrees_by_index("orchard".to_string(), NoteCommitmentSubtreeIndex(0), None)
            .await
            .unwrap());

        test_manager.close().await;
    }

    #[tokio::test]
    async fn fetch_service_get_raw_transaction_zcashd() {
        fetch_service_get_raw_transaction("zcashd").await;
    }

    async fn fetch_service_get_raw_transaction(validator: &str) {
        let mut test_manager = TestManager::launch(validator, None, None, true, true)
            .await
            .unwrap();

        let clients = test_manager
            .clients
            .as_ref()
            .expect("Clients are not initialized");

        clients.faucet.do_sync(true).await.unwrap();
        let tx = zingolib::testutils::lightclient::from_inputs::quick_send(
            &clients.faucet,
            vec![(
                &clients.get_recipient_address("unified").await,
                250_000,
                None,
            )],
        )
        .await
        .unwrap();

        test_manager.local_net.generate_blocks(1).await.unwrap();

        let fetch_service = FetchService::spawn(FetchServiceConfig::new(
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

        dbg!(fetch_service
            .get_raw_transaction(tx.first().to_string(), Some(1))
            .await
            .unwrap());

        test_manager.close().await;
    }

    #[tokio::test]
    async fn fetch_service_get_address_tx_ids_zcashd() {
        fetch_service_get_address_tx_ids("zcashd").await;
    }

    async fn fetch_service_get_address_tx_ids(validator: &str) {
        let mut test_manager = TestManager::launch(validator, None, None, true, true)
            .await
            .unwrap();

        let clients = test_manager
            .clients
            .as_ref()
            .expect("Clients are not initialized");
        let recipient_address = clients.get_recipient_address("transparent").await;

        clients.faucet.do_sync(true).await.unwrap();
        let tx = zingolib::testutils::lightclient::from_inputs::quick_send(
            &clients.faucet,
            vec![(recipient_address.as_str(), 250_000, None)],
        )
        .await
        .unwrap();
        test_manager.local_net.generate_blocks(1).await.unwrap();

        let fetch_service = FetchService::spawn(FetchServiceConfig::new(
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

        let fetch_service_txids = fetch_service
            .get_address_tx_ids(GetAddressTxIdsRequest::from_parts(
                vec![recipient_address],
                0,
                2,
            ))
            .await
            .unwrap();

        dbg!(&tx);
        dbg!(&fetch_service_txids);
        assert_eq!(tx.first().to_string(), fetch_service_txids[0]);

        test_manager.close().await;
    }

    #[tokio::test]
    async fn fetch_service_get_address_utxos_zcashd() {
        fetch_service_get_address_utxos("zcashd").await;
    }

    async fn fetch_service_get_address_utxos(validator: &str) {
        let mut test_manager = TestManager::launch(validator, None, None, true, true)
            .await
            .unwrap();

        let clients = test_manager
            .clients
            .as_ref()
            .expect("Clients are not initialized");
        let recipient_address = clients.get_recipient_address("transparent").await;

        clients.faucet.do_sync(true).await.unwrap();
        let txid_1 = zingolib::testutils::lightclient::from_inputs::quick_send(
            &clients.faucet,
            vec![(recipient_address.as_str(), 250_000, None)],
        )
        .await
        .unwrap();
        test_manager.local_net.generate_blocks(1).await.unwrap();
        clients.faucet.do_sync(true).await.unwrap();

        let fetch_service = FetchService::spawn(FetchServiceConfig::new(
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

        let fetch_service_utxos = fetch_service
            .z_get_address_utxos(AddressStrings::new_valid(vec![recipient_address]).unwrap())
            .await
            .unwrap();
        let (_, fetch_service_txid, ..) = fetch_service_utxos[0].into_parts();

        dbg!(&txid_1);
        dbg!(&fetch_service_utxos);
        assert_eq!(txid_1.first().to_string(), fetch_service_txid.to_string());

        test_manager.close().await;
    }
}
