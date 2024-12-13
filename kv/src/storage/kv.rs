use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    hash::Hash,
};
use tokio::sync::Mutex;

use raft::{
    server::{self, state::Raft},
    utils::Config,
};
use tracing::info;

///
/// Pair<K, V> is the most fundamental portion of this
/// hashmap. This need not be separated out of the hashmap
/// rather its being experimented with to understand how
/// to make the raft layer apply committed logs to the
/// state of the HashMap
///
#[derive(Debug, Serialize, Deserialize)]
pub struct Pair<K, V>
where
    K: 'static + Eq + Hash + Debug + Send + Clone,
    V: 'static + Debug + Send + Clone,
{
    pub key: K,
    pub val: V,
}

impl<K, V> Clone for Pair<K, V>
where
    K: 'static + Eq + Hash + Debug + Send + Clone,
    V: 'static + Debug + Send + Clone,
{
    fn clone(&self) -> Self {
        Pair {
            key: self.key.clone(),
            val: self.val.clone(),
        }
    }
}

pub enum Operation {
    Set,
    Get,
    Delete,
}

pub struct Command<K, V> {
    pub operation: Operation,
    pub key: K,
    pub value: V,
}

impl<K, V> Display for Pair<K, V>
where
    K: 'static + Eq + Hash + Debug + Send + Clone,
    V: 'static + Debug + Send + Clone,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ {:?}: {:?} }}", self.key, self.val)
    }
}

impl<K, V> server::log::Entry for Pair<K, V>
where
    K: 'static + Eq + Hash + Debug + Send + Clone,
    V: 'static + Debug + Send + Clone,
{
    fn deliver(&mut self) {}
}

///
/// Quite literally...a hashmap behind a RwLock. Thinking of
/// making use of [DashMap](https://docs.rs/dashmap/latest/dashmap/)
///
/// The KVStore<K, T> uses raft for its consensus layer, by having
/// and agreement over the type Pair<K, V> defined above. The raft layer is a generic
/// consensus layer to reach consensus over a single generic type.
///
#[derive(Debug)]
pub struct KVStore<K, V>
where
    K: 'static + Eq + Hash + Debug + Serialize + DeserializeOwned + Send + Clone,
    V: 'static + Debug + Serialize + DeserializeOwned + Send + Clone,
{
    pub map: Mutex<HashMap<K, V>>,
    pub raft: Raft<Pair<K, V>>,
}

impl<K, V> KVStore<K, V>
where
    K: 'static + Eq + Hash + Debug + Serialize + DeserializeOwned + Send + Clone,
    V: 'static + Debug + Serialize + DeserializeOwned + Send + Clone,
{
    pub async fn init_from_conf(config: &Config) -> KVStore<K, V> {
        // load file and read
        KVStore {
            map: Mutex::new(HashMap::new()),
            raft: Raft::new_from_config(config).await,
        }
    }

    pub async fn insert(&mut self, key: K, value: V) {
        let mut map = self.map.lock().await;
        map.insert(key, value);
    }

    pub async fn display(&self) {
        let map = self.map.lock().await;
        info!("{:?}", map);
    }

    pub async fn delete(&self, key: K) {
        let mut map = self.map.lock().await;
        if let Some(_value) = map.get(&key) {
            // consensus over (key, value)
            map.remove(&key);
        }
    }
}
