//! Holds Zaino's mempool implementation.

use std::collections::HashSet;

use crate::{
    broadcast::{Broadcast, BroadcastSubscriber},
    error::{MempoolError, StatusError},
    status::{AtomicStatus, StatusType},
};
use zaino_fetch::jsonrpc::connector::JsonRpcConnector;
use zebra_chain::block::Hash;

/// Mempool key
///
/// Holds txid.
#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub struct MempoolKey(pub String);

/// Mempool value.
///
/// NOTE: Currently holds a copy of txid,
///       this could be updated to store the corresponding transaction as the value,
///       this would enable the serving of mempool trasactions directly, significantly increasing efficiency.
#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub struct MempoolValue(pub String);

/// Zcash mempool, uses dashmap for efficient serving of mempool tx.
#[derive(Debug)]
pub struct Mempool {
    /// Zcash chain fetch service.
    fetcher: JsonRpcConnector,
    /// Wrapper for a dashmap of mempool transactions.
    state: Broadcast<MempoolKey, MempoolValue>,
    /// Mempool sync handle.
    sync_task_handle: Option<tokio::task::JoinHandle<()>>,
    /// mempool status.
    status: AtomicStatus,
}

impl Mempool {
    /// Spawns a new [`Mempool`].
    pub async fn spawn(
        fetcher: &JsonRpcConnector,
        capacity_and_shard_amount: Option<(usize, usize)>,
    ) -> Result<Self, MempoolError> {
        let mut mempool = Mempool {
            fetcher: fetcher.clone(),
            state: match capacity_and_shard_amount {
                Some((capacity, shard_amount)) => Broadcast::new_custom(capacity, shard_amount),
                None => Broadcast::new_default(),
            },
            sync_task_handle: None,
            status: AtomicStatus::new(StatusType::Spawning.into()),
        };

        mempool.state.insert_filtered_set(
            mempool
                .fetcher
                .get_raw_mempool()
                .await?
                .transactions
                .into_iter()
                .map(|s| (MempoolKey(s.clone()), MempoolValue(s)))
                .collect(),
            StatusType::Ready,
        );

        mempool.sync_task_handle = Some(mempool.serve().await?);

        Ok(mempool)
    }

    async fn serve(&self) -> Result<tokio::task::JoinHandle<()>, MempoolError> {
        let fetcher = self.fetcher.clone();
        let state = self.state.clone();
        let status = self.status.clone();
        status.store(StatusType::Ready.into());

        let sync_handle = tokio::spawn(async move {
            let mut best_block_hash: Hash;
            let mut check_block_hash: Hash;

            match fetcher.get_blockchain_info().await {
                Ok(chain_info) => {
                    best_block_hash = chain_info.best_block_hash.clone();
                }
                Err(e) => {
                    status.store(StatusType::RecoverableError.into());
                    state.notify(status.into());
                    eprintln!("{e}");
                    return;
                }
            }

            loop {
                match fetcher.get_blockchain_info().await {
                    Ok(chain_info) => {
                        check_block_hash = chain_info.best_block_hash.clone();
                    }
                    Err(e) => {
                        status.store(StatusType::RecoverableError.into());
                        state.notify(status.into());
                        eprintln!("{e}");
                        return;
                    }
                }

                if check_block_hash != best_block_hash {
                    best_block_hash = check_block_hash;
                    state.notify(StatusType::Syncing);
                    state.clear();
                }

                match fetcher.get_raw_mempool().await {
                    Ok(mempool_tx) => {
                        state.insert_filtered_set(
                            mempool_tx
                                .transactions
                                .into_iter()
                                .map(|s| (MempoolKey(s.clone()), MempoolValue(s)))
                                .collect(),
                            StatusType::Ready,
                        );
                    }
                    Err(e) => {
                        status.store(StatusType::RecoverableError.into());
                        state.notify(status.into());
                        eprintln!("{e}");
                        return;
                    }
                };

                if status.load() == StatusType::Closing as usize {
                    state.notify(status.into());
                    return;
                }

                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        });

        Ok(sync_handle)
    }

    /// Returns a [`MempoolSubscriber`].
    pub fn subscriber(&self) -> MempoolSubscriber {
        MempoolSubscriber {
            subscriber: self.state.subscriber(),
            seen_txids: HashSet::new(),
            status: self.status.clone(),
        }
    }

    /// Returns the status of the mempool.
    pub fn status(&self) -> StatusType {
        self.status.load().into()
    }

    /// Sets the mempool to close gracefully.
    pub fn close(&mut self) {
        self.status.store(StatusType::Closing.into());
        self.state.notify(self.status());
        if let Some(handle) = self.sync_task_handle.take() {
            handle.abort();
        }
    }
}

impl Drop for Mempool {
    fn drop(&mut self) {
        self.status.store(StatusType::Closing.into());
        self.state.notify(StatusType::Closing);
        if let Some(handle) = self.sync_task_handle.take() {
            handle.abort();
        }
    }
}

impl Clone for Mempool {
    fn clone(&self) -> Self {
        Self {
            fetcher: self.fetcher.clone(),
            state: self.state.clone(),
            sync_task_handle: None,
            status: self.status.clone(),
        }
    }
}

/// A subscriber to a [`Mempool`].
#[derive(Debug, Clone)]
pub struct MempoolSubscriber {
    subscriber: BroadcastSubscriber<MempoolKey, MempoolValue>,
    seen_txids: HashSet<MempoolKey>,
    status: AtomicStatus,
}

impl MempoolSubscriber {
    /// Returns all tx currently in the mempool and updates seen_txids.
    pub fn get_mempool(&self) -> Vec<(MempoolKey, MempoolValue)> {
        self.subscriber.get_filtered_state(&HashSet::new())
    }

    /// Returns all tx currently in the mempool and updates seen_txids.
    pub fn get_filtered_mempool(
        &self,
        ignore_list: HashSet<MempoolKey>,
    ) -> Vec<(MempoolKey, MempoolValue)> {
        self.subscriber.get_filtered_state(&ignore_list)
    }

    /// Returns a stream of mempool txids, closes the channel when a new block has been mined.
    pub async fn get_mempool_stream(
        &mut self,
    ) -> Result<
        (
            tokio::sync::mpsc::Receiver<Result<(MempoolKey, MempoolValue), StatusError>>,
            tokio::task::JoinHandle<()>,
        ),
        MempoolError,
    > {
        let mut subscriber = self.clone();
        let (channel_tx, channel_rx) = tokio::sync::mpsc::channel(32);

        let streamer_handle = tokio::spawn(async move {
            let mempool_result: Result<(), MempoolError> = async {
                loop {
                    let (mempool_status, mempool_updates) = subscriber.wait_on_update().await?;
                    match mempool_status {
                        StatusType::Ready => {
                            for (mempool_key, mempool_value) in mempool_updates {
                                channel_tx.send(Ok((mempool_key, mempool_value))).await?;
                            }
                        }
                        StatusType::Syncing => {
                            return Ok(());
                        }
                        StatusType::Closing => {
                            return Err(MempoolError::StatusError(StatusError(
                                StatusType::Closing,
                            )));
                        }
                        status => {
                            return Err(MempoolError::StatusError(StatusError(status)));
                        }
                    }
                    if subscriber.status.load() == StatusType::Closing as usize {
                        return Err(MempoolError::StatusError(StatusError(StatusType::Closing)));
                    }
                }
            }
            .await;

            if let Err(mempool_error) = mempool_result {
                eprintln!("Error in mempool stream: {:?}", mempool_error);
                match mempool_error {
                    MempoolError::StatusError(error_status) => {
                        let _ = channel_tx.send(Err(error_status)).await;
                    }
                    _ => {
                        let _ = channel_tx
                            .send(Err(StatusError(StatusType::RecoverableError)))
                            .await;
                    }
                }
            }
        });

        Ok((channel_rx, streamer_handle))
    }

    /// Returns the status of the mempool.
    pub fn status(&self) -> StatusType {
        self.status.load().into()
    }

    /// Returns all tx currently in the mempool and updates seen_txids.
    fn get_mempool_and_update_seen(&mut self) -> Vec<(MempoolKey, MempoolValue)> {
        let mempool_updates = self.subscriber.get_filtered_state(&HashSet::new());
        for (mempool_key, _) in mempool_updates.clone() {
            self.seen_txids.insert(mempool_key);
        }
        mempool_updates
    }

    /// Returns txids not yet seen by the subscriber and updates seen_txids.
    fn get_mempool_updates_and_update_seen(&mut self) -> Vec<(MempoolKey, MempoolValue)> {
        let mempool_updates = self.subscriber.get_filtered_state(&self.seen_txids);
        for (mempool_key, _) in mempool_updates.clone() {
            self.seen_txids.insert(mempool_key);
        }
        mempool_updates
    }

    /// Waits on update from mempool and updates the mempool, returning either the new mempool or the mempool updates, along with the mempool status.
    async fn wait_on_update(
        &mut self,
    ) -> Result<(StatusType, Vec<(MempoolKey, MempoolValue)>), MempoolError> {
        let update_status = self.subscriber.wait_on_notifier().await?;
        match update_status {
            StatusType::Ready => Ok((
                StatusType::Ready,
                self.get_mempool_updates_and_update_seen(),
            )),
            StatusType::Syncing => {
                self.clear_seen();
                Ok((StatusType::Syncing, self.get_mempool_and_update_seen()))
            }
            StatusType::Closing => Ok((StatusType::Closing, Vec::new())),
            status => return Err(MempoolError::StatusError(StatusError(status))),
        }
    }

    /// Clears the subscribers seen_txids.
    fn clear_seen(&mut self) {
        self.seen_txids.clear();
    }
}
