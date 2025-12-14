use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    sync::{atomic::AtomicBool, Arc},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::Mutex,
};

use raft::{
    server::{self, log::LogEntry, state::Raft},
    utils::Config,
};
use tracing::{debug, info, warn};

use super::util::validate;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Operation {
    Set { key: String, value: String },
    Get { key: String },
    Delete { key: String },
    Invalid,
}

impl server::log::Entry for Operation {
    fn deliver(&mut self) {}
}

impl std::fmt::Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Get { key } => {
                write!(f, "GET: {}", key)
            }
            Self::Set { key, value } => {
                write!(f, "SET: {} {}", key, value)
            }
            Self::Delete { key } => write!(f, "DELETE {}", key),
            Self::Invalid => write!(f, "INVALID"),
        }
    }
}

impl Operation {
    pub fn parse_command(command: String) -> Operation {
        if let Some(res) = validate(command) {
            match res.0.as_str() {
                "get" => Operation::Get {
                    key: res.1[1].clone(),
                },
                "set" => Operation::Set {
                    key: res.1[1].clone(),
                    value: res.1[2].clone(),
                },
                "delete" => Operation::Delete {
                    key: res.1[1].clone(),
                },
                _ => Operation::Invalid,
            }
        } else {
            Operation::Invalid
        }
    }
}

#[derive(Debug)]
pub struct StorageLayer {
    pub map: Mutex<HashMap<String, String>>,
}

impl StorageLayer {
    pub async fn init_default() -> StorageLayer {
        StorageLayer {
            map: Mutex::new(HashMap::new()),
        }
    }

    pub async fn insert(&mut self, key: String, value: String) {
        let mut map = self.map.lock().await;
        map.insert(key, value);
    }

    pub async fn get(&mut self, key: String) -> Option<String> {
        let map = self.map.lock().await;
        if let Some(value) = map.get(&key) {
            Some(value.clone())
        } else {
            None
        }
    }

    pub async fn set(&mut self, key: String, value: String) {
        let mut map = self.map.lock().await;
        map.entry(key)
            .and_modify(|val| *val = value.clone())
            .or_insert(value);
    }

    pub async fn delete(&mut self, key: String) {
        let mut map = self.map.lock().await;
        map.remove(&key);
    }

    pub async fn display(&self) {
        let map = self.map.lock().await;
        info!("{:?}", map);
    }
}

///
/// Quite literally...a hashmap behind a Mutex. Thinking of
/// making use of [DashMap](https://docs.rs/dashmap/latest/dashmap/)
///
/// The KVStore uses raft for its consensus layer, by having
/// and agreement over the Opeartion defined above. The raft layer is a generic
/// consensus layer to reach consensus over a single generic type.
///
#[derive(Debug)]
pub struct KVStore {
    pub storage: Arc<Mutex<StorageLayer>>,
    pub raft: Raft<Operation>,
    pub deliver_rx: Arc<Mutex<tokio::sync::mpsc::UnboundedReceiver<LogEntry<Operation>>>>,
    pub client_rx: Arc<Mutex<tokio::sync::mpsc::UnboundedReceiver<Operation>>>,
    pub client_tx: Arc<Mutex<tokio::sync::mpsc::UnboundedSender<Operation>>>,

    pub leader_ref: Arc<AtomicBool>,
}

impl KVStore {
    pub async fn init_from_conf(config: &Config) -> KVStore {
        // load file and read

        let (deliver_tx, deliver_rx) = tokio::sync::mpsc::unbounded_channel();
        let (client_tx, client_rx) = tokio::sync::mpsc::unbounded_channel();

        let deliver_tx = Arc::new(Mutex::new(deliver_tx));
        let deliver_rx = Arc::new(Mutex::new(deliver_rx));

        let raft = Raft::new_from_config(config, deliver_tx).await;
        let leader_ref = Arc::clone(&raft.is_leader);

        KVStore {
            storage: Arc::new(Mutex::new(StorageLayer::init_default().await)),
            raft,
            deliver_rx,
            leader_ref,
            client_rx: Arc::new(Mutex::new(client_rx)),
            client_tx: Arc::new(Mutex::new(client_tx)),
        }
    }

    pub async fn apply(&mut self, message: LogEntry<Operation>) {
        match message.value {
            Operation::Set { key, value } => self.storage.lock().await.set(key, value).await,
            Operation::Delete { key } => self.storage.lock().await.delete(key).await,
            _ => {}
        }
    }

    pub async fn start_deliver_interface(&mut self) {
        let deliver_rx = Arc::clone(&self.deliver_rx);
        let mut deliver_rx = deliver_rx.lock().await;
        loop {
            let try_read = deliver_rx.blocking_recv();
            match try_read {
                Some(message) => {
                    debug!("[KV_STORE] Delivering {:?} to the state", message);
                    self.apply(message).await;
                }
                None => warn!("Recieved None through deliver channel"),
            }
        }
    }

    pub async fn generic_handler_interface(&mut self) {
        let deliver_rx = Arc::clone(&self.deliver_rx);
        let mut deliver_rx = deliver_rx.lock().await;

        let client_rx = Arc::clone(&self.client_rx);
        let mut client_rx = client_rx.lock().await;

        loop {
            tokio::select! {
                Some(client_message) = client_rx.recv() => {
                    self.handle_cmd(client_message).await;
                }
                Some(raft_message) = deliver_rx.recv() => {
                    self.apply(raft_message).await;
                }
            }
        }
    }

    pub async fn handle_client(
        stream: TcpStream,
        sl: Arc<Mutex<StorageLayer>>,
        leader_ref: Arc<AtomicBool>,
        client_tx: Arc<Mutex<tokio::sync::mpsc::UnboundedSender<Operation>>>,
    ) {
        let mut stream = stream;

        loop {
            let mut buf = Vec::new();

            match stream.read_buf(&mut buf).await {
                Ok(_) => {
                    if buf.is_empty() {
                        let _ = stream.shutdown().await;
                        return;
                    }
                    let cmd = String::from_utf8(buf.clone()).unwrap();
                    let parsed = Operation::parse_command(cmd);
                    // let parsed = validate(cmd);

                    if let Operation::Get { key } = parsed {
                        let mut sl_handler = sl.lock().await;
                        let result = sl_handler.get(key).await;
                        let mut val = "undefined".to_owned();
                        if let Some(temp) = result {
                            val = format!("\"{}\"", temp);
                        }
                        stream.write_all(&val.into_bytes()).await.unwrap();
                        stream.flush().await.unwrap();
                    } else {
                        // this affects state
                        if !leader_ref.load(std::sync::atomic::Ordering::Relaxed) {
                            let _ = stream.write(b"ERR. Not Leader").await;
                            let _ = stream.flush().await;
                        } else {
                            let tx_handler = client_tx.lock().await;
                            let _ = tx_handler.send(parsed);

                            let _ = stream.write(b"OK.").await;
                            let _ = stream.flush().await;
                        }
                    }
                }
                Err(e) => {
                    warn!("{:?}", e);
                    stream.shutdown().await.unwrap();
                }
            }
        }
    }

    pub async fn handle_cmd(&mut self, cmd: Operation) {
        match cmd {
            // these two dont need to interact with the raft layer
            // Opeartion::Get only interacts with the current
            // state of the db
            Operation::Get { key } => {
                self.storage.lock().await.get(key).await;
            }
            Operation::Invalid => {}
            // For the Set and Delete
            _ => self.raft.append_entry(cmd).await,
        }
    }
}
