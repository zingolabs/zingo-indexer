//! Lightwallet service RPC implementations.

use futures::StreamExt;

use crate::rpc::GrpcClient;
use zaino_proto::proto::{
    compact_formats::CompactBlock,
    service::{
        compact_tx_streamer_server::CompactTxStreamer, Address, AddressList, Balance, BlockId,
        BlockRange, ChainSpec, Duration, Empty, Exclude, GetAddressUtxosArg,
        GetAddressUtxosReplyList, GetSubtreeRootsArg, LightdInfo, PingResponse, RawTransaction,
        SendResponse, TransparentAddressBlockFilter, TreeState, TxFilter,
    },
};
use zaino_state::{
    indexer::LightWalletIndexer,
    stream::{
        AddressStream, CompactBlockStream, CompactTransactionStream, RawTransactionStream,
        SubtreeRootReplyStream, UtxoReplyStream,
    },
};

impl CompactTxStreamer for GrpcClient {
    /// Return the height of the tip of the best chain.
    fn get_latest_block<'life0, 'async_trait>(
        &'life0 self,
        _request: tonic::Request<ChainSpec>,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<
                    Output = std::result::Result<tonic::Response<BlockId>, tonic::Status>,
                > + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        println!("[TEST] Received call of get_latest_block.");
        Box::pin(async {
            Ok(tonic::Response::new(
                self.service_subscriber
                    .inner_ref()
                    .get_latest_block()
                    .await?,
            ))
        })
    }

    /// Return the compact block corresponding to the given block identifier.
    fn get_block<'life0, 'async_trait>(
        &'life0 self,
        request: tonic::Request<BlockId>,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<
                    Output = std::result::Result<tonic::Response<CompactBlock>, tonic::Status>,
                > + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        println!("[TEST] Received call of get_block.");
        Box::pin(async {
            Ok(tonic::Response::new(
                self.service_subscriber
                    .inner_ref()
                    .get_block(request.into_inner())
                    .await?,
            ))
        })
    }

    /// Same as GetBlock except actions contain only nullifiers.
    fn get_block_nullifiers<'life0, 'async_trait>(
        &'life0 self,
        request: tonic::Request<BlockId>,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<
                    Output = std::result::Result<tonic::Response<CompactBlock>, tonic::Status>,
                > + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        println!("[TEST] Received call of get_block_nullifiers.");
        Box::pin(async {
            Ok(tonic::Response::new(
                self.service_subscriber
                    .inner_ref()
                    .get_block_nullifiers(request.into_inner())
                    .await?,
            ))
        })
    }

    /// Server streaming response type for the GetBlockRange method.
    #[doc = "Server streaming response type for the GetBlockRange method."]
    type GetBlockRangeStream = std::pin::Pin<Box<CompactBlockStream>>;

    /// Return a list of consecutive compact blocks.
    // #[cfg(not(feature = "state_service"))]
    fn get_block_range<'life0, 'async_trait>(
        &'life0 self,
        request: tonic::Request<BlockRange>,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<
                    Output = std::result::Result<
                        tonic::Response<Self::GetBlockRangeStream>,
                        tonic::Status,
                    >,
                > + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        println!("[TEST] Received call of get_block_range.");
        Box::pin(async move {
            Ok(tonic::Response::new(Box::pin(
                self.service_subscriber
                    .inner_ref()
                    .get_block_range(request.into_inner())
                    .await?,
            )))
        })
    }

    /// Server streaming response type for the GetBlockRangeNullifiers method.
    #[doc = " Server streaming response type for the GetBlockRangeNullifiers method."]
    type GetBlockRangeNullifiersStream = std::pin::Pin<Box<CompactBlockStream>>;

    /// Same as GetBlockRange except actions contain only nullifiers.
    fn get_block_range_nullifiers<'life0, 'async_trait>(
        &'life0 self,
        request: tonic::Request<BlockRange>,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<
                    Output = std::result::Result<
                        tonic::Response<Self::GetBlockRangeNullifiersStream>,
                        tonic::Status,
                    >,
                > + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        println!("[TEST] Received call of get_block_range_nullifiers.");
        Box::pin(async move {
            Ok(tonic::Response::new(Box::pin(
                self.service_subscriber
                    .inner_ref()
                    .get_block_range_nullifiers(request.into_inner())
                    .await?,
            )))
        })
    }

    /// Return the requested full (not compact) transaction (as from zcashd).
    fn get_transaction<'life0, 'async_trait>(
        &'life0 self,
        request: tonic::Request<TxFilter>,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<
                    Output = std::result::Result<tonic::Response<RawTransaction>, tonic::Status>,
                > + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        println!("[TEST] Received call of get_transaction.");
        Box::pin(async {
            Ok(tonic::Response::new(
                self.service_subscriber
                    .inner_ref()
                    .get_transaction(request.into_inner())
                    .await?,
            ))
        })
    }

    /// Submit the given transaction to the Zcash network.
    fn send_transaction<'life0, 'async_trait>(
        &'life0 self,
        request: tonic::Request<RawTransaction>,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<
                    Output = std::result::Result<tonic::Response<SendResponse>, tonic::Status>,
                > + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        println!("[TEST] Received call of send_transaction.");
        Box::pin(async {
            Ok(tonic::Response::new(
                self.service_subscriber
                    .inner_ref()
                    .send_transaction(request.into_inner())
                    .await?,
            ))
        })
    }

    /// Server streaming response type for the GetTaddressTxids method.
    #[doc = "Server streaming response type for the GetTaddressTxids method."]
    type GetTaddressTxidsStream = std::pin::Pin<Box<RawTransactionStream>>;

    /// This name is misleading, returns the full transactions that have either inputs or outputs connected to the given transparent address.
    fn get_taddress_txids<'life0, 'async_trait>(
        &'life0 self,
        request: tonic::Request<TransparentAddressBlockFilter>,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<
                    Output = std::result::Result<
                        tonic::Response<Self::GetTaddressTxidsStream>,
                        tonic::Status,
                    >,
                > + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        println!("[TEST] Received call of get_taddress_txids.");
        Box::pin(async move {
            Ok(tonic::Response::new(Box::pin(
                self.service_subscriber
                    .inner_ref()
                    .get_taddress_txids(request.into_inner())
                    .await?,
            )))
        })
    }

    /// Returns the total balance for a list of taddrs
    fn get_taddress_balance<'life0, 'async_trait>(
        &'life0 self,
        request: tonic::Request<AddressList>,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<
                    Output = std::result::Result<tonic::Response<Balance>, tonic::Status>,
                > + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        println!("[TEST] Received call of get_taddress_balance.");
        Box::pin(async {
            Ok(tonic::Response::new(
                self.service_subscriber
                    .inner_ref()
                    .get_taddress_balance(request.into_inner())
                    .await?,
            ))
        })
    }

    /// Returns the total balance for a list of taddrs
    #[must_use]
    #[allow(clippy::type_complexity, clippy::type_repetition_in_bounds)]
    fn get_taddress_balance_stream<'life0, 'async_trait>(
        &'life0 self,
        request: tonic::Request<tonic::Streaming<Address>>,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = Result<tonic::Response<Balance>, tonic::Status>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        println!("[TEST] Received call of get_taddress_balance_stream.");
        Box::pin(async {
            let (channel_tx, channel_rx) =
                tokio::sync::mpsc::channel::<Result<Address, tonic::Status>>(32);
            let mut request_stream = request.into_inner();

            tokio::spawn(async move {
                while let Some(address_result) = request_stream.next().await {
                    if channel_tx.send(address_result).await.is_err() {
                        break;
                    }
                }
                drop(channel_tx);
            });
            let address_stream = AddressStream::new(channel_rx);

            Ok(tonic::Response::new(
                self.service_subscriber
                    .inner_ref()
                    .get_taddress_balance_stream(address_stream)
                    .await?,
            ))
        })
    }

    /// Server streaming response type for the GetMempoolTx method.
    #[doc = "Server streaming response type for the GetMempoolTx method."]
    type GetMempoolTxStream = std::pin::Pin<Box<CompactTransactionStream>>;

    /// Return the compact transactions currently in the mempool; the results
    /// can be a few seconds out of date. If the Exclude list is empty, return
    /// all transactions; otherwise return all *except* those in the Exclude list
    /// (if any); this allows the client to avoid receiving transactions that it
    /// already has (from an earlier call to this rpc). The transaction IDs in the
    /// Exclude list can be shortened to any number of bytes to make the request
    /// more bandwidth-efficient; if two or more transactions in the mempool
    /// match a shortened txid, they are all sent (none is excluded). Transactions
    /// in the exclude list that don't exist in the mempool are ignored.
    fn get_mempool_tx<'life0, 'async_trait>(
        &'life0 self,
        request: tonic::Request<Exclude>,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<
                    Output = std::result::Result<
                        tonic::Response<Self::GetMempoolTxStream>,
                        tonic::Status,
                    >,
                > + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        println!("[TEST] Received call of get_mempool_tx.");
        Box::pin(async {
            Ok(tonic::Response::new(Box::pin(
                self.service_subscriber
                    .inner_ref()
                    .get_mempool_tx(request.into_inner())
                    .await?,
            )))
        })
    }

    /// Server streaming response type for the GetMempoolStream method.
    #[doc = "Server streaming response type for the GetMempoolStream method."]
    type GetMempoolStreamStream = std::pin::Pin<Box<RawTransactionStream>>;

    /// Return a stream of current Mempool transactions. This will keep the output stream open while
    /// there are mempool transactions. It will close the returned stream when a new block is mined.
    fn get_mempool_stream<'life0, 'async_trait>(
        &'life0 self,
        _request: tonic::Request<Empty>,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<
                    Output = std::result::Result<
                        tonic::Response<Self::GetMempoolStreamStream>,
                        tonic::Status,
                    >,
                > + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        println!("[TEST] Received call of get_mempool_stream.");
        Box::pin(async {
            Ok(tonic::Response::new(Box::pin(
                self.service_subscriber
                    .inner_ref()
                    .get_mempool_stream()
                    .await?,
            )))
        })
    }

    /// GetTreeState returns the note commitment tree state corresponding to the given block.
    /// See section 3.7 of the Zcash protocol specification. It returns several other useful
    /// values also (even though they can be obtained using GetBlock).
    /// The block can be specified by either height or hash.
    fn get_tree_state<'life0, 'async_trait>(
        &'life0 self,
        request: tonic::Request<BlockId>,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<
                    Output = std::result::Result<tonic::Response<TreeState>, tonic::Status>,
                > + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        println!("[TEST] Received call of get_tree_state.");
        Box::pin(async {
            Ok(tonic::Response::new(
                self.service_subscriber
                    .inner_ref()
                    .get_tree_state(request.into_inner())
                    .await?,
            ))
        })
    }

    /// GetLatestTreeState returns the note commitment tree state corresponding to the chain tip.
    fn get_latest_tree_state<'life0, 'async_trait>(
        &'life0 self,
        _request: tonic::Request<Empty>,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<
                    Output = std::result::Result<tonic::Response<TreeState>, tonic::Status>,
                > + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        println!("[TEST] Received call of get_latest_tree_state.");
        Box::pin(async {
            Ok(tonic::Response::new(
                self.service_subscriber
                    .inner_ref()
                    .get_latest_tree_state()
                    .await?,
            ))
        })
    }

    /// Server streaming response type for the GetSubtreeRoots method.
    #[doc = " Server streaming response type for the GetSubtreeRoots method."]
    type GetSubtreeRootsStream = std::pin::Pin<Box<SubtreeRootReplyStream>>;

    /// Returns a stream of information about roots of subtrees of the Sapling and Orchard
    /// note commitment trees.
    fn get_subtree_roots<'life0, 'async_trait>(
        &'life0 self,
        request: tonic::Request<GetSubtreeRootsArg>,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<
                    Output = std::result::Result<
                        tonic::Response<Self::GetSubtreeRootsStream>,
                        tonic::Status,
                    >,
                > + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        println!("[TEST] Received call of get_subtree_roots.");
        Box::pin(async move {
            Ok(tonic::Response::new(Box::pin(
                self.service_subscriber
                    .inner_ref()
                    .get_subtree_roots(request.into_inner())
                    .await?,
            )))
        })
    }

    /// Returns all unspent outputs for a list of addresses.
    ///
    /// Ignores all utxos below block height [GetAddressUtxosArg.start_height].
    /// Returns max [GetAddressUtxosArg.max_entries] utxos, or unrestricted if [GetAddressUtxosArg.max_entries] = 0.
    /// Utxos are collected and returned as a single Vec.
    fn get_address_utxos<'life0, 'async_trait>(
        &'life0 self,
        request: tonic::Request<GetAddressUtxosArg>,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<
                    Output = std::result::Result<
                        tonic::Response<GetAddressUtxosReplyList>,
                        tonic::Status,
                    >,
                > + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        println!("[TEST] Received call of get_address_utxos.");
        Box::pin(async {
            Ok(tonic::Response::new(
                self.service_subscriber
                    .inner_ref()
                    .get_address_utxos(request.into_inner())
                    .await?,
            ))
        })
    }

    /// Server streaming response type for the GetAddressUtxosStream method.
    #[doc = "Server streaming response type for the GetAddressUtxosStream method."]
    type GetAddressUtxosStreamStream = std::pin::Pin<Box<UtxoReplyStream>>;

    /// Returns all unspent outputs for a list of addresses.
    ///
    /// Ignores all utxos below block height [GetAddressUtxosArg.start_height].
    /// Returns max [GetAddressUtxosArg.max_entries] utxos, or unrestricted if [GetAddressUtxosArg.max_entries] = 0.
    /// Utxos are returned in a stream.
    fn get_address_utxos_stream<'life0, 'async_trait>(
        &'life0 self,
        request: tonic::Request<GetAddressUtxosArg>,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<
                    Output = std::result::Result<
                        tonic::Response<Self::GetAddressUtxosStreamStream>,
                        tonic::Status,
                    >,
                > + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        println!("[TEST] Received call of get_address_utxos_stream.");
        Box::pin(async {
            Ok(tonic::Response::new(Box::pin(
                self.service_subscriber
                    .inner_ref()
                    .get_address_utxos_stream(request.into_inner())
                    .await?,
            )))
        })
    }

    /// Return information about this lightwalletd instance and the blockchain
    fn get_lightd_info<'life0, 'async_trait>(
        &'life0 self,
        _request: tonic::Request<Empty>,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<
                    Output = std::result::Result<tonic::Response<LightdInfo>, tonic::Status>,
                > + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        println!("[TEST] Received call of get_lightd_info.");
        Box::pin(async {
            Ok(tonic::Response::new(
                self.service_subscriber
                    .inner_ref()
                    .get_lightd_info()
                    .await?,
            ))
        })
    }

    /// Testing-only, requires lightwalletd --ping-very-insecure (do not enable in production) [from zebrad]
    /// This RPC has not been implemented as it is not currently used by zingolib.
    /// If you require this RPC please open an issue or PR at the Zingo-Indexer github (https://github.com/zingolabs/zingo-indexer).
    fn ping<'life0, 'async_trait>(
        &'life0 self,
        request: tonic::Request<Duration>,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<
                    Output = std::result::Result<tonic::Response<PingResponse>, tonic::Status>,
                > + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        println!("[TEST] Received call of ping.");
        Box::pin(async {
            Ok(tonic::Response::new(
                self.service_subscriber
                    .inner_ref()
                    .ping(request.into_inner())
                    .await?,
            ))
        })
    }
}
