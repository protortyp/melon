use crate::{Result, Scheduler, Settings};
use melon_common::{log, proto::melon_scheduler_server::MelonSchedulerServer};
use tokio::net::TcpListener;
use tonic::transport::{server::Router, Server};

pub struct Application {
    /// Settings
    #[allow(dead_code)]
    settings: Settings,
    /// Server
    server: Router,
    /// Port
    port: u16,
    /// Listener
    listener: TcpListener,
}

impl Application {
    #[tracing::instrument(level = "info", name = "Build Application")]
    pub async fn build(settings: Settings) -> Result<Self> {
        let addr = format!(
            "{}:{}",
            settings.application.host, settings.application.port
        );
        let listener = TcpListener::bind(&addr).await?;
        let port = listener.local_addr()?.port();

        log!(
            info,
            "Starting scheduler on {}:{}",
            settings.application.host,
            port
        );
        let mut scheduler = Scheduler::new(&settings);
        scheduler.start().await?;
        scheduler.start_health_polling().await?;
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
            .await?;
        Ok(())
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}
