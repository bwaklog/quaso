use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    hash::Hash,
    sync::Mutex,
};

use raft::{
    server::{self, state::Raft},
    utils::Config,
};
use tracing::info;

#[derive(Debug)]
pub struct Pair<K, V>
where
    K: Eq + Hash + Debug,
    V: Debug,
{
    pub key: K,
    pub val: V,
}

impl<K, V> Display for Pair<K, V>
where
    K: Eq + Hash + Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ {:?}: {:?} }}", self.key, self.val)
    }
}

impl<K, V> server::log::Entry for Pair<K, V>
where
    K: Eq + Hash + Debug,
    V: Debug,
{
    fn deliver(&mut self) {}
}

// a hashmap...
#[derive(Debug)]
pub struct KVStore<K, V>
where
    K: Eq + Hash + Debug,
    V: Debug,
{
    pub map: Mutex<HashMap<K, V>>,
    pub raft: Raft<Pair<K, V>>,
}

impl<K, V> KVStore<K, V>
where
    K: Eq + Hash + Debug,
    V: Debug,
{
    pub fn init_from_conf(config: &Config) -> KVStore<K, V> {
        // load file and read
        KVStore {
            map: Mutex::new(HashMap::new()),
            raft: Raft::new_from_config(config),
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        let mut map = self.map.lock().unwrap();
        map.insert(key, value);
    }

    pub fn display(&self) {
        let map = self.map.lock().unwrap();
        info!("{:?}", map);
    }

    pub fn delete(&self, key: K) {
        let mut map = self.map.lock().unwrap();
        if let Some(_value) = map.get(&key) {
            // consensus over (key, value)
            map.remove(&key);
        }
    }
}
