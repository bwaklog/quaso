use clap::Parser;
use tokio::{
    io::{stdin, stdout, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    publisher: bool,

    #[arg(short, long, default_value = "localhost:3511")]
    addr: String,

    #[arg(short, long)]
    server: String,

    topic: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if args.publisher {
        start_publisher(args.topic, args.server).await;
    } else {
        start_subscriber(args.addr, args.topic, args.server).await;
    }
}

async fn start_publisher(topic: String, server: String) {
    // register a topic
    let mut stream = TcpStream::connect(server)
        .await
        .expect("failed to crete stream to register topic");

    let _ = stream
        .write_all(&format!("PUB {}", topic).into_bytes())
        .await;
    let _ = stream.flush().await;

    loop {
        print!("> ");
        let _ = stdout().flush().await;

        let mut buf = Vec::new();

        let _ = stdin().read(&mut buf).await;

        let _ = stream.write_all(&buf).await;
        let _ = stream.flush().await;
    }
}

async fn start_subscriber(addr: String, topic: String, server: String) {
    let listener = TcpListener::bind(addr.clone()).await.unwrap();

    let mut stream = TcpStream::connect(server)
        .await
        .expect("failed to crete stream to register topic");

    let _ = stream
        .write_all(&format!("SUB {}", topic).into_bytes())
        .await;
    let _ = stream.flush().await;

    let _ = stream.shutdown().await;

    loop {
        let (mut stream, _) = listener.accept().await.unwrap();

        let mut buf = Vec::new();

        let _ = stream.read_buf(&mut buf).await;
        println!("{}", String::from_utf8(buf).unwrap());
    }
}
