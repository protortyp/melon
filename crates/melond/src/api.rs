use crate::settings::Settings;
use axum::extract::State;
use axum::http::Method;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use axum::{routing::get, Router};
use melon_common::proto::melon_scheduler_client::MelonSchedulerClient;
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use thiserror::Error;
use tower_http::cors::{Any, CorsLayer};

#[derive(Error, Debug)]
enum JobError {
    #[error("Failed to connect to scheduler: {0}")]
    ConnectionError(#[from] tonic::transport::Error),
    #[error("Failed to list jobs: {0}")]
    ListError(#[from] tonic::Status),
}

impl IntoResponse for JobError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            JobError::ConnectionError(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, "Scheduler unavailable")
            }
            JobError::ListError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Failed to retrieve jobs")
            }
        };

        let body = Json(json!({
            "error": error_message,
            "details": self.to_string(),
        }));

        (status, body).into_response()
    }
}

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
            .route("/api/jobs", get(get_jobs))
            .route("/api/health", get(health_check))
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
}

async fn get_jobs(
    State(settings): State<Arc<Settings>>,
) -> Result<Json<Vec<melon_common::Job>>, JobError> {
    println!("Get job from api at {:?}", settings.application.port);

    let mut client =
        MelonSchedulerClient::connect(format!("http://[::1]:{}", settings.application.port))
            .await?;

    let request = tonic::Request::new(melon_common::proto::JobListRequest {});
    let response = client.list_jobs(request).await?;

    let jobs = response.into_inner().jobs;
    Ok(Json(jobs.into_iter().map(|job| (&job).into()).collect()))
}

async fn health_check() -> &'static str {
    "Ok"
}
