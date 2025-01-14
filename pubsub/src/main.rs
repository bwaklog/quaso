use std::collections::HashMap;
use std::fmt::Display;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use crossbeam::queue::SegQueue;
use raft::server::log::LogEntry;
use raft::server::{log::Entry, state::Raft};
use raft::utils::{Config, RaftConfig, StoreConfig};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tracing::debug;
use tracing_subscriber::fmt::time;
use tracing_subscriber::FmtSubscriber;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Message {
    content: String,
    channel: String,
}

impl Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{ \"content\": \"{}\", \"channel\":\"{}\" }}",
            self.content, self.channel
        )
    }
}

// need to remove this
impl Entry for Message {
    fn deliver(&mut self) {}
}

struct Topic {
    subscribers: Vec<TcpStream>,
}

#[allow(dead_code)]
struct PubSub {
    topics: HashMap<String, Topic>,
    queue: SegQueue<Message>,
}

impl PubSub {}

#[allow(dead_code)]
struct Server {
    raft: Raft<Message>,
    pubsub: PubSub,
    client_rx: Arc<Mutex<tokio::sync::mpsc::UnboundedReceiver<TcpStream>>>,
}

impl Server {
    async fn publish(&mut self, message: Message) {
        self.raft
            .append_entry(message, raft::server::log::Command::Set)
            .await;
    }

    async fn task(&mut self) {}
}

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long, default_value = "./pubsub/conf.yml")]
    conf_path: String,

    #[arg(short, long, default_value = "localhost:8080")]
    listener: String,

    #[arg(short, long)]
    node_id: u64,

    #[arg(short, long)]
    raftaddr: String,

    #[arg(long)]
    conns: String,
}

#[allow(unused)]
#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::DEBUG)
        .with_timer(time::ChronoLocal::rfc_3339())
        .with_target(true)
        .with_writer(io::stderr)
        .finish();

    tracing::subscriber::set_global_default(subscriber).unwrap();
    let args = Args::parse();

    let (deliver_tx, deliver_rx) = tokio::sync::mpsc::unbounded_channel::<LogEntry<Message>>();
    let deliver_tx = Arc::new(Mutex::new(deliver_tx));

    let (client_tx, client_rx) = tokio::sync::mpsc::unbounded_channel();

    let raft = Raft::new_from_config(
        &Config {
            node_id: args.node_id,
            node_name: "dummy".to_owned(),
            raft: RaftConfig {
                persist_path: PathBuf::from(format!("./pubsub/node_{}.state", args.node_id)),
                listener_addr: args.raftaddr,
                connections: args.conns.split(";").map(String::from).collect(),
            },
            store: StoreConfig {
                server_addr: "localhost:6060".to_string(),
                local_path: PathBuf::from("."),
            },
        },
        deliver_tx,
    )
    .await;

    let server = Server {
        raft,
        pubsub: PubSub {
            topics: HashMap::new(),
            queue: SegQueue::new(),
        },
        client_rx: Arc::new(Mutex::new(client_rx)),
    };

    server.raft.start_raft_server().await;

    let raft_clone = Arc::new(Mutex::new(server.raft.clone()));

    tokio::spawn(async move {
        raft_clone.lock().await.tick().await;
    });

    let listener = TcpListener::bind(args.listener.clone()).await.unwrap();
    println!("Started pubsub listener at {}", args.listener.clone());

    loop {
        let (mut stream, _) = listener.accept().await.unwrap();
        debug!("connected client {:?}", stream.local_addr());

        tokio::spawn(async move {
            let mut stream = stream;
            loop {
                let mut buf: Vec<u8> = Vec::new();
                let mut temp: Vec<u8> = Vec::new();

                stream.read_buf(&mut buf).await;

                if buf.is_empty() {
                    stream.shutdown().await;
                    break;
                } else {
                    stream.write_all(b"recieved\n").await;
                    stream.flush().await;
                }
            }
        });
    }
}
