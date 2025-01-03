//! Holds Zaino's mempool implementation.

use crate::{broadcast::Broadcast, status::AtomicStatus};
use zebra_chain::block::Hash;

pub struct Mempool {
    state: Broadcast<String, String>,
    best_block_hash: Hash,
    sync_task_handle: tokio::task::JoinHandle<()>,
    status: AtomicStatus,
}

impl Mempool {
    pub fn spawn() -> Self {
        todo!()
    }
}
