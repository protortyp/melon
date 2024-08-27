use crate::settings::Settings;
use axum::extract::State;
use axum::http::Method;
use axum::{routing::get, Json, Router};
use melon_common::proto::melon_scheduler_client::MelonSchedulerClient;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

pub struct Api {
    settings: Settings,
}

impl Api {
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }
    pub fn router(&self) -> Router {
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([Method::GET])
            .allow_headers(Any);

        Router::new()
            .route("/api/jobs", get(Self::get_jobs))
            .route("/api/health", get(Self::health_check))
            .layer(cors)
            .with_state(Arc::new(self.settings.clone()))
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let addr: SocketAddr = format!("{}:{}", self.settings.api.host, self.settings.api.port)
            .parse()
            .expect("Failed to parse socket address");
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, self.router()).await?;
        Ok(())
    }

    async fn get_jobs(State(settings): State<Arc<Settings>>) -> Json<Vec<melon_common::Job>> {
        let mut client =
            MelonSchedulerClient::connect(format!("http://[::1]:{}", settings.application.port))
                .await
                .unwrap();
        let request = tonic::Request::new(melon_common::proto::JobListRequest {});
        let response = client.list_jobs(request).await.unwrap();
        let jobs = response.into_inner().jobs;
        Json(jobs.into_iter().map(|job| (&job).into()).collect())
    }

    async fn health_check() -> &'static str {
        "Ok"
    }
}
