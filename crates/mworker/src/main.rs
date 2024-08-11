use clap::Parser;
use melon_common::telemetry::{get_subscriber, init_subscriber};
use mworker::{worker::Worker, Args};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = get_subscriber("mworker".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let args = Args::parse();
    let mut worker = Worker::new(&args)?;

    // connect worker
    worker.register_node().await?;

    // start regular heartbeats
    worker.start_heartbeats().await?;

    // start polling
    worker.start_polling().await?;

    // start the server
    worker.start_server().await?;

    Ok(())
}
