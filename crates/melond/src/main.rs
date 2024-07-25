use arg::Args;
use clap::Parser;
mod arg;
use anyhow::Result;
use melon_common::{
    proto::melon_scheduler_server::MelonSchedulerServer,
    telemetry::{get_subscriber, init_subscriber},
};
use tonic::transport::Server;
mod scheduler;
use melon_common::log;
use scheduler::Scheduler;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::Notify;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let addr = format!("[::1]:{}", args.port).parse()?;

    let subscriber = get_subscriber("melond".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    log!(info, "Starting up at {}", addr);

    let mut scheduler = Scheduler::default();
    // setup scheduler threads
    scheduler.start().await?;

    // start node poller
    scheduler.start_health_polling().await?;

    // catch shutdown signal
    let shutdown = Arc::new(Notify::new());
    let shutdown_clone = shutdown.clone();
    tokio::spawn(async move {
        signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
        shutdown_clone.notify_one();
    });

    let server = Server::builder()
        .add_service(MelonSchedulerServer::new(scheduler))
        .serve_with_shutdown(addr, shutdown.notified());

    server.await?;

    log!(info, "Server shutting down");

    Ok(())
}
