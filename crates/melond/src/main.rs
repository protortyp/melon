use anyhow::Result;
use melon_common::log;
use melon_common::{
    configuration::get_configuration,
    proto::melon_scheduler_server::MelonSchedulerServer,
    telemetry::{get_subscriber, init_subscriber},
};
use melond::scheduler::Scheduler;
use melond::settings::Settings;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let configuration: Settings = get_configuration().expect("Failed to read configuration.");

    let subscriber = get_subscriber("melond".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let addr = format!(
        "{}:{}",
        configuration.application.host, configuration.application.port
    )
    .parse()?;

    log!(info, "Starting up at {}", addr);

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
