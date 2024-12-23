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

use super::util::validate;

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
    pub val: Option<V>,
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

#[derive(Debug, Clone)]
pub enum Operation {
    Set,
    Get,
    Delete,
    Invalid,
}

#[derive(Clone)]
pub struct Command<K, V> {
    pub operation: Operation,
    pub key: Option<K>,
    pub value: Option<V>,
}

impl Debug for Command<String, String> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{ cmd: {:?}, key: {:?}, value: {:?} }}",
            self.operation, self.key, self.value
        )
    }
}

impl Command<String, String> {
    pub fn parse_command(command: String) -> Command<String, String> {
        if let Some(res) = validate(command) {
            match res.0 {
                Operation::Get => Command {
                    operation: res.0,
                    key: Some(res.1[1].clone()),
                    value: None,
                },
                Operation::Set => Command {
                    operation: res.0,
                    key: Some(res.1[1].clone()),
                    value: Some(res.1[2].clone()),
                },
                Operation::Delete => Command {
                    operation: res.0,
                    key: Some(res.1[1].clone()),
                    value: None,
                },
                Operation::Invalid => Command {
                    operation: res.0,
                    key: None,
                    value: None,
                },
            }
        } else {
            Command {
                operation: Operation::Invalid,
                key: None,
                value: None,
            }
        }
    }
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

#[derive(Debug)]
pub struct StorageLayer<K, V>
where
    K: 'static + Eq + Hash + Debug + Serialize + DeserializeOwned + Send + Clone,
    V: 'static + Debug + Serialize + DeserializeOwned + Send + Clone,
{
    pub map: Mutex<HashMap<K, V>>,
}

impl<K, V> StorageLayer<K, V>
where
    K: 'static + Eq + Hash + Debug + Serialize + DeserializeOwned + Send + Clone,
    V: 'static + Debug + Serialize + DeserializeOwned + Send + Clone,
{
    pub async fn init_default() -> StorageLayer<K, V> {
        StorageLayer {
            map: Mutex::new(HashMap::new()),
        }
    }

    pub async fn insert(&mut self, key: K, value: V) {
        let mut map = self.map.lock().await;
        map.insert(key, value);
    }

    pub async fn get(&mut self, key: K) -> Command<K, V> {
        let map = self.map.lock().await;
        if let Some(value) = map.get(&key) {
            return Command {
                operation: Operation::Set,
                key: Some(key),
                value: Some(value.clone()),
            };
        } else {
            return Command {
                operation: Operation::Set,
                key: Some(key),
                value: None,
            };
        }
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
    pub storage: StorageLayer<K, V>,
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
            storage: StorageLayer::init_default().await,
            raft: Raft::new_from_config(config).await,
        }
    }

    pub async fn handle_cmd(&mut self, cmd: Command<K, V>) -> Command<K, V> {
        match cmd.operation {
            // these two dont need to interact with the raft layer
            // Opeartion::Get only interacts with the current
            // state of the db
            Operation::Get => {
                return self.storage.get(cmd.key.unwrap()).await;
            }
            Operation::Invalid => {}

            // These need to pass down the entries to the raft
            // log for consensus before they can be applied to
            // the state
            Operation::Set => {
                self.raft
                    .append_entry(
                        Pair {
                            key: cmd.key.clone().unwrap(),
                            val: cmd.value.clone(),
                        },
                        raft::server::log::Command::Set,
                    )
                    .await;
                return cmd;
            }
            Operation::Delete => {
                self.raft
                    .append_entry(
                        Pair {
                            key: cmd.key.clone().unwrap(),
                            val: None,
                        },
                        raft::server::log::Command::Remove,
                    )
                    .await;
                return cmd;
            }
        }
        return Command {
            operation: Operation::Invalid,
            key: None,
            value: None,
        };
    }
}
