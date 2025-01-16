//! Holds Zaino's local compact block cache implementation.

use crate::status::AtomicStatus;

pub mod finalised_state;
pub mod non_finalised_state;

use finalised_state::FinalisedState;
use non_finalised_state::NonFinalisedState;

/// Zaino's internal compact block cache.
///
/// Used by the FetchService for efficiency.
pub struct BlockCache {
    _non_finalised_state: NonFinalisedState,
    _finalised_state: FinalisedState,
}

impl BlockCache {
    // spawn

    // subscriber

    // status

    // close
}

// drop

// block cache subscriber

// get chain height

// get compact block
