//! Compact Block Cache finalised state implementation.

use crate::status::AtomicStatus;

/// Fanalised part of the chain, held in an LLDB database.
pub struct FinalisedState {
    _status: AtomicStatus,
}
