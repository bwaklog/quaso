use std::path::PathBuf;

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
    println!("Hello, world!");

    let args = Args::parse();
    let config = parse_config(PathBuf::from(args.conf_path)).expect("failed to parse config");

    let mut kv: KVStore<i32, String> = KVStore::init_from_conf(&config);

    {
        kv.insert(1, "foo".to_owned());
        kv.insert(2, "bar".to_owned());
        kv.display();
    }
}
