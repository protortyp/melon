use clap::Parser;
use mworker::{worker::Worker, Args};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Start worker");
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

    // fixme: just await on start_server
    let h = worker.server_handle.take().unwrap();
    let h = h.lock().await;
    while !h.is_finished() {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}
