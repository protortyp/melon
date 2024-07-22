use arg::Args;
use clap::Parser;
mod arg;
use anyhow::Result;
use melon::proto::melon_scheduler_server::MelonSchedulerServer;
use tonic::transport::Server;
mod scheduler;
use scheduler::Scheduler;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let addr = format!("[::1]:{}", args.port).parse()?;

    println!("Starting up at {}", addr);
    let mut scheduler = Scheduler::default();
    // setup scheduler threads
    scheduler.start().await?;

    // start node poller
    scheduler.start_health_polling().await?;

    Server::builder()
        .add_service(MelonSchedulerServer::new(scheduler))
        .serve(addr)
        .await?;

    Ok(())
}
