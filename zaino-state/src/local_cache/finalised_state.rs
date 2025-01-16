//! Compact Block Cache finalised state implementation.

use std::{fs, sync::Arc};

use lmdb::{Database, Environment, Transaction};
use prost::Message;
use serde::{Deserialize, Serialize};
use zaino_fetch::jsonrpc::connector::JsonRpcConnector;
use zaino_proto::proto::compact_formats::CompactBlock;
use zebra_chain::block::{Hash, Height};
use zebra_state::HashOrHeight;

use crate::{
    error::FinalisedStateError,
    status::{AtomicStatus, StatusType},
};

/// Wrapper for `Height`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct DbHeight(pub Height);

/// Wrapper for `Hash`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct DbHash(pub Hash);

/// Wrapper for `CompactBlock`.
#[derive(Debug, Clone, PartialEq)]
struct DbCompactBlock(pub CompactBlock);

/// Custom `Serialize` implementation using Prost's `encode_to_vec()`.
impl Serialize for DbCompactBlock {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let bytes = self.0.encode_to_vec();
        serializer.serialize_bytes(&bytes)
    }
}

/// Custom `Deserialize` implementation using Prost's `decode()`.
impl<'de> Deserialize<'de> for DbCompactBlock {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes: Vec<u8> = serde::de::Deserialize::deserialize(deserializer)?;
        CompactBlock::decode(&*bytes)
            .map(DbCompactBlock)
            .map_err(serde::de::Error::custom)
    }
}

/// A Zaino database request.
struct DbRequest {
    hash_or_height: HashOrHeight,
    response_channel: tokio::sync::oneshot::Sender<Result<CompactBlock, FinalisedStateError>>,
}

impl DbRequest {
    /// Creates a new [`DbRequest`].
    fn new(
        hash_or_height: HashOrHeight,
        response_channel: tokio::sync::oneshot::Sender<Result<CompactBlock, FinalisedStateError>>,
    ) -> Self {
        Self {
            hash_or_height,
            response_channel,
        }
    }
}

/// Fanalised part of the chain, held in an LMDB database.
pub struct FinalisedState {
    /// Chain fetch service.
    fetcher: JsonRpcConnector,
    /// LMDB Database Environmant.
    database: Arc<Environment>,
    /// LMDB Databas containing `<block_height, block_hash>`.
    heights_to_hashes: Database,
    /// LMDB Databas containing `<block_hash, compact_block>`.
    hashes_to_blocks: Database,
    /// Database reader request sender.
    request_sender: tokio::sync::mpsc::Sender<DbRequest>,
    /// Database reader task handle.
    read_task_handle: Option<tokio::task::JoinHandle<()>>,
    /// Database writer task handle.
    write_task_handle: Option<tokio::task::JoinHandle<()>>,
    /// Non-finalised state status.
    status: AtomicStatus,
}

impl FinalisedState {
    /// Spawns a new [`NonFinalisedState`] and syncs the FinalisedState to the servers finalised state.
    ///
    /// Inputs:
    /// - fetcher: Json RPC client.
    /// - db_path: File path of the db.
    /// - db_size: Max size of the db in gb.
    /// - block_reciever: Channel that recieves new blocks to add to the db.
    /// - status: FinalisedState status.
    pub async fn spawn(
        fetcher: &JsonRpcConnector,
        db_path: &str,
        db_size: Option<usize>,
        block_receiver: tokio::sync::mpsc::Receiver<(Height, Hash, CompactBlock)>,
        status: AtomicStatus,
    ) -> Result<Self, FinalisedStateError> {
        let db_size = db_size.unwrap_or(8);
        if !std::path::Path::new(db_path).exists() {
            fs::create_dir_all(db_path)?;
        }
        let database = Arc::new(
            Environment::new()
                .set_max_dbs(2)
                .set_map_size(db_size * 1024 * 1024 * 1024)
                .open(std::path::Path::new(db_path))?,
        );

        let heights_to_hashes = match database.open_db(Some("heights_to_hashes")) {
            Ok(db) => db,
            Err(lmdb::Error::NotFound) => {
                database.create_db(Some("heights_to_hashes"), lmdb::DatabaseFlags::empty())?
            }
            Err(e) => return Err(FinalisedStateError::LmdbError(e)),
        };
        let hashes_to_blocks = match database.open_db(Some("hashes_to_blocks")) {
            Ok(db) => db,
            Err(lmdb::Error::NotFound) => {
                database.create_db(Some("hashes_to_blocks"), lmdb::DatabaseFlags::empty())?
            }
            Err(e) => return Err(FinalisedStateError::LmdbError(e)),
        };

        let (request_tx, request_rx) = tokio::sync::mpsc::channel(124);

        let mut finalised_state = FinalisedState {
            fetcher: fetcher.clone(),
            database,
            heights_to_hashes,
            hashes_to_blocks,
            request_sender: request_tx,
            read_task_handle: None,
            write_task_handle: None,
            status,
        };

        finalised_state.write_task_handle =
            Some(finalised_state.spawn_writer(block_receiver).await?);

        finalised_state.read_task_handle = Some(finalised_state.spawn_reader(request_rx).await?);

        Ok(finalised_state)
    }

    async fn spawn_writer(
        &self,
        mut block_receiver: tokio::sync::mpsc::Receiver<(Height, Hash, CompactBlock)>,
    ) -> Result<tokio::task::JoinHandle<()>, FinalisedStateError> {
        let finalised_state = self.clone();

        let writer_handle = tokio::spawn(async move {
            while let Some((height, mut hash, mut compact_block)) = block_receiver.recv().await {
                let mut retry_attempts = 3;

                loop {
                    match finalised_state.insert_block((height, hash, compact_block.clone())) {
                        Ok(_) => {
                            println!("✅ Block at height {} successfully inserted.", height.0);
                            break;
                        }
                        Err(FinalisedStateError::LmdbError(lmdb::Error::KeyExist)) => {
                            match finalised_state.get_hash(height.0) {
                                Ok(db_hash) => {
                                    if db_hash != hash {
                                        if finalised_state.delete_block((height, db_hash)).is_err()
                                        {
                                            finalised_state
                                                .status
                                                .store(StatusType::CriticalError.into());
                                            return;
                                        };
                                        continue;
                                    } else {
                                        println!(
                                            "⚠️ Block at height {} already exists, skipping.",
                                            height.0
                                        );
                                        break;
                                    }
                                }
                                Err(_) => {
                                    finalised_state
                                        .status
                                        .store(StatusType::CriticalError.into());
                                    return;
                                }
                            }
                        }
                        Err(FinalisedStateError::LmdbError(db_err)) => {
                            eprintln!("❌ LMDB error inserting block {}: {:?}", height.0, db_err);
                            // serious error!
                            break;
                        }
                        Err(e) => {
                            eprintln!(
                                "⚠️ Unknown error inserting block {}: {:?}. Retrying...",
                                height.0, e
                            );

                            if retry_attempts == 0 {
                                eprintln!(
                                    "❌ Failed to insert block {} after multiple retries.",
                                    height.0
                                );
                                finalised_state
                                    .status
                                    .store(StatusType::CriticalError.into());
                                return;
                            }

                            retry_attempts -= 1;

                            match finalised_state.fetch_block_from_node(height.0).await {
                                Ok((new_hash, new_compact_block)) => {
                                    eprintln!(
                                        "🔄 Re-fetched block at height {}, retrying insert.",
                                        height.0
                                    );
                                    hash = new_hash;
                                    compact_block = new_compact_block;
                                }
                                Err(fetch_err) => {
                                    eprintln!(
                                        "❌ Failed to fetch block {} from validator: {:?}",
                                        height.0, fetch_err
                                    );
                                    finalised_state
                                        .status
                                        .store(StatusType::CriticalError.into());
                                    return;
                                }
                            }

                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        }
                    }
                }
            }
        });

        Ok(writer_handle)
    }

    async fn spawn_reader(
        &self,
        mut request_receiver: tokio::sync::mpsc::Receiver<DbRequest>,
    ) -> Result<tokio::task::JoinHandle<()>, FinalisedStateError> {
        let finalised_state = self.clone();

        let reader_handle = tokio::spawn(async move {
            while let Some(DbRequest {
                hash_or_height,
                response_channel,
            }) = request_receiver.recv().await
            {
                let response = match finalised_state.get_block(hash_or_height) {
                    Ok(block) => Ok(block),
                    Err(_) => Err(FinalisedStateError::MissingData(format!(
                        "Block {:?} not found in finalised state.",
                        hash_or_height
                    ))),
                };

                if response_channel.send(response).is_err() {
                    eprintln!("Failed to send response for request: {:?}", hash_or_height);
                }
            }
        });

        Ok(reader_handle)
    }

    /// Inserts a block into the finalised state.
    fn insert_block(&self, block: (Height, Hash, CompactBlock)) -> Result<(), FinalisedStateError> {
        let (height, hash, compact_block) = block;
        let height_key = serde_json::to_vec(&DbHeight(height))?;
        let hash_key = serde_json::to_vec(&DbHash(hash))?;
        let block_value = serde_json::to_vec(&DbCompactBlock(compact_block))?;

        let mut txn = self.database.begin_rw_txn()?;
        if txn
            .put(
                self.heights_to_hashes,
                &height_key,
                &hash_key,
                lmdb::WriteFlags::NO_OVERWRITE,
            )
            .is_err()
            || txn
                .put(
                    self.hashes_to_blocks,
                    &hash_key,
                    &block_value,
                    lmdb::WriteFlags::NO_OVERWRITE,
                )
                .is_err()
        {
            txn.abort();
            return Err(FinalisedStateError::Custom(
                "Transaction failed.".to_string(),
            ));
        }
        txn.commit()?;
        Ok(())
    }

    /// Deletes a block from the finalised state.
    fn delete_block(&self, block: (Height, Hash)) -> Result<(), FinalisedStateError> {
        let (height, hash) = block;
        let height_key = serde_json::to_vec(&DbHeight(height))?;
        let hash_key = serde_json::to_vec(&DbHash(hash))?;

        let mut txn = self.database.begin_rw_txn()?;
        txn.del(self.heights_to_hashes, &height_key, None)?;
        txn.del(self.hashes_to_blocks, &hash_key, None)?;
        txn.commit()?;
        Ok(())
    }

    /// Retrieves a CompactBlock by Height or Hash.
    ///
    /// NOTE: It may be more efficient to implement a `get_block_range` method and batch database read calls.
    fn get_block(&self, height_or_hash: HashOrHeight) -> Result<CompactBlock, FinalisedStateError> {
        let txn = self.database.begin_ro_txn()?;

        let hash_key = match height_or_hash {
            HashOrHeight::Height(height) => {
                let height_key = serde_json::to_vec(&DbHeight(height))?;
                let hash_bytes: &[u8] = txn.get(self.heights_to_hashes, &height_key)?;
                hash_bytes.to_vec()
            }
            HashOrHeight::Hash(hash) => serde_json::to_vec(&DbHash(hash))?,
        };

        let block_bytes: &[u8] = txn.get(self.hashes_to_blocks, &hash_key)?;
        let block: DbCompactBlock = serde_json::from_slice(block_bytes)?;
        Ok(block.0)
    }

    /// Retrieves a Hash by Height.
    fn get_hash(&self, height: u32) -> Result<Hash, FinalisedStateError> {
        let txn = self.database.begin_ro_txn()?;

        let height_key = serde_json::to_vec(&DbHeight(Height(height)))?;

        let hash_bytes: &[u8] = match txn.get(self.heights_to_hashes, &height_key) {
            Ok(bytes) => bytes,
            Err(lmdb::Error::NotFound) => {
                return Err(FinalisedStateError::MissingData(format!(
                    "No hash found for height {}",
                    height
                )));
            }
            Err(e) => return Err(FinalisedStateError::LmdbError(e)),
        };

        let hash: Hash = serde_json::from_slice(hash_bytes)?;
        Ok(hash)
    }

    /// Fetches CompactBlock from the validator.
    ///
    /// Uses 2 calls as z_get_block verbosity=1 is required to fetch txids from zcashd.
    async fn fetch_block_from_node(
        &self,
        height: u32,
    ) -> Result<(Hash, CompactBlock), FinalisedStateError> {
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
                }) => Err(FinalisedStateError::Custom(
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
                Err(FinalisedStateError::Custom(
                    "Found transaction of `Raw` type, expected only `Hash` types.".to_string(),
                ))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Takes a vec of big endian hex encoded txids and returns them as a vec of little endian raw bytes.
    fn display_txids_to_server(txids: Vec<String>) -> Result<Vec<Vec<u8>>, FinalisedStateError> {
        txids
            .iter()
            .map(|txid| {
                txid.as_bytes()
                    .chunks(2)
                    .map(|chunk| {
                        let hex_pair =
                            std::str::from_utf8(chunk).map_err(FinalisedStateError::from)?;
                        u8::from_str_radix(hex_pair, 16).map_err(FinalisedStateError::from)
                    })
                    .rev()
                    .collect::<Result<Vec<u8>, _>>()
            })
            .collect::<Result<Vec<Vec<u8>>, _>>()
    }

    /// Returns a [`FinalisedStateSubscriber`].
    pub fn subscriber(&self) -> FinalisedStateSubscriber {
        FinalisedStateSubscriber {
            request_sender: self.request_sender.clone(),
            status: self.status.clone(),
        }
    }

    /// Returns the status of the finalised state.
    pub fn status(&self) -> StatusType {
        self.status.load().into()
    }

    /// Sets the finalised state to close gracefully.
    pub fn close(&mut self) {
        self.status.store(StatusType::Closing.into());
        if let Some(handle) = self.read_task_handle.take() {
            handle.abort();
        }
        if let Some(handle) = self.write_task_handle.take() {
            handle.abort();
        }

        if let Err(e) = self.database.sync(true) {
            eprintln!("❌ Error syncing LMDB before shutdown: {:?}", e);
        }
    }
}

impl Drop for FinalisedState {
    fn drop(&mut self) {
        self.status.store(StatusType::Closing.into());
        if let Some(handle) = self.read_task_handle.take() {
            handle.abort();
        }
        if let Some(handle) = self.write_task_handle.take() {
            handle.abort();
        }

        if let Err(e) = self.database.sync(true) {
            eprintln!("❌ Error syncing LMDB before shutdown: {:?}", e);
        }
    }
}

impl Clone for FinalisedState {
    fn clone(&self) -> Self {
        Self {
            fetcher: self.fetcher.clone(),
            database: Arc::clone(&self.database),
            heights_to_hashes: self.heights_to_hashes,
            hashes_to_blocks: self.hashes_to_blocks,
            request_sender: self.request_sender.clone(),
            read_task_handle: None,
            write_task_handle: None,
            status: self.status.clone(),
        }
    }
}

/// A subscriber to a [`NonFinalisedState`].
#[derive(Debug, Clone)]
pub struct FinalisedStateSubscriber {
    request_sender: tokio::sync::mpsc::Sender<DbRequest>,
    status: AtomicStatus,
}

impl FinalisedStateSubscriber {
    /// Returns a Compact Block from the non-finalised state.
    pub async fn get_compact_block(
        &self,
        hash_or_height: String,
    ) -> Result<CompactBlock, FinalisedStateError> {
        let hash_or_height: HashOrHeight = hash_or_height.parse().map_err(|_| {
            FinalisedStateError::InvalidHashOrHeight(format!(
                "Failed to parse hash_or_height: {}",
                hash_or_height
            ))
        })?;

        let (channel_tx, channel_rx) = tokio::sync::oneshot::channel();
        if self
            .request_sender
            .send(DbRequest::new(hash_or_height, channel_tx))
            .await
            .is_err()
        {
            return Err(FinalisedStateError::Custom(
                "Error sending request to db reader".to_string(),
            ));
        }

        let result = tokio::time::timeout(std::time::Duration::from_secs(30), channel_rx).await;
        match result {
            Ok(Ok(compact_block)) => compact_block,
            Ok(Err(_)) => Err(FinalisedStateError::Custom(
                "Error receiving block from db reader".to_string(),
            )),
            Err(_) => Err(FinalisedStateError::Custom(
                "Timeout while waiting for compact block".to_string(),
            )),
        }
    }

    /// Returns the status of the FinalisedState..
    pub fn status(&self) -> StatusType {
        self.status.load().into()
    }
}
