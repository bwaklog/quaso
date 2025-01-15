//
// A naive KVStore which is just a wrapper over a
// HashMap<String, String> type.
//
// Uses raft as a consensus layer : [raft](https://github.com/bwaklog/quaso/tree/main/raft)
//
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::{net::TcpListener, sync::Mutex};
use tracing::field::debug;
use tracing::{debug, info};
use tracing_subscriber::fmt::time;
use tracing_subscriber::FmtSubscriber;

use clap::Parser;
use raft::utils::parse_config;
use storage::kv::KVStore;

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

    let mut kv = KVStore::init_from_conf(&config).await;
    kv.raft.start_raft_server().await;
    debug("started raft listener");

    let raft_clone = Arc::new(Mutex::new(kv.raft.clone()));

    let client_tx = Arc::clone(&kv.client_tx);

    tokio::spawn(async move {
        raft_clone.lock().await.tick().await;
    });

    let sl = kv.storage.clone();

    tokio::spawn(async move {
        kv.generic_handler_interface().await;
    });

    let listener = TcpListener::bind(config.store.server_addr.clone())
        .await
        .expect("failed to start a tcp stream");
    info!("started a tcp listener at {:?}", config.store.server_addr);
    debug!("started kv server");

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let client_tx = Arc::clone(&client_tx);
        let sl_clone = sl.clone();

        debug!("{:?}", stream);
        tokio::spawn(async move {
            KVStore::handle_client(stream, sl_clone, client_tx).await;
        });
    }
}
