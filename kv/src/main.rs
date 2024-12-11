use std::io::{BufRead, BufReader, BufWriter, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::{io, thread};
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

    let args = Args::parse();
    let config = parse_config(PathBuf::from(args.conf_path)).expect("failed to parse config");

    let mut kv: KVStore<i32, String> = KVStore::init_from_conf(&config);

    debug!("started kv server");

    {
        kv.insert(1, "foo".to_owned());
        kv.insert(2, "bar".to_owned());
        kv.display();
    }

    let kv_raft = Arc::new(Mutex::new(kv.raft));

    let kv_raft_clone = Arc::clone(&kv_raft);

    thread::spawn(move || {
        kv_raft_clone.lock().unwrap().tick();
    });

    let listener = TcpListener::bind("127.0.0.1:3511").expect("failed to start a tcp stream");
    info!("started a tcp listener at :3511");

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        let stream = Arc::new(stream);

        thread::spawn(move || {
            debug!("{:?}", stream);
            let mut reader = BufReader::new(stream.as_ref());
            let mut writer = BufWriter::new(stream.as_ref());

            'clientloop: loop {
                let mut buf = String::new();
                if reader.read_line(&mut buf).is_ok() {
                    if buf.is_empty() {
                        // BUG: this might be wrong
                        let _ = stream.shutdown(std::net::Shutdown::Both);
                        break 'clientloop;
                    }
                    buf.pop();
                    debug!("client request {}", buf);

                    // basic echo testing
                    writer
                        .write_all(&format!("echo: {buf}\n").into_bytes())
                        .unwrap();
                    writer.flush().unwrap();
                }
            }
        });
    }
}
