use std::io;
use std::path::PathBuf;
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

fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::DEBUG)
        .with_timer(time::ChronoLocal::rfc_3339())
        .with_target(true)
        .with_writer(io::stderr)
        .finish();

    tracing::subscriber::set_global_default(subscriber).unwrap();

    println!("Hello, world!");

    let args = Args::parse();
    let config = parse_config(PathBuf::from(args.conf_path)).expect("failed to parse config");

    let mut kv: KVStore<i32, String> = KVStore::init_from_conf(&config);

    debug!("started kv server");

    {
        kv.insert(1, "foo".to_owned());
        kv.insert(2, "bar".to_owned());
        kv.display();
    }

    kv.raft.tick();

    dbg!(kv.raft.state);
}
