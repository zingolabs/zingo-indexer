//! Holds Zaino's local compact block cache implementation.

use crate::{error::BlockCacheError, status::StatusType};

pub mod finalised_state;
pub mod non_finalised_state;

use finalised_state::{FinalisedState, FinalisedStateSubscriber};
use non_finalised_state::{NonFinalisedState, NonFinalisedStateSubscriber};
use zaino_fetch::jsonrpc::connector::JsonRpcConnector;

/// Zaino's internal compact block cache.
///
/// Used by the FetchService for efficiency.
pub struct BlockCache {
    non_finalised_state: NonFinalisedState,
    finalised_state: FinalisedState,
}

impl BlockCache {
    /// Spawns a new [`BlockCache`].
    pub async fn spawn(
        fetcher: &JsonRpcConnector,
        capacity_and_shard_amount: Option<(usize, usize)>,
        db_path: &str,
        db_size: Option<usize>,
    ) -> Result<Self, BlockCacheError> {
        let (channel_tx, channel_rx) = tokio::sync::mpsc::channel(100);

        let finalised_state = FinalisedState::spawn(fetcher, db_path, db_size, channel_rx).await?;

        // TODO: sync db to server and wait for server to sync to p2p network.

        let non_finalised_state =
            NonFinalisedState::spawn(fetcher, capacity_and_shard_amount, channel_tx).await?;

        Ok(BlockCache {
            non_finalised_state,
            finalised_state,
        })
    }

    /// Returns a [`BlockCacheSubscriber`].
    pub fn subscriber(&self) -> BlockCacheSubscriber {
        BlockCacheSubscriber {
            non_finalised_state: self.non_finalised_state.subscriber(),
            finalised_state: self.finalised_state.subscriber(),
        }
    }

    /// Returns the status of the block cache as:
    /// (non_finalised_state_status, finalised_state_status).
    pub fn status(&self) -> (StatusType, StatusType) {
        (
            self.non_finalised_state.status(),
            self.finalised_state.status(),
        )
    }

    /// Sets the block cache to close gracefully.
    pub fn close(&mut self) {
        self.non_finalised_state.close();
        self.finalised_state.close();
    }
}

// drop

/// A subscriber to a [`BlockCache`].
#[derive(Debug, Clone)]
pub struct BlockCacheSubscriber {
    non_finalised_state: NonFinalisedStateSubscriber,
    finalised_state: FinalisedStateSubscriber,
}

impl BlockCacheSubscriber {
    // get chain height

    // get compact block
}
