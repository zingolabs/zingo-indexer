//! Holds a thread safe status implementation.

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

/// Holds a thread safe representation of a StatusType.
/// Possible values:
/// - [0: Spawning]
/// - [1: Syncing]
/// - [2: Ready]
/// - [3: Busy]
/// - [4: Closing].
/// - [>=5: Offline].
/// - [>=6: Error].
/// - [>=7: Critical-Error].
///   TODO: Refine error code spec.
#[derive(Debug, Clone)]
pub struct AtomicStatus(Arc<AtomicUsize>);

impl AtomicStatus {
    /// Creates a new AtomicStatus
    pub fn new(status: u16) -> Self {
        Self(Arc::new(AtomicUsize::new(status as usize)))
    }

    /// Loads the value held in the AtomicStatus
    pub fn load(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }

    /// Sets the value held in the AtomicStatus
    pub fn store(&self, status: usize) {
        self.0.store(status, Ordering::SeqCst);
    }
}

/// Status of the server's components.
#[derive(Debug, PartialEq, Clone)]
pub enum StatusType {
    /// Running initial startup routine.
    Spawning = 0,
    /// Back-end process is currently syncing.
    Syncing = 1,
    /// Process is ready.
    Ready = 2,
    /// Process is busy working.
    Busy = 3,
    /// Running shutdown routine.
    Closing = 4,
    /// Offline.
    Offline = 5,
    /// Non Critical Errors.
    RecoverableError = 6,
    /// Critical Errors.
    CriticalError = 7,
}

impl From<usize> for StatusType {
    fn from(value: usize) -> Self {
        match value {
            0 => StatusType::Spawning,
            1 => StatusType::Syncing,
            2 => StatusType::Ready,
            3 => StatusType::Busy,
            4 => StatusType::Closing,
            5 => StatusType::Offline,
            6 => StatusType::RecoverableError,
            _ => StatusType::CriticalError,
        }
    }
}

impl From<StatusType> for usize {
    fn from(status: StatusType) -> Self {
        status as usize
    }
}

impl From<AtomicStatus> for StatusType {
    fn from(status: AtomicStatus) -> Self {
        status.load().into()
    }
}

impl From<StatusType> for u16 {
    fn from(status: StatusType) -> Self {
        status as u16
    }
}
