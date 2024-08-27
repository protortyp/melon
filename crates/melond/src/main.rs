use anyhow::Result;
use melon_common::{
    configuration::get_configuration,
    log,
    telemetry::{get_subscriber, init_subscriber},
};
use melond::{api::Api, application::Application, db::get_prod_database_path, settings::Settings};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut settings: Settings = get_configuration().expect("Failed to read configuration.");
    settings.database.path = get_prod_database_path();

    let subscriber = get_subscriber("melond".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let application = Application::build(settings.clone()).await.map_err(|e| {
        log!(info, "Failed to build application: {}", e);
        std::io::Error::new(std::io::ErrorKind::Other, "Failed to build application.")
    })?;

    #[cfg(feature = "api")]
    {
        let api = Api::new(settings.clone());
        tokio::spawn(async move {
            if let Err(e) = api.start().await {
                log!(error, "API Server error: {}", e);
            }
        });
    }

    application.run_until_stopped().await?;
    Ok(())
}
