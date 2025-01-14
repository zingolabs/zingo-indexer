//! Compact Block Cache non-finalised state implementation.

use std::collections::HashSet;

use zaino_fetch::jsonrpc::connector::JsonRpcConnector;
use zaino_proto::proto::compact_formats::CompactBlock;
use zebra_chain::block::{Hash, Height};
use zebra_state::HashOrHeight;

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
                match non_finalised_state.fetch_block_from_node(height).await {
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

        let sync_handle = tokio::spawn(async move {
            let mut best_block_hash: Hash;
            let mut check_block_hash: Hash;

            loop {
                match non_finalised_state.fetcher.get_blockchain_info().await {
                    Ok(chain_info) => {
                        best_block_hash = chain_info.best_block_hash.clone();
                        non_finalised_state.status.store(StatusType::Ready.into());
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

            loop {
                if non_finalised_state.status.load() == StatusType::Closing as usize {
                    non_finalised_state
                        .heights_to_hashes
                        .notify(non_finalised_state.status.clone().into());
                    non_finalised_state
                        .heights_to_hashes
                        .notify(non_finalised_state.status.clone().into());
                    return;
                }

                match non_finalised_state.fetcher.get_blockchain_info().await {
                    Ok(chain_info) => {
                        check_block_hash = chain_info.best_block_hash.clone();
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

                if check_block_hash != best_block_hash {
                    best_block_hash = check_block_hash;
                    non_finalised_state.status.store(StatusType::Syncing.into());
                    non_finalised_state
                        .heights_to_hashes
                        .notify(non_finalised_state.status.clone().into());
                    non_finalised_state
                        .heights_to_hashes
                        .notify(non_finalised_state.status.clone().into());
                    loop {
                        match non_finalised_state.fill_from_reorg().await {
                            Ok(_) => break,
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

                non_finalised_state.status.store(StatusType::Ready.into());
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        });

        Ok(sync_handle)
    }

    /// Looks back through the chain to find reorg height and repopulates block cache.
    ///
    /// Newly mined blocks are treated as a reorg at chain_height[-0].
    async fn fill_from_reorg(&self) -> Result<(), NonFinalisedStateError> {
        let mut reorg_height = self
            .heights_to_hashes
            .get_state()
            .iter()
            .max_by_key(|entry| entry.key().0)
            .ok_or_else(|| {
                NonFinalisedStateError::MissingData(
                    "Failed to find the maximum height in the non-finalised state.".to_string(),
                )
            })?
            .key()
            .clone();

        let mut reorg_hash = self.heights_to_hashes.get(&reorg_height).ok_or_else(|| {
            NonFinalisedStateError::MissingData(format!(
                "Missing hash for height: {}",
                reorg_height.0
            ))
        })?;

        let mut check_hash = match self
            .fetcher
            .get_block(reorg_height.0.to_string(), Some(1))
            .await?
        {
            zaino_fetch::jsonrpc::response::GetBlockResponse::Object { hash, .. } => hash.0,
            _ => {
                return Err(NonFinalisedStateError::Custom(
                    "Unexpected block response type".to_string(),
                ))
            }
        };

        // Find reorg height.
        //
        // Here this is the latest height at which the internal block hash matches the server block hash.
        while reorg_hash != check_hash.into() {
            match reorg_height.previous() {
                Ok(height) => reorg_height = height,
                // Underflow error meaning reorg_height = start of chain.
                // This means the whole non-finalised state is old.
                // We fetch the first block in the chain here as the later refill logic always starts from [reorg_height + 1].
                Err(_) => {
                    self.heights_to_hashes.clear();
                    self.hashes_to_blocks.clear();
                    loop {
                        match self.fetch_block_from_node(reorg_height.0).await {
                            Ok((hash, block)) => {
                                self.heights_to_hashes.insert(reorg_height, hash, None);
                                self.hashes_to_blocks.insert(hash, block, None);
                                break;
                            }
                            Err(e) => {
                                self.status.store(StatusType::RecoverableError.into());
                                self.heights_to_hashes.notify(self.status.clone().into());
                                self.heights_to_hashes.notify(self.status.clone().into());
                                eprintln!("{e}");
                                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                                continue;
                            }
                        }
                    }
                    break;
                }
            };

            reorg_hash = self.heights_to_hashes.get(&reorg_height).ok_or_else(|| {
                NonFinalisedStateError::MissingData(format!(
                    "Missing hash for height: {}",
                    reorg_height.0
                ))
            })?;

            check_hash = match self
                .fetcher
                .get_block(reorg_height.0.to_string(), Some(1))
                .await?
            {
                zaino_fetch::jsonrpc::response::GetBlockResponse::Object { hash, .. } => hash.0,
                _ => {
                    return Err(NonFinalisedStateError::Custom(
                        "Unexpected block response type".to_string(),
                    ))
                }
            };
        }

        // Refill from reorg_height[+1].
        for block_height in (reorg_height.0 + 1)..self.fetcher.get_blockchain_info().await?.blocks.0
        {
            loop {
                match self.fetch_block_from_node(block_height).await {
                    Ok((hash, block)) => {
                        self.heights_to_hashes
                            .insert(Height(block_height), hash, None);
                        self.hashes_to_blocks.insert(hash, block, None);
                        break;
                    }
                    Err(e) => {
                        self.status.store(StatusType::RecoverableError.into());
                        self.heights_to_hashes.notify(self.status.clone().into());
                        self.heights_to_hashes.notify(self.status.clone().into());
                        eprintln!("{e}");
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        continue;
                    }
                }
            }

            // Either pop the reorged block or pop the oldest block in non-finalised state.
            //
            // TODO: Block being popped from non-finalised state should be fetched and sent to finalised state.
            if self.heights_to_hashes.contains_key(&Height(block_height)) {
                if let Some(hash) = self.heights_to_hashes.get(&Height(block_height)) {
                    self.hashes_to_blocks.remove(&hash, None);
                    self.heights_to_hashes.remove(&Height(block_height), None);
                }
            } else {
                let pop_height = self
                    .heights_to_hashes
                    .get_state()
                    .iter()
                    .min_by_key(|entry| entry.key().0)
                    .ok_or_else(|| {
                        NonFinalisedStateError::MissingData(
                            "Failed to find the minimum height in the non-finalised state."
                                .to_string(),
                        )
                    })?
                    .key()
                    .clone();
                if let Some(hash) = self.heights_to_hashes.get(&pop_height) {
                    self.hashes_to_blocks.remove(&hash, None);
                    self.heights_to_hashes.remove(&pop_height, None);
                }
            }
        }

        Ok(())
    }

    /// Fetches CompactBlock from the validator.
    ///
    /// Uses 2 calls as z_get_block verbosity=1 is required to fetch txids from zcashd.
    async fn fetch_block_from_node(
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
            heights_to_hashes: self.heights_to_hashes.subscriber(),
            hashes_to_blocks: self.hashes_to_blocks.subscriber(),
            status: self.status.clone(),
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
    heights_to_hashes: BroadcastSubscriber<Height, Hash>,
    hashes_to_blocks: BroadcastSubscriber<Hash, CompactBlock>,
    status: AtomicStatus,
}

impl NonFinalisedStateSubscriber {
    /// Returns a Compact Block from the non-finalised state.
    pub async fn get_compact_block(
        &self,
        hash_or_height: String,
    ) -> Result<CompactBlock, NonFinalisedStateError> {
        let hash_or_height: HashOrHeight = hash_or_height.parse().map_err(|_| {
            NonFinalisedStateError::InvalidHashOrHeight(format!(
                "Failed to parse hash_or_height: {}",
                hash_or_height
            ))
        })?;

        let hash = match hash_or_height {
            HashOrHeight::Hash(hash) => hash,
            HashOrHeight::Height(height) => {
                *self.heights_to_hashes.get(&height).ok_or_else(|| {
                    NonFinalisedStateError::MissingData(format!(
                        "Height not found in non-finalised state: {}",
                        height.0
                    ))
                })?
            }
        };

        self.hashes_to_blocks
            .get(&hash)
            .and_then(|block| Some(block.as_ref().clone()))
            .ok_or_else(|| {
                NonFinalisedStateError::MissingData(format!("Block not found for hash: {}", hash))
            })
    }

    /// Returns the height of the latest block in the non-finalised state.
    pub async fn get_chain_height(&self) -> Result<Height, NonFinalisedStateError> {
        let (height, _) = self
            .heights_to_hashes
            .get_filtered_state(&HashSet::new())
            .iter()
            .max_by_key(|(height, ..)| height.0)
            .ok_or_else(|| {
                NonFinalisedStateError::MissingData("Non-finalised state is empty.".into())
            })?
            .clone();

        Ok(height)
    }

    /// Returns the status of the NonFinalisedState..
    pub fn status(&self) -> StatusType {
        self.status.load().into()
    }
}
