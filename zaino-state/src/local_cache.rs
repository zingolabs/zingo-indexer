//! Holds Zaino's local compact block cache implementation.

use crate::{error::BlockCacheError, status::StatusType};

pub mod finalised_state;
pub mod non_finalised_state;

use finalised_state::{FinalisedState, FinalisedStateSubscriber};
use non_finalised_state::{NonFinalisedState, NonFinalisedStateSubscriber};
use zaino_fetch::jsonrpc::connector::JsonRpcConnector;
use zaino_proto::proto::compact_formats::CompactBlock;
use zebra_chain::block::Height;
use zebra_state::HashOrHeight;

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
    /// Returns a Compact Block from the [`BlockCache`].
    pub async fn get_compact_block(
        &self,
        hash_or_height: String,
    ) -> Result<CompactBlock, BlockCacheError> {
        let hash_or_height: HashOrHeight = hash_or_height.parse()?;

        match self
            .non_finalised_state
            .conatins_hash_or_height(hash_or_height)
            .await
        {
            true => self
                .non_finalised_state
                .get_compact_block(hash_or_height)
                .await
                .map_err(|e| BlockCacheError::NonFinalisedStateError(e)),
            false => self
                .finalised_state
                .get_compact_block(hash_or_height)
                .await
                .map_err(|e| BlockCacheError::FinalisedStateError(e)),
        }
    }

    /// Returns the height of the latest block in the [`BlockCache`].
    pub async fn get_chain_height(&self) -> Result<Height, BlockCacheError> {
        self.non_finalised_state
            .get_chain_height()
            .await
            .map_err(|e| BlockCacheError::NonFinalisedStateError(e))
    }

    /// Returns the status of the [`BlockCache`]..
    pub fn status(&self) -> (StatusType, StatusType) {
        (
            self.non_finalised_state.status(),
            self.finalised_state.status(),
        )
    }
}
