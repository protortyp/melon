use melon_common::{
    configuration::get_configuration,
    log,
    telemetry::{get_subscriber, init_subscriber},
};
use melond::{db::get_prod_database_path, Api, Settings};
use melond::{Application, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let mut settings: Settings = get_configuration().expect("Failed to read configuration.");
    if settings.database.path.is_empty() {
        settings.database.path = get_prod_database_path();
    }

    let subscriber = get_subscriber("melond".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let application = Application::build(settings.clone()).await?;

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
