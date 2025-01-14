//! Compact Block Cache finalised state implementation.

use lmdb::{Database, Environment};
use zaino_proto::proto::compact_formats::CompactBlock;
use zebra_state::HashOrHeight;

use crate::status::AtomicStatus;

/// Fanalised part of the chain, held in an LLDB database.
pub struct FinalisedState {
    /// LMDB Database Environmant.
    _database: Environment,
    /// LMDB Databas containing `<block_height, block_hash>`.
    _heights_to_hashes: Database,
    /// LMDB Databas containing `<block_hash, compact_block>`.
    _hashes_to_blocks: Database,
    /// Database reader request sender.
    _reader: tokio::sync::mpsc::Sender<(HashOrHeight, tokio::sync::oneshot::Sender<CompactBlock>)>,
    /// Database reader task handle.
    _read_task_handle: Option<tokio::task::JoinHandle<()>>,
    /// Database writer task handle.
    _write_task_handle: Option<tokio::task::JoinHandle<()>>,
    /// Non-finalised state status.
    _status: AtomicStatus,
}
