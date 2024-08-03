use anyhow::Result;
use melon_common::{
    configuration::get_configuration,
    log,
    telemetry::{get_subscriber, init_subscriber},
};
use melond::application::Application;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = get_configuration().expect("Failed to read configuration.");

    let subscriber = get_subscriber("melond".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let application = Application::build(settings).await.map_err(|e| {
        log!(info, "Failed to build application: {}", e);
        std::io::Error::new(std::io::ErrorKind::Other, "Failed to build application.")
    })?;

    application.run_until_stopped().await?;
    Ok(())
}
