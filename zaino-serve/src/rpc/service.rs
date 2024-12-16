//! Lightwallet service RPC implementations.

use futures::StreamExt;
use hex::FromHex;
use tokio::time::timeout;

use zaino_state::stream::{CompactBlockStream, CompactTransactionStream, RawTransactionStream, SubtreeRootReplyStream, UtxoReplyStream};
use crate::{rpc::GrpcClient, utils::get_build_info};
use zaino_fetch::{
    chain::{
        block::{get_block_from_node, get_nullifiers_from_node},
        mempool::Mempool,
        transaction::FullTransaction,
        utils::ParseFromSlice,
    },
    jsonrpc::{connector::JsonRpcConnector, response::{GetBlockResponse, GetTransactionResponse}},
};
use zaino_proto::proto::{
    compact_formats::CompactBlock,
    service::{
        compact_tx_streamer_server::CompactTxStreamer, Address, AddressList, Balance, BlockId, 
        BlockRange, ChainSpec, Duration, Empty, Exclude, GetAddressUtxosArg, GetAddressUtxosReply, 
        GetAddressUtxosReplyList, GetSubtreeRootsArg, LightdInfo, PingResponse, RawTransaction, 
        SendResponse, ShieldedProtocol, SubtreeRoot, TransparentAddressBlockFilter, TreeState, 
        TxFilter
    },
};

/// T Address Regex
static TADDR_REGEX: lazy_regex::Lazy<lazy_regex::regex::Regex> =
    lazy_regex::lazy_regex!(r"^t[a-zA-Z0-9]{34}$");

/// Checks for valid t Address.
///
/// Returns Some(taddress) if address is valid else none.
fn check_taddress(taddr: &str) -> Option<&str> {
    if TADDR_REGEX.is_match(taddr) {
        Some(taddr)
    } else {
        None
    }
}

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
            let blockchain_info = JsonRpcConnector::new(
                self.zebrad_rpc_uri.clone(),
                Some("xxxxxx".to_string()),
                Some("xxxxxx".to_string()),
            )
            .await?
            .get_blockchain_info()
            .await
            .map_err(|e| e.to_grpc_status())?;

            let block_id = BlockId {
                height: blockchain_info.blocks.0 as u64,
                hash: blockchain_info
                    .best_block_hash
                    .bytes_in_display_order()
                    .to_vec(),
            };

            Ok(tonic::Response::new(block_id))
        })
    }

    /// Return the compact block corresponding to the given block identifier.
    ///
    /// TODO: This implementation is slow. An internal block cache should be implemented that this rpc, along with the get_block rpc, can rely on.
    ///       - add get_block function that queries the block cache / internal state for block and calls get_block_from_node to fetch block if not present.
    ///       - use chain height held in internal state to validate block height being requested.
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
            let zebrad_uri = self.zebrad_rpc_uri.clone();
            let height: u32 = match request.into_inner().height.try_into() {
                Ok(height) => height,
                Err(_) => {
                    return Err(tonic::Status::invalid_argument(
                        "Error: Height out of range. Failed to convert to u32.",
                    ));
                }
            };
            match get_block_from_node(&zebrad_uri, &height).await {
                Ok(block) => Ok(tonic::Response::new(block)),
                Err(e) => {
                    let chain_height = JsonRpcConnector::new(
                        self.zebrad_rpc_uri.clone(),
                        Some("xxxxxx".to_string()),
                        Some("xxxxxx".to_string()),
                    )
                    .await?
                    .get_blockchain_info()
                    .await
                    .map_err(|e| e.to_grpc_status())?
                    .blocks
                    .0;
                    if height >= chain_height {
                        Err(tonic::Status::out_of_range(
                            format!(
                                "Error: Height out of range [{}]. Height requested is greater than the best chain tip [{}].",
                                height, chain_height,
                            )
                        ))
                    } else {
                        // TODO: Hide server error from clients before release. Currently useful for dev purposes.
                        Err(tonic::Status::unknown(format!(
                            "Error: Failed to retrieve block from node. Server Error: {}",
                            e,
                        )))
                    }
                }
            }
        })
    }

    /// Same as GetBlock except actions contain only nullifiers.
    ///
    /// NOTE: This should be reimplemented with the introduction of the BlockCache.
    ///       - use chain height held in internal state to validate block height being requested.
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
            let zebrad_uri = self.zebrad_rpc_uri.clone();
            let height: u32 = match request.into_inner().height.try_into() {
                Ok(height) => height,
                Err(_) => {
                    return Err(tonic::Status::invalid_argument(
                        "Error: Height out of range. Failed to convert to u32.",
                    ));
                }
            };
            match get_nullifiers_from_node(&zebrad_uri, &height).await {
                Ok(block) => Ok(tonic::Response::new(block)),
                Err(e) => {
                    let chain_height = JsonRpcConnector::new(
                        self.zebrad_rpc_uri.clone(),
                        Some("xxxxxx".to_string()),
                        Some("xxxxxx".to_string()),
                    )
                    .await?
                    .get_blockchain_info()
                    .await
                    .map_err(|e| e.to_grpc_status())?
                    .blocks
                    .0;
                    if height >= chain_height {
                        Err(tonic::Status::out_of_range(
                            format!(
                                "Error: Height out of range [{}]. Height requested is greater than the best chain tip [{}].",
                                height, chain_height,
                            )
                        ))
                    } else {
                        // TODO: Hide server error from clients before release. Currently useful for dev purposes.
                        Err(tonic::Status::unknown(format!(
                            "Error: Failed to retrieve nullifiers from node. Server Error: {}",
                            e,
                        )))
                    }
                }
            }
        })
    }

    /// Server streaming response type for the GetBlockRange method.
    #[doc = "Server streaming response type for the GetBlockRange method."]
    type GetBlockRangeStream = std::pin::Pin<Box<CompactBlockStream>>;

    /// Return a list of consecutive compact blocks.
    ///
    /// TODO: This implementation is slow. An internal block cache should be implemented that this rpc, along with the get_block rpc, can rely on.
    ///       - add get_block function that queries the block cache for block and calls get_block_from_node to fetch block if not present.
    ///       - use chain height held in internal state to validate block height being requested.
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
        let zebrad_uri = self.zebrad_rpc_uri.clone();
        Box::pin(async move {
            let blockrange = request.into_inner();
            let mut start: u32 = match blockrange.start {
                Some(block_id) => match block_id.height.try_into() {
                    Ok(height) => height,
                    Err(_) => {
                        return Err(tonic::Status::invalid_argument(
                            "Error: Start height out of range. Failed to convert to u32.",
                        ));
                    }
                },
                None => {
                    return Err(tonic::Status::invalid_argument(
                        "Error: No start height given.",
                    ));
                }
            };
            let mut end: u32 = match blockrange.end {
                Some(block_id) => match block_id.height.try_into() {
                    Ok(height) => height,
                    Err(_) => {
                        return Err(tonic::Status::invalid_argument(
                            "Error: End height out of range. Failed to convert to u32.",
                        ));
                    }
                },
                None => {
                    return Err(tonic::Status::invalid_argument(
                        "Error: No start height given.",
                    ));
                }
            };
            let rev_order = if start > end {
                (start, end) = (end, start);
                true
            } else {
                false
            };
            let chain_height = JsonRpcConnector::new(
                self.zebrad_rpc_uri.clone(),
                Some("xxxxxx".to_string()),
                Some("xxxxxx".to_string()),
            )
            .await?
            .get_blockchain_info()
            .await
            .map_err(|e| e.to_grpc_status())?
            .blocks
            .0;
            println!("[TEST] Fetching blocks in range: {}-{}.", start, end);
            let (channel_tx, channel_rx) = tokio::sync::mpsc::channel(32);
            tokio::spawn(async move {
                // NOTE: This timeout is so slow due to the blockcache not being implemented. This should be reduced to 30s once functionality is in place.
                // TODO: Make [rpc_timout] a configurable system variable with [default = 30s] and [mempool_rpc_timout = 4*rpc_timeout]
                let timeout = timeout(std::time::Duration::from_secs(120), async {
                    for height in start..=end {
                        let height = if rev_order {
                            end - (height - start)
                        } else {
                            height
                        };
                        println!("[TEST] Fetching block at height: {}.", height);
                        match get_block_from_node(&zebrad_uri, &height).await {
                            Ok(block) => {
                                if channel_tx.send(Ok(block)).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
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
                })
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
            let output_stream = CompactBlockStream::new(channel_rx);
            let stream_boxed = Box::pin(output_stream);
            Ok(tonic::Response::new(stream_boxed))
        })
    }

    /// Server streaming response type for the GetBlockRangeNullifiers method.
    #[doc = " Server streaming response type for the GetBlockRangeNullifiers method."]
    type GetBlockRangeNullifiersStream = std::pin::Pin<Box<CompactBlockStream>>;

    /// Same as GetBlockRange except actions contain only nullifiers.
    ///
    /// NOTE: This should be reimplemented with the introduction of the BlockCache.
    ///       - use chain height held in internal state to validate block height being requested.
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
        let zebrad_uri = self.zebrad_rpc_uri.clone();
        Box::pin(async move {
            let blockrange = request.into_inner();
            let mut start: u32 = match blockrange.start {
                Some(block_id) => match block_id.height.try_into() {
                    Ok(height) => height,
                    Err(_) => {
                        return Err(tonic::Status::invalid_argument(
                            "Error: Start height out of range. Failed to convert to u32.",
                        ));
                    }
                },
                None => {
                    return Err(tonic::Status::invalid_argument(
                        "Error: No start height given.",
                    ));
                }
            };
            let mut end: u32 = match blockrange.end {
                Some(block_id) => match block_id.height.try_into() {
                    Ok(height) => height,
                    Err(_) => {
                        return Err(tonic::Status::invalid_argument(
                            "Error: End height out of range. Failed to convert to u32.",
                        ));
                    }
                },
                None => {
                    return Err(tonic::Status::invalid_argument(
                        "Error: No end height given.",
                    ));
                }
            };
            let rev_order = if start > end {
                (start, end) = (end, start);
                true
            } else {
                false
            };
            let chain_height = JsonRpcConnector::new(
                self.zebrad_rpc_uri.clone(),
                Some("xxxxxx".to_string()),
                Some("xxxxxx".to_string()),
            )
            .await?
            .get_blockchain_info()
            .await
            .map_err(|e| e.to_grpc_status())?
            .blocks
            .0;
            let (channel_tx, channel_rx) = tokio::sync::mpsc::channel(32);
            tokio::spawn(async move {
                // NOTE: This timeout is so slow due to the blockcache not being implemented. This should be reduced to 30s once functionality is in place.
                // TODO: Make [rpc_timout] a configurable system variable with [default = 30s] and [mempool_rpc_timout = 4*rpc_timeout]
                let timeout = timeout(std::time::Duration::from_secs(120), async {
                    for height in start..=end {
                        let height = if rev_order {
                            end - (height - start)
                        } else {
                            height
                        };
                        let compact_block = get_nullifiers_from_node(&zebrad_uri, &height).await;
                        match compact_block {
                            Ok(block) => {
                                if channel_tx.send(Ok(block)).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
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
                })
                .await;
                match timeout {
                    Ok(_) => {}
                    Err(_) => {
                        channel_tx
                            .send(Err(tonic::Status::deadline_exceeded(
                                "Error: get_block_range_nullifiers gRPC request timed out.",
                            )))
                            .await
                            .ok();
                    }
                }
            });
            let output_stream = CompactBlockStream::new(channel_rx);
            let stream_boxed = Box::pin(output_stream);
            Ok(tonic::Response::new(stream_boxed))
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
            let hash = request.into_inner().hash;
            if hash.len() == 32 {
                let reversed_hash = hash.iter().rev().copied().collect::<Vec<u8>>();
                let hash_hex = hex::encode(reversed_hash);
                let tx = JsonRpcConnector::new(
                    self.zebrad_rpc_uri.clone(),
                    Some("xxxxxx".to_string()),
                    Some("xxxxxx".to_string()),
                )
                .await?
                .get_raw_transaction(hash_hex, Some(1))
                .await
                .map_err(|e| e.to_grpc_status())?;

                let (hex, height) = if let GetTransactionResponse::Object { hex, height, .. } = tx {
                    (hex, height)
                } else {
                    return Err(tonic::Status::not_found("Error: Transaction not received"));
                };
                let height: u64 = height.try_into().map_err(|_e| {
                    tonic::Status::unknown(
                        "Error: Invalid response from server - Height conversion failed",
                    )
                })?;

                Ok(tonic::Response::new(RawTransaction {
                    data: hex.as_ref().to_vec(),
                    height,
                }))
            } else {
                Err(tonic::Status::invalid_argument(
                    "Error: Transaction hash incorrect",
                ))
            }
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
            let hex_tx = hex::encode(request.into_inner().data);
            let tx_output = JsonRpcConnector::new(
                self.zebrad_rpc_uri.clone(),
                Some("xxxxxx".to_string()),
                Some("xxxxxx".to_string()),
            )
            .await?
            .send_raw_transaction(hex_tx)
            .await
            .map_err(|e| e.to_grpc_status())?;

            Ok(tonic::Response::new(SendResponse {
                error_code: 0,
                error_message: tx_output.0.to_string(),
            }))
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
            let zebrad_client = JsonRpcConnector::new(
                self.zebrad_rpc_uri.clone(),
                Some("xxxxxx".to_string()),
                Some("xxxxxx".to_string()),
            )
            .await?;
            let chain_height = zebrad_client.get_blockchain_info().await?.blocks.0;
            let block_filter = request.into_inner();
            let (start, end) =
                match block_filter.range {
                    Some(range) => match (range.start, range.end) {
                        (Some(start), Some(end)) => {
                            let start = match u32::try_from(start.height) {
                                Ok(height) => height.min(chain_height),
                                Err(_) => return Err(tonic::Status::invalid_argument(
                                    "Error: Start height out of range. Failed to convert to u32.",
                                )),
                            };
                            let end =
                                match u32::try_from(end.height) {
                                    Ok(height) => height.min(chain_height),
                                    Err(_) => return Err(tonic::Status::invalid_argument(
                                        "Error: End height out of range. Failed to convert to u32.",
                                    )),
                                };
                            if start > end {
                                (end, start)
                            } else {
                                (start, end)
                            }
                        }
                        _ => {
                            return Err(tonic::Status::invalid_argument(
                                "Error: Incomplete block range given.",
                            ))
                        }
                    },
                    None => {
                        return Err(tonic::Status::invalid_argument(
                            "Error: No block range given.",
                        ))
                    }
                };
            let txids = zebrad_client
                .get_address_txids(vec![block_filter.address], start, end)
                .await
                .map_err(|e| e.to_grpc_status())?;
            let (channel_tx, channel_rx) = tokio::sync::mpsc::channel(32);
            tokio::spawn(async move {
                // NOTE: This timeout is so slow due to the blockcache not being implemented. This should be reduced to 30s once functionality is in place.
                // TODO: Make [rpc_timout] a configurable system variable with [default = 30s] and [mempool_rpc_timout = 4*rpc_timeout]
                let timeout = timeout(std::time::Duration::from_secs(120), async {
                    for txid in txids.transactions {
                        let transaction = zebrad_client.get_raw_transaction(txid, Some(1)).await;
                        match transaction {
                            Ok(GetTransactionResponse::Object { hex, height, .. }) => {
                                if channel_tx
                                    .send(Ok(RawTransaction {
                                        data: hex.as_ref().to_vec(),
                                        height: height as u64,
                                    }))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Ok(GetTransactionResponse::Raw(_)) => {
                                if channel_tx
                                    .send(Err(tonic::Status::unknown(
                                    "Received raw transaction type, this should not be impossible.",
                                    )))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Err(e) => {
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
                })
                .await;
                match timeout {
                    Ok(_) => {}
                    Err(_) => {
                        channel_tx
                            .send(Err(tonic::Status::internal(
                                "Error: get_taddress_txids gRPC request timed out",
                            )))
                            .await
                            .ok();
                    }
                }
            });
            let output_stream = RawTransactionStream::new(channel_rx);
            let stream_boxed = Box::pin(output_stream);
            Ok(tonic::Response::new(stream_boxed))
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
            let zebrad_client = JsonRpcConnector::new(
                self.zebrad_rpc_uri.clone(),
                Some("xxxxxx".to_string()),
                Some("xxxxxx".to_string()),
            )
            .await?;
            let taddrs = request.into_inner().addresses;
            if !taddrs.iter().all(|taddr| check_taddress(taddr).is_some()) {
                return Err(tonic::Status::invalid_argument(
                    "Error: One or more invalid taddresses given.",
                ));
            }
            let balance = zebrad_client.get_address_balance(taddrs).await?;
            let checked_balance: i64 = match i64::try_from(balance.balance) {
                Ok(balance) => balance,
                Err(_) => {
                    return Err(tonic::Status::unknown(
                        "Error: Error converting balance from u64 to i64.",
                    ));
                }
            };
            Ok(tonic::Response::new(Balance {
                value_zat: checked_balance,
            }))
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
            let zebrad_client = JsonRpcConnector::new(
                self.zebrad_rpc_uri.clone(),
                Some("xxxxxx".to_string()),
                Some("xxxxxx".to_string()),
            )
            .await?;
            let (channel_tx, mut channel_rx) = tokio::sync::mpsc::channel::<String>(32);
            let fetcher_task_handle = tokio::spawn(async move {
                // NOTE: This timeout is so slow due to the blockcache not being implemented. This should be reduced to 30s once functionality is in place.
                // TODO: Make [rpc_timout] a configurable system variable with [default = 30s] and [mempool_rpc_timout = 4*rpc_timeout]
                let fetcher_timeout = timeout(std::time::Duration::from_secs(120), async {
                    let mut total_balance: u64 = 0;
                    loop {
                        match channel_rx.recv().await {
                            Some(taddr) => {
                                if check_taddress(taddr.as_str()).is_some() {
                                    let balance =
                                        zebrad_client.get_address_balance(vec![taddr]).await?;
                                    total_balance += balance.balance;
                                } else {
                                    return Err(tonic::Status::invalid_argument(
                                        "Error: One or more invalid taddresses given.",
                                    ));
                                }
                            }
                            None => {
                                return Ok(total_balance);
                            }
                        }
                    }
                })
                .await;
                match fetcher_timeout {
                    Ok(result) => result,
                    Err(_) => Err(tonic::Status::deadline_exceeded(
                        "Error: get_taddress_balance_stream request timed out.",
                    )),
                }
            });
            // NOTE: This timeout is so slow due to the blockcache not being implemented. This should be reduced to 30s once functionality is in place.
            // TODO: Make [rpc_timout] a configurable system variable with [default = 30s] and [mempool_rpc_timout = 4*rpc_timeout]
            let addr_recv_timeout = timeout(std::time::Duration::from_secs(120), async {
                let mut address_stream = request.into_inner();
                while let Some(address_result) = address_stream.next().await {
                    // TODO: Hide server error from clients before release. Currently useful for dev purposes.
                    let address = address_result.map_err(|e| {
                        tonic::Status::unknown(format!("Failed to read from stream: {}", e))
                    })?;
                    if channel_tx.send(address.address).await.is_err() {
                        // TODO: Hide server error from clients before release. Currently useful for dev purposes.
                        return Err(tonic::Status::unknown(
                            "Error: Failed to send address to balance task.",
                        ));
                    }
                }
                drop(channel_tx);
                Ok::<(), tonic::Status>(())
            })
            .await;
            match addr_recv_timeout {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    fetcher_task_handle.abort();
                    return Err(e);
                }
                Err(_) => {
                    fetcher_task_handle.abort();
                    return Err(tonic::Status::deadline_exceeded(
                        "Error: get_taddress_balance_stream request timed out in address loop.",
                    ));
                }
            }
            match fetcher_task_handle.await {
                Ok(Ok(total_balance)) => {
                    let checked_balance: i64 = match i64::try_from(total_balance) {
                        Ok(balance) => balance,
                        Err(_) => {
                            // TODO: Hide server error from clients before release. Currently useful for dev purposes.
                            return Err(tonic::Status::unknown(
                                "Error: Error converting balance from u64 to i64.",
                            ));
                        }
                    };
                    Ok(tonic::Response::new(Balance {
                        value_zat: checked_balance,
                    }))
                }
                Ok(Err(e)) => Err(e),
                // TODO: Hide server error from clients before release. Currently useful for dev purposes.
                Err(e) => Err(tonic::Status::unknown(format!(
                    "Fetcher Task failed: {}",
                    e
                ))),
            }
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
    ///
    /// NOTE: This implementation is slow and should be re-implemented with the addition of the internal mempool and blockcache.
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
            let zebrad_uri = self.zebrad_rpc_uri.clone();
            let zebrad_client = JsonRpcConnector::new(
                self.zebrad_rpc_uri.clone(),
                Some("xxxxxx".to_string()),
                Some("xxxxxx".to_string()),
            )
            .await?;
            let exclude_txids: Vec<String> = request
                .into_inner()
                .txid
                .iter()
                .map(|txid_bytes| {
                    let reversed_txid_bytes: Vec<u8> = txid_bytes.iter().cloned().rev().collect();
                    hex::encode(&reversed_txid_bytes)
                })
                .collect();
            let (channel_tx, channel_rx) = tokio::sync::mpsc::channel(32);
            tokio::spawn(async move {
                // NOTE: This timeout is so slow due to the blockcache not being implemented. This should be reduced to 30s once functionality is in place.
                // TODO: Make [rpc_timout] a configurable system variable with [default = 30s] and [mempool_rpc_timout = 4*rpc_timeout]
                let timeout = timeout(std::time::Duration::from_secs(480), async {
                    let mempool = Mempool::new();
                    if let Err(e) = mempool.update(&zebrad_uri).await {
                        channel_tx.send(Err(tonic::Status::unknown(e.to_string())))
                            .await
                            .ok();
                        return;
                    }
                    match mempool.get_filtered_mempool_txids(exclude_txids).await {
                        Ok(mempool_txids) => {
                            for txid in mempool_txids {
                                match zebrad_client
                                    .get_raw_transaction(txid.clone(), Some(0))
                                    .await {
                                    Ok(GetTransactionResponse::Object { .. }) => {
                                        if channel_tx
                                        .send(Err(tonic::Status::internal(
                                            "Error: Received transaction object type, this should not be impossible.",
                                        )))
                                        .await
                                        .is_err()
                                        {
                                            break;
                                        }

                                    }
                                    Ok(GetTransactionResponse::Raw(raw)) => {
                                        let txid_bytes = match hex::decode(txid) {
                                            Ok(bytes) => bytes,
                                            Err(e) => {
                                                if channel_tx
                                                    .send(Err(tonic::Status::unknown(e.to_string())))
                                                    .await
                                                    .is_err()
                                                {
                                                    break;
                                                } else {
                                                    continue;
                                                }
                                            }
                                        };
                                        match FullTransaction::parse_from_slice(raw.as_ref(), Some(vec!(txid_bytes)), None) {
                                            Ok(transaction) => {
                                                if !transaction.0.is_empty() {
                                                    // TODO: Hide server error from clients before release. Currently useful for dev purposes.
                                                    if channel_tx
                                                        .send(Err(tonic::Status::unknown("Error: ")))
                                                        .await
                                                        .is_err()
                                                    {
                                                        break;
                                                    }
                                                } else {
                                                    match transaction.1.to_compact(0) {
                                                        Ok(compact_tx) => {
                                                            if channel_tx
                                                                .send(Ok(compact_tx))
                                                                .await
                                                                .is_err()
                                                            {
                                                                break;
                                                            }
                                                        }
                                                        Err(e) => {
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
                                            Err(e) => {
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
                                    Err(e) => {
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
                        Err(e) => {
                            // TODO: Hide server error from clients before release. Currently useful for dev purposes.
                            if channel_tx
                            .send(Err(tonic::Status::unknown(e.to_string())))
                            .await
                            .is_err()
                            {
                            }
                        }
                    }
                })
                .await;
                match timeout {
                    Ok(_) => {
                    }
                    Err(_) => {
                        channel_tx
                            .send(Err(tonic::Status::deadline_exceeded(
                                "Error: get_mempool_stream gRPC request timed out",
                            )))
                            .await
                            .ok();
                    }
                }
            });
            let output_stream = CompactTransactionStream::new(channel_rx);
            let stream_boxed = Box::pin(output_stream);
            Ok(tonic::Response::new(stream_boxed))
        })
    }

    /// Server streaming response type for the GetMempoolStream method.
    #[doc = "Server streaming response type for the GetMempoolStream method."]
    type GetMempoolStreamStream = std::pin::Pin<Box<RawTransactionStream>>;

    /// Return a stream of current Mempool transactions. This will keep the output stream open while
    /// there are mempool transactions. It will close the returned stream when a new block is mined.
    ///
    /// TODO: This implementation is slow. Zingo-Indexer's blockcache state engine should keep its own internal mempool state.
    ///     - This RPC should query Zingo-Indexer's internal mempool state rather than creating its own mempool and directly querying zebrad.
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
            let zebrad_uri = self.zebrad_rpc_uri.clone();
            let zebrad_client = JsonRpcConnector::new(
                self.zebrad_rpc_uri.clone(),
                Some("xxxxxx".to_string()),
                Some("xxxxxx".to_string()),
            )
            .await?;
            let mempool_height = zebrad_client.get_blockchain_info().await?.blocks.0;
            let (channel_tx, channel_rx) = tokio::sync::mpsc::channel(32);
            tokio::spawn(async move {
                // NOTE: This timeout is so slow due to the blockcache not being implemented. This should be reduced to 30s once functionality is in place.
                // TODO: Make [rpc_timout] a configurable system variable with [default = 30s] and [mempool_rpc_timout = 4*rpc_timeout]
                let timeout = timeout(std::time::Duration::from_secs(480), async {
                    let mempool = Mempool::new();
                    if let Err(e) = mempool.update(&zebrad_uri).await {
                        // TODO: Hide server error from clients before release. Currently useful for dev purposes.
                        channel_tx.send(Err(tonic::Status::unknown(e.to_string())))
                            .await
                            .ok();
                        return;
                    }
                    let mut mined = false;
                    let mut txid_index: usize = 0;
                    while !mined {
                        match mempool.get_mempool_txids().await {
                            Ok(mempool_txids) => {
                                for txid in &mempool_txids[txid_index..] {
                                    match zebrad_client
                                        .get_raw_transaction(txid.clone(), Some(1))
                                        .await {
                                        Ok(GetTransactionResponse::Object { hex, height: _, .. }) => {
                                            txid_index += 1;
                                            if channel_tx
                                                .send(Ok(RawTransaction {
                                                    data: hex.as_ref().to_vec(),
                                                    height: mempool_height as u64,
                                                }))
                                                .await
                                                .is_err()
                                            {
                                                break;
                                            }
                                        }
                                        Ok(GetTransactionResponse::Raw(_)) => {
                                            if channel_tx
                                            .send(Err(tonic::Status::internal(
                                                "Error: Received raw transaction type, this should not be impossible.",
                                            )))
                                            .await
                                            .is_err()
                                        {
                                            break;
                                        }
                                        }
                                        Err(e) => {
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
                            Err(e) => {
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
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                        mined = match mempool.update(&zebrad_uri).await {
                            Ok(mined) => mined,
                            Err(e) => {
                                // TODO: Hide server error from clients before release. Currently useful for dev purposes.
                                channel_tx.send(Err(tonic::Status::unknown(e.to_string())))
                                    .await
                                    .ok();
                                break;
                            }
                        };
                    }
                })
                .await;
                match timeout {
                    Ok(_) => {
                    }
                    Err(_) => {
                        channel_tx
                            .send(Err(tonic::Status::deadline_exceeded(
                                "Error: get_mempool_stream gRPC request timed out",
                            )))
                            .await
                            .ok();
                    }
                }
            });
            let output_stream = RawTransactionStream::new(channel_rx);
            let stream_boxed = Box::pin(output_stream);
            Ok(tonic::Response::new(stream_boxed))
        })
    }

    /// GetTreeState returns the note commitment tree state corresponding to the given block.
    /// See section 3.7 of the Zcash protocol specification. It returns several other useful
    /// values also (even though they can be obtained using GetBlock).
    /// The block can be specified by either height or hash.
    ///
    /// TODO: This is slow. Chain, along with other blockchain info should be saved on startup and used here [blockcache?].
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
            let zebrad_client = JsonRpcConnector::new(
                self.zebrad_rpc_uri.clone(),
                Some("xxxxxx".to_string()),
                Some("xxxxxx".to_string()),
            )
            .await?;
            let chain_info = zebrad_client
                .get_blockchain_info()
                .await
                .map_err(|e| e.to_grpc_status())?;
            let block_id = request.into_inner();
            let hash_or_height = if block_id.height != 0 {
                match u32::try_from(block_id.height) {
                    Ok(height) => {
                        if height >= chain_info.blocks.0 {
                            return Err(tonic::Status::out_of_range(
                            format!(
                                "Error: Height out of range [{}]. Height requested is greater than the best chain tip [{}].",
                                height, chain_info.blocks.0,
                            )
                        ));
                        } else {
                            height.to_string()
                        }
                    }
                    Err(_) => {
                        return Err(tonic::Status::invalid_argument(
                            "Error: Height out of range. Failed to convert to u32.",
                        ));
                    }
                }
            } else {
                hex::encode(block_id.hash)
            };
            match zebrad_client.get_treestate(hash_or_height).await {
                Ok(state) => Ok(tonic::Response::new(TreeState {
                    network: chain_info.chain,
                    height: state.height as u64,
                    hash: state.hash.to_string(),
                    time: state.time,
                    sapling_tree: state.sapling.inner().inner().clone(),
                    orchard_tree: state.orchard.inner().inner().clone(),
                })),
                Err(e) => {
                    // TODO: Hide server error from clients before release. Currently useful for dev purposes.
                    Err(tonic::Status::unknown(format!(
                        "Error: Failed to retrieve treestate from node. Server Error: {}",
                        e,
                    )))
                }
            }
        })
    }

    /// GetLatestTreeState returns the note commitment tree state corresponding to the chain tip.
    ///
    /// TODO: This is slow. Chain, along with other blockchain info should be saved on startup and used here [blockcache?].
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
            let zebrad_client = JsonRpcConnector::new(
                self.zebrad_rpc_uri.clone(),
                Some("xxxxxx".to_string()),
                Some("xxxxxx".to_string()),
            )
            .await?;
            let chain_info = zebrad_client
                .get_blockchain_info()
                .await
                .map_err(|e| e.to_grpc_status())?;
            match zebrad_client
                .get_treestate(chain_info.blocks.0.to_string())
                .await
            {
                Ok(state) => Ok(tonic::Response::new(TreeState {
                    network: chain_info.chain,
                    height: state.height as u64,
                    hash: state.hash.to_string(),
                    time: state.time,
                    sapling_tree: state.sapling.inner().inner().clone(),
                    orchard_tree: state.orchard.inner().inner().clone(),
                })),
                Err(e) => {
                    // TODO: Hide server error from clients before release. Currently useful for dev purposes.
                    Err(tonic::Status::unknown(format!(
                        "Error: Failed to retrieve treestate from node. Server Error: {}",
                        e,
                    )))
                }
            }
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
            let zebrad_uri  =self.zebrad_rpc_uri.clone();
            let zebrad_client = JsonRpcConnector::new(
                zebrad_uri.clone(),
                Some("xxxxxx".to_string()),
                Some("xxxxxx".to_string()),
            )
            .await?;
            let subtree_roots_args = request.into_inner();
            let pool = match ShieldedProtocol::try_from(subtree_roots_args.shielded_protocol) {
                Ok(protocol) => protocol.as_str_name(),
                Err(_) => return Err(tonic::Status::invalid_argument("Error: Invalid shielded protocol value.")),
            };
            let start_index = match u16::try_from(subtree_roots_args.start_index) {
                Ok(value) => value,
                Err(_) => return Err(tonic::Status::invalid_argument("Error: start_index value exceeds u16 range.")),
            };
            let limit = if subtree_roots_args.max_entries == 0 {
                None
            } else {
                match u16::try_from(subtree_roots_args.max_entries) {
                    Ok(value) => Some(value),
                    Err(_) => return Err(tonic::Status::invalid_argument("Error: max_entries value exceeds u16 range.")),
                }
            };
            let subtrees = zebrad_client.get_subtrees_by_index(pool.to_string(), start_index, limit).await?;
            let (channel_tx, channel_rx) = tokio::sync::mpsc::channel(32);
            tokio::spawn(async move {
                // NOTE: This timeout is so slow due to the blockcache not being implemented. This should be reduced to 30s once functionality is in place.
                // TODO: Make [rpc_timout] a configurable system variable with [default = 30s] and [mempool_rpc_timout = 4*rpc_timeout]
                let timeout = timeout(std::time::Duration::from_secs(120), async {
                    for subtree in subtrees.subtrees {
                        match zebrad_client.get_block(subtree.end_height.0.to_string(), Some(1)).await {
                            Ok(GetBlockResponse::Object {
                                hash,
                                confirmations: _,
                                height,
                                time: _,
                                tx: _,
                                trees: _,
                            }) => {
                                let checked_height = match height {
                                    Some(h) => h.0 as u64,
                                    None => {
                                        match channel_tx
                                            .send(Err(tonic::Status::unknown("Error: No block height returned by node.")))
                                            .await
                                        {
                                            Ok(_) => break,
                                            Err(e) => {
                                                eprintln!("Error: Channel closed unexpectedly: {}", e);
                                                break;
                                            }
                                        }
                                    }
                                };
                                let checked_root_hash = match hex::decode(&subtree.root) {
                                    Ok(hash) => hash,
                                    Err(e) => {
                                        match channel_tx
                                            .send(Err(tonic::Status::unknown(format!("Error: Failed to hex decode root hash: {}.", 
                                                e
                                            ))))
                                            .await
                                        {
                                            Ok(_) => break,
                                            Err(e) => {
                                                eprintln!("Error: Channel closed unexpectedly: {}", e);
                                                break;
                                            }
                                        }
                                    }
                                };
                                if channel_tx.send(
                                    Ok(SubtreeRoot {
                                        root_hash: checked_root_hash,
                                        completing_block_hash: hash.0.bytes_in_display_order().to_vec(),
                                        completing_block_height: checked_height,
                                    })).await.is_err() 
                                {
                                    break;
                                }

                            }
                            Ok(GetBlockResponse::Raw(_)) => {
                                // TODO: Hide server error from clients before release. Currently useful for dev purposes.
                                if channel_tx
                                    .send(Err(tonic::Status::unknown("Error: Received raw block type, this should not be possible.")))
                                    .await
                                    .is_err()
                                    {
                                        break;
                                    }
                            }
                            Err(e) => {
                                // TODO: Hide server error from clients before release. Currently useful for dev purposes.
                                if channel_tx
                                    .send(Err(tonic::Status::unknown(format!("Error: Could not fetch block at height [{}] from node: {}", 
                                        subtree.end_height.0, 
                                        e
                                    ))))
                                    .await
                                    .is_err()
                                    {
                                        break;
                                    }
                            }
                        }
                    }
                })
                .await;
                match timeout {
                    Ok(_) => {
                    }
                    Err(_) => {
                        channel_tx
                            .send(Err(tonic::Status::deadline_exceeded(
                                "Error: get_mempool_stream gRPC request timed out",
                            )))
                            .await
                            .ok();
                    }
                }
            });
            let output_stream = SubtreeRootReplyStream::new(channel_rx);
            let stream_boxed = Box::pin(output_stream);
            Ok(tonic::Response::new(stream_boxed))
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
            let zebrad_client = JsonRpcConnector::new(
                self.zebrad_rpc_uri.clone(),
                Some("xxxxxx".to_string()),
                Some("xxxxxx".to_string()),
            )
            .await?;
            let addr_args = request.into_inner();
            if !addr_args
                .addresses
                .iter()
                .all(|taddr| check_taddress(taddr).is_some())
            {
                return Err(tonic::Status::invalid_argument(
                    "Error: One or more invalid taddresses given.",
                ));
            }
            let utxos = zebrad_client.get_address_utxos(addr_args.addresses).await?;
            let mut address_utxos: Vec<GetAddressUtxosReply> = Vec::new();
            let mut entries: u32 = 0;
            for utxo in utxos {
                if (utxo.height.0 as u64) < addr_args.start_height {
                    continue;
                }
                entries += 1;
                if addr_args.max_entries > 0 && entries > addr_args.max_entries {
                    break;
                }
                let checked_index = match i32::try_from(utxo.output_index) {
                    Ok(index) => index,
                    Err(_) => {
                        return Err(tonic::Status::unknown(
                            "Error: Index out of range. Failed to convert to i32.",
                        ));
                    }
                };
                let checked_satoshis = match i64::try_from(utxo.satoshis) {
                    Ok(satoshis) => satoshis,
                    Err(_) => {
                        return Err(tonic::Status::unknown(
                            "Error: Satoshis out of range. Failed to convert to i64.",
                        ));
                    }
                };
                let utxo_reply = GetAddressUtxosReply {
                    address: utxo.address.to_string(),
                    txid: utxo.txid.0.to_vec(),
                    index: checked_index,
                    script: utxo.script.as_ref().to_vec(),
                    value_zat: checked_satoshis,
                    height: utxo.height.0 as u64,
                };
                address_utxos.push(utxo_reply)
            }
            Ok(tonic::Response::new(GetAddressUtxosReplyList {
                address_utxos,
            }))
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
            let zebrad_client = JsonRpcConnector::new(
                self.zebrad_rpc_uri.clone(),
                Some("xxxxxx".to_string()),
                Some("xxxxxx".to_string()),
            )
            .await?;
            let addr_args = request.into_inner();
            if !addr_args
                .addresses
                .iter()
                .all(|taddr| check_taddress(taddr).is_some())
            {
                return Err(tonic::Status::invalid_argument(
                    "Error: One or more invalid taddresses given.",
                ));
            }
            let utxos = zebrad_client.get_address_utxos(addr_args.addresses).await?;
            let (channel_tx, channel_rx) = tokio::sync::mpsc::channel(32);
            tokio::spawn(async move {
                // NOTE: This timeout is so slow due to the blockcache not being implemented. This should be reduced to 30s once functionality is in place.
                // TODO: Make [rpc_timout] a configurable system variable with [default = 30s] and [mempool_rpc_timout = 4*rpc_timeout]
                let timeout = timeout(std::time::Duration::from_secs(120), async {
                    let mut entries: u32 = 0;
                    for utxo in utxos {
                        if (utxo.height.0 as u64) < addr_args.start_height {
                            continue;
                        }
                        entries += 1;
                        if addr_args.max_entries > 0 && entries > addr_args.max_entries {
                            break;
                        }
                        let checked_index = match i32::try_from(utxo.output_index) {
                            Ok(index) => index,
                            Err(_) => {
                                let _ = channel_tx
                                    .send(Err(tonic::Status::unknown(
                                        "Error: Index out of range. Failed to convert to i32.",
                                    )))
                                    .await;
                                return;
                            }
                        };
                        let checked_satoshis = match i64::try_from(utxo.satoshis) {
                            Ok(satoshis) => satoshis,
                            Err(_) => {
                                let _ = channel_tx
                                    .send(Err(tonic::Status::unknown(
                                        "Error: Satoshis out of range. Failed to convert to i64.",
                                    )))
                                    .await;
                                return;
                            }
                        };
                        let utxo_reply = GetAddressUtxosReply {
                            address: utxo.address.to_string(),
                            txid: utxo.txid.0.to_vec(),
                            index: checked_index,
                            script: utxo.script.as_ref().to_vec(),
                            value_zat: checked_satoshis,
                            height: utxo.height.0 as u64,
                        };
                        if channel_tx.send(Ok(utxo_reply)).await.is_err() {
                            return;
                        }
                    }
                })
                .await;
                match timeout {
                    Ok(_) => {
                    }
                    Err(_) => {
                        channel_tx
                            .send(Err(tonic::Status::deadline_exceeded(
                                "Error: get_mempool_stream gRPC request timed out",
                            )))
                            .await
                            .ok();
                    }
                }
            });
            let output_stream = UtxoReplyStream::new(channel_rx);
            let stream_boxed = Box::pin(output_stream);
            Ok(tonic::Response::new(stream_boxed))
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
        // TODO: Add user and password as fields of GrpcClient and use here.
        Box::pin(async {
            let zebrad_client = JsonRpcConnector::new(
                self.zebrad_rpc_uri.clone(),
                Some("xxxxxx".to_string()),
                Some("xxxxxx".to_string()),
            )
            .await?;

            let zebra_info = zebrad_client
                .get_info()
                .await
                .map_err(|e| e.to_grpc_status())?;
            let blockchain_info = zebrad_client
                .get_blockchain_info()
                .await
                .map_err(|e| e.to_grpc_status())?;
            let build_info = get_build_info();

            let sapling_id = zebra_rpc::methods::ConsensusBranchIdHex::new(
                zebra_chain::parameters::ConsensusBranchId::from_hex("76b809bb")
                    .map_err(|_e| {
                        tonic::Status::internal(
                            "Internal Error - Consesnsus Branch ID hex conversion failed",
                        )
                    })?
                    .into(),
            );
            let sapling_activation_height = blockchain_info
                .upgrades
                .get(&sapling_id)
                .map_or(zebra_chain::block::Height(1), |sapling_json| {
                    sapling_json.into_parts().1
                });

            let consensus_branch_id = zebra_chain::parameters::ConsensusBranchId::from(
                blockchain_info.consensus.into_parts().0,
            )
            .to_string();

            Ok(tonic::Response::new(LightdInfo {
                version: build_info.version,
                vendor: "ZingoLabs ZainoD".to_string(),
                taddr_support: true,
                chain_name: blockchain_info.chain,
                sapling_activation_height: sapling_activation_height.0 as u64,
                consensus_branch_id,
                block_height: blockchain_info.blocks.0 as u64,
                git_commit: build_info.commit_hash,
                branch: build_info.branch,
                build_date: build_info.build_date,
                build_user: build_info.build_user,
                estimated_height: blockchain_info.estimated_height.0 as u64,
                zcashd_build: zebra_info.build,
                zcashd_subversion: zebra_info.subversion,
            }))
        })
    }

    /// Testing-only, requires lightwalletd --ping-very-insecure (do not enable in production) [from zebrad]
    /// This RPC has not been implemented as it is not currently used by zingolib.
    /// If you require this RPC please open an issue or PR at the Zingo-Indexer github (https://github.com/zingolabs/zingo-indexer).
    fn ping<'life0, 'async_trait>(
        &'life0 self,
        _request: tonic::Request<Duration>,
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
            Err(tonic::Status::unimplemented("ping not yet implemented. If you require this RPC please open an issue or PR at the Zingo-Indexer github (https://github.com/zingolabs/zingo-indexer)."))
        })
    }
}
