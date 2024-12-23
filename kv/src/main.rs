//
// A naive KVStore<K, V> which is just a wrapper over a
// RwLock<HashMap<K, V>> type.
//
// Uses raft as a consensus layer : [raft](https://github.com/bwaklog/quaso/tree/main/raft)
//
use std::io;
// use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::{net::TcpListener, sync::Mutex};
use tracing::field::debug;
use tracing::{debug, info, warn};
use tracing_subscriber::fmt::time;
use tracing_subscriber::FmtSubscriber;

use clap::Parser;
use raft::utils::parse_config;
use storage::kv::{Command, KVStore};

pub mod storage;

#[derive(Parser, Debug)]
pub struct Args {
    #[arg(short, long)]
    pub conf_path: String,
}

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
    let config = parse_config(PathBuf::from(args.conf_path)).expect("failed to parse config");

    let mut kv: KVStore<String, String> = KVStore::init_from_conf(&config).await;
    kv.raft.start_raft_server().await;
    debug("started raft listener");

    let raft_clone = Arc::new(Mutex::new(kv.raft.clone()));

    tokio::spawn(async move {
        raft_clone.lock().await.tick().await;

        // let mut raft_clone = raft_clone.lock().await;
        // raft_clone.tick().await;
    });

    let listener = TcpListener::bind(config.store.server_addr)
        .await
        .expect("failed to start a tcp stream");
    info!("started a tcp listener at {:?}", config.store.server_addr);
    debug!("started kv server");

    loop {
        let (mut stream, _) = listener.accept().await.unwrap();

        // tokio::spawn(async move {
        debug!("{:?}", stream);
        loop {
            let mut buf = Vec::new();

            match stream.read_buf(&mut buf).await {
                Ok(_) => {
                    if buf.is_empty() {
                        stream.shutdown().await.unwrap();
                        break;
                    }

                    let cmd = String::from_utf8(buf.clone()).unwrap();
                    let parsed = Command::<String, String>::parse_command(cmd);
                    debug!("recieved {:?}", parsed);

                    // let kv_temp = kv_clone_inner.deref();
                    // let result = kv_temp.handle_cmd(parsed.clone()).await;

                    let result = kv.handle_cmd(parsed.clone()).await;

                    stream
                        .write_all(format!("{:?}\n", result).as_bytes())
                        .await
                        .unwrap();
                    stream.flush().await.unwrap();
                }
                Err(e) => {
                    warn!("{:?}", e);
                    stream.shutdown().await.unwrap();
                    break;
                }
            }
        }
        // });
    }
}
