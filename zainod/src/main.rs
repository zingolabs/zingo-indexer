//! Zingo-Indexer daemon

use clap::{CommandFactory, Parser};
use std::path::PathBuf;
use zainodlib::{config::load_config, indexer::Indexer};

#[derive(Parser, Debug)]
#[command(name = "Zaino", about = "The Zcash Indexing Service", version)]
struct Args {
    /// Path to the configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if std::env::args().any(|arg| arg == "--version" || arg == "-V") {
        let cmd = Args::command();
        println!("{}", cmd.get_version().unwrap());
        return;
    }

    Indexer::start(load_config(
        &args
            .config
            .unwrap_or_else(|| PathBuf::from("./zainod/zindexer.toml")),
    ))
    .await
    .unwrap();
}
