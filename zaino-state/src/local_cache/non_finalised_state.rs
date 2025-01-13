//! Compact Block Cache non-finalised state implementation.

use zaino_fetch::jsonrpc::connector::JsonRpcConnector;
use zaino_proto::proto::compact_formats::CompactBlock;
use zebra_chain::block::{Hash, Height};

use crate::{
    broadcast::{Broadcast, BroadcastSubscriber},
    error::NonFinalisedStateError,
    status::{AtomicStatus, StatusType},
};

/// Non-finalised part of the chain (last 100 blocks), held in memory to ease the handling of reorgs.
///
/// NOTE: We hold the last 102 blocks to ensure there are no gaps in the block cache.
#[derive(Debug)]
pub struct NonFinalisedState {
    fetcher: JsonRpcConnector,
    heights_to_hashes: Broadcast<Height, Hash>,
    hashes_to_blocks: Broadcast<Hash, CompactBlock>,
    sync_task_handle: Option<tokio::task::JoinHandle<()>>,
    status: AtomicStatus,
}

impl NonFinalisedState {
    /// Spawns a new [`NonFinalisedState`].
    pub async fn spawn(
        fetcher: &JsonRpcConnector,
        capacity_and_shard_amount: Option<(usize, usize)>,
    ) -> Result<Self, NonFinalisedStateError> {
        println!("Launching Non-Finalised State..");
        let (heights_to_hashes, hashes_to_blocks) = match capacity_and_shard_amount {
            Some((capacity, shard_amount)) => (
                Broadcast::new_custom(capacity, shard_amount),
                Broadcast::new_custom(capacity, shard_amount),
            ),
            None => (Broadcast::new_default(), Broadcast::new_default()),
        };

        let mut non_finalised_state = NonFinalisedState {
            fetcher: fetcher.clone(),
            heights_to_hashes,
            hashes_to_blocks,
            sync_task_handle: None,
            status: AtomicStatus::new(StatusType::Spawning.into()),
        };

        let chain_height = fetcher.get_blockchain_info().await?.blocks.0;
        for height in chain_height.saturating_sub(101).max(1)..=chain_height {
            loop {
                match non_finalised_state
                    .fetch_compact_block_from_node(height)
                    .await
                {
                    Ok((hash, block)) => {
                        non_finalised_state
                            .heights_to_hashes
                            .insert(Height(height), hash, None);
                        non_finalised_state
                            .hashes_to_blocks
                            .insert(hash, block, None);
                        break;
                    }
                    Err(e) => {
                        non_finalised_state
                            .status
                            .store(StatusType::RecoverableError.into());
                        non_finalised_state
                            .heights_to_hashes
                            .notify(non_finalised_state.status.clone().into());
                        non_finalised_state
                            .heights_to_hashes
                            .notify(non_finalised_state.status.clone().into());
                        eprintln!("{e}");
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        continue;
                    }
                }
            }
        }

        non_finalised_state.sync_task_handle = Some(non_finalised_state.serve().await?);

        Ok(non_finalised_state)
    }

    async fn serve(&self) -> Result<tokio::task::JoinHandle<()>, NonFinalisedStateError> {
        let non_finalised_state = self.clone();
        let status = self.status.clone();
        loop {
            todo!()
        }
    }

    /// Fetches CompactBlock from the validator.
    ///
    /// Uses 2 calls as z_get_block verbosity=1 is required to fetch txids from zcashd.
    async fn fetch_compact_block_from_node(
        &self,
        height: u32,
    ) -> Result<(Hash, CompactBlock), NonFinalisedStateError> {
        match self.fetcher.get_block(height.to_string(), Some(1)).await {
            Ok(zaino_fetch::jsonrpc::response::GetBlockResponse::Object {
                hash,
                confirmations: _,
                height: _,
                time: _,
                tx,
                trees,
            }) => match self.fetcher.get_block(hash.0.to_string(), Some(0)).await {
                Ok(zaino_fetch::jsonrpc::response::GetBlockResponse::Object {
                    hash: _,
                    confirmations: _,
                    height: _,
                    time: _,
                    tx: _,
                    trees: _,
                }) => Err(NonFinalisedStateError::Custom(
                    "Found transaction of `Object` type, expected only `Hash` types.".to_string(),
                )),
                Ok(zaino_fetch::jsonrpc::response::GetBlockResponse::Raw(block_hex)) => Ok((
                    hash.0,
                    zaino_fetch::chain::block::FullBlock::parse_from_hex(
                        block_hex.as_ref(),
                        Some(Self::display_txids_to_server(tx)?),
                    )?
                    .into_compact(
                        u32::try_from(trees.sapling())?,
                        u32::try_from(trees.orchard())?,
                    )?,
                )),
                Err(e) => Err(e.into()),
            },
            Ok(zaino_fetch::jsonrpc::response::GetBlockResponse::Raw(_)) => {
                Err(NonFinalisedStateError::Custom(
                    "Found transaction of `Raw` type, expected only `Hash` types.".to_string(),
                ))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Takes a vec of big endian hex encoded txids and returns them as a vec of little endian raw bytes.
    fn display_txids_to_server(txids: Vec<String>) -> Result<Vec<Vec<u8>>, NonFinalisedStateError> {
        txids
            .iter()
            .map(|txid| {
                txid.as_bytes()
                    .chunks(2)
                    .map(|chunk| {
                        let hex_pair =
                            std::str::from_utf8(chunk).map_err(NonFinalisedStateError::from)?;
                        u8::from_str_radix(hex_pair, 16).map_err(NonFinalisedStateError::from)
                    })
                    .rev()
                    .collect::<Result<Vec<u8>, _>>()
            })
            .collect::<Result<Vec<Vec<u8>>, _>>()
    }

    /// Returns a [`NonFinalisedStateSubscriber`].
    pub fn subscriber(&self) -> NonFinalisedStateSubscriber {
        NonFinalisedStateSubscriber {
            _heights_to_hashes: self.heights_to_hashes.subscriber(),
            _hashes_to_blocks: self.hashes_to_blocks.subscriber(),
            _status: self.status.clone(),
        }
    }

    /// Returns the status of the non-finalised state.
    pub fn status(&self) -> StatusType {
        self.status.load().into()
    }

    /// Sets the non-finalised state to close gracefully.
    pub fn close(&mut self) {
        self.status.store(StatusType::Closing.into());
        self.heights_to_hashes.notify(self.status());
        self.heights_to_hashes.notify(self.status());
        if let Some(handle) = self.sync_task_handle.take() {
            handle.abort();
        }
    }
}

impl Drop for NonFinalisedState {
    fn drop(&mut self) {
        self.status.store(StatusType::Closing.into());
        self.heights_to_hashes.notify(self.status());
        self.heights_to_hashes.notify(self.status());
        if let Some(handle) = self.sync_task_handle.take() {
            handle.abort();
        }
    }
}

impl Clone for NonFinalisedState {
    fn clone(&self) -> Self {
        Self {
            fetcher: self.fetcher.clone(),
            heights_to_hashes: self.heights_to_hashes.clone(),
            hashes_to_blocks: self.hashes_to_blocks.clone(),
            sync_task_handle: None,
            status: self.status.clone(),
        }
    }
}

/// A subscriber to a [`NonFinalisedState`].
#[derive(Debug, Clone)]
pub struct NonFinalisedStateSubscriber {
    _heights_to_hashes: BroadcastSubscriber<Height, Hash>,
    _hashes_to_blocks: BroadcastSubscriber<Hash, CompactBlock>,
    _status: AtomicStatus,
}
