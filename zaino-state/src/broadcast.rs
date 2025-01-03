//! Holds zaino-state::Broadcast, a thread safe broadcaster used by the mempool and non-finalised state.

use dashmap::DashMap;
use std::{collections::HashSet, hash::Hash, sync::Arc};
use tokio::sync::watch;

use crate::status::StatusType;

/// A generic, thread-safe broadcaster that manages mutable state and notifies clients of updates.
pub struct Broadcast<K, V> {
    state: Arc<DashMap<K, V>>,
    notifier: watch::Sender<StatusType>,
}

impl<K: Eq + Hash + Clone, V: Clone> Broadcast<K, V> {
    /// Creates a new Broadcast instance, uses default dashmap spec.
    pub fn new_default() -> Self {
        let (notifier, _) = watch::channel(StatusType::Spawning);
        Self {
            state: Arc::new(DashMap::new()),
            notifier,
        }
    }

    /// Creates a new Broadcast instance, exposes dashmap spec.
    pub fn new_custom(capacity: usize, shard_amount: usize) -> Self {
        let (notifier, _) = watch::channel(StatusType::Spawning);
        Self {
            state: Arc::new(DashMap::with_capacity_and_shard_amount(
                capacity,
                shard_amount,
            )),
            notifier,
        }
    }

    /// Inserts or updates an entry in the state and broadcasts an update.
    pub fn insert(&self, key: K, value: V, status: StatusType) {
        self.state.insert(key, value);
        let _ = self.notifier.send(status);
    }

    /// Inserts or updates an entry in the state and broadcasts an update.
    pub fn insert_set(&self, set: Vec<(K, V)>, status: StatusType) {
        for (key, value) in set {
            self.state.insert(key, value);
        }
        let _ = self.notifier.send(status);
    }

    /// Removes an entry from the state and broadcasts an update.
    pub fn remove(&self, key: &K, status: StatusType) {
        self.state.remove(key);
        let _ = self.notifier.send(status);
    }

    /// Retrieves a value from the state by key.
    pub fn get(&self, key: &K) -> Option<Arc<V>> {
        self.state
            .get(key)
            .map(|entry| Arc::new((*entry.value()).clone()))
    }

    /// Retrieves a set of values from the state by a list of keys.
    pub fn get_set(&self, keys: &[K]) -> Vec<(K, Arc<V>)>
    where
        K: Clone,
    {
        keys.iter()
            .filter_map(|key| {
                self.state
                    .get(key)
                    .map(|entry| (key.clone(), Arc::new((*entry.value()).clone())))
            })
            .collect()
    }

    /// Checks if a key exists in the state.
    pub fn contains_key(&self, key: &K) -> bool {
        self.state.contains_key(key)
    }

    /// Returns a receiver to listen for state update notifications.
    pub fn subscribe(&self) -> watch::Receiver<StatusType> {
        self.notifier.subscribe()
    }

    /// Provides read access to the internal state.
    pub fn get_state(&self) -> Arc<DashMap<K, V>> {
        Arc::clone(&self.state)
    }

    /// Returns the whole state excluding keys in the ignore list.
    pub fn get_filtered_state(&self, ignore_list: &HashSet<K>) -> Vec<(K, V)>
    where
        K: Clone,
        V: Clone,
    {
        self.state
            .iter()
            .filter(|entry| !ignore_list.contains(entry.key()))
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    /// Clears all entries from the state and broadcasts an update.
    pub fn clear(&self, status: StatusType) {
        self.state.clear();
        let _ = self.notifier.send(status);
    }

    /// Returns the number of entries in the state.
    pub fn len(&self) -> usize {
        self.state.len()
    }

    /// Returns true if the state is empty.
    pub fn is_empty(&self) -> bool {
        self.state.is_empty()
    }

    /// Broadcasts an update.
    pub fn notify(&self, status: StatusType) {
        if self.notifier.send(status).is_err() {
            eprintln!("No subscribers are currently listening for updates.");
        }
    }
}

impl<K: Eq + Hash + Clone, V: Clone> Default for Broadcast<K, V> {
    fn default() -> Self {
        Self::new_default()
    }
}

impl<K: Eq + Hash + Clone + std::fmt::Debug, V: Clone + std::fmt::Debug> std::fmt::Debug
    for Broadcast<K, V>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Broadcast")
            .field("state", &self.state)
            .finish()
    }
}
