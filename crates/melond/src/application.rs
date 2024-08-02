use crate::{scheduler::Scheduler, settings::Settings};
use anyhow::{Context, Result};
use melon_common::proto::melon_scheduler_server::MelonSchedulerServer;
use tokio::net::TcpListener;
use tonic::transport::{server::Router, Server};

pub struct Application {
    /// Settings
    settings: Settings,
    /// Server
    server: Router,
    /// Port
    port: u16,
    /// Listener
    listener: TcpListener,
}

impl Application {
    #[tracing::instrument(level = "debug", name = "Build Application")]
    pub async fn build(settings: Settings) -> Result<Self, anyhow::Error> {
        let addr = format!(
            "{}:{}",
            settings.application.host, settings.application.port
        );
        let listener = TcpListener::bind(&addr).await?;
        let port = listener.local_addr()?.port();

        let mut scheduler = Scheduler::default();
        scheduler
            .start()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to start scheduler: {}", e))?;
        scheduler
            .start_health_polling()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to start health polling: {}", e))?;

        let server = Server::builder().add_service(MelonSchedulerServer::new(scheduler));

        Ok(Self {
            settings,
            server,
            port,
            listener,
        })
    }

    pub async fn run_until_stopped(self) -> Result<()> {
        self.server
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(
                self.listener,
            ))
            .await
            .context("Server error")?;
        Ok(())
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}
