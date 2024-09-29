use anyhow::Result;
use melon_common::proto;
use melon_common::proto::melon_worker_server::{MelonWorker, MelonWorkerServer};
use tokio::net::TcpListener;
use tokio::sync::mpsc::{self, Sender};
use tokio::sync::watch;
use tonic::transport::Server;

pub struct MockWorker {
    // Job assignmend sender
    job_assignment_sender: Sender<proto::JobAssignment>,

    // Used when the worker receives a cancellation request for running jobs
    job_cancellation_sender: Sender<proto::CancelJobRequest>,

    // Used when the worker receives an extension request for running jobs
    job_extension_sender: Sender<proto::ExtendJobRequest>,
}

impl MockWorker {
    pub async fn new(
        job_assignment_sender: Sender<proto::JobAssignment>,
        job_cancellation_sender: Sender<proto::CancelJobRequest>,
        job_extension_sender: Sender<proto::ExtendJobRequest>,
    ) -> Result<Self, anyhow::Error> {
        Ok(Self {
            job_assignment_sender,
            job_cancellation_sender,
            job_extension_sender,
        })
    }
}

#[tonic::async_trait]
impl MelonWorker for MockWorker {
    async fn assign_job(
        &self,
        request: tonic::Request<proto::JobAssignment>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let job_assignment = request.into_inner();
        self.job_assignment_sender
            .send(job_assignment)
            .await
            .map_err(|e| tonic::Status::internal(e.to_string()))?;

        Ok(tonic::Response::new(()))
    }

    async fn cancel_job(
        &self,
        request: tonic::Request<proto::CancelJobRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let cancel_request = request.into_inner();
        self.job_cancellation_sender
            .send(cancel_request)
            .await
            .map_err(|e| tonic::Status::internal(e.to_string()))?;
        Ok(tonic::Response::new(()))
    }

    async fn extend_job(
        &self,
        request: tonic::Request<proto::ExtendJobRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let extension_request = request.into_inner();
        self.job_extension_sender
            .send(extension_request)
            .await
            .map_err(|e| tonic::Status::internal(e.to_string()))?;
        Ok(tonic::Response::new(()))
    }
}

pub struct MockWorkerSetup {
    pub job_assignment_receiver: mpsc::Receiver<proto::JobAssignment>,
    pub job_cancellation_receiver: mpsc::Receiver<proto::CancelJobRequest>,
    pub server_notifier: watch::Sender<()>,
    pub server_handle: tokio::task::JoinHandle<()>,
    pub job_extension_receiver: mpsc::Receiver<proto::ExtendJobRequest>,
    pub port: u16,
}

pub async fn setup_mock_worker() -> MockWorkerSetup {
    let (job_assignment_sender, job_assignment_receiver) = mpsc::channel(1);
    let (job_cancellation_sender, job_cancellation_receiver) = mpsc::channel(1);
    let (server_notifier, server_notifier_rx) = watch::channel(());
    let (job_extension_sender, job_extension_receiver) = mpsc::channel(1);

    let worker = MockWorker::new(
        job_assignment_sender.clone(),
        job_cancellation_sender.clone(),
        job_extension_sender.clone(),
    )
    .await
    .unwrap();

    let addr = String::from("[::1]:0");
    let listener = TcpListener::bind(&addr).await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let mut shutdown_rx = server_notifier_rx.clone();

    let server_handle = tokio::spawn(async move {
        Server::builder()
            .add_service(MelonWorkerServer::new(worker))
            .serve_with_incoming_shutdown(
                tokio_stream::wrappers::TcpListenerStream::new(listener),
                async {
                    shutdown_rx.changed().await.ok();
                },
            )
            .await
            .unwrap();
    });

    MockWorkerSetup {
        job_assignment_receiver,
        job_cancellation_receiver,
        server_notifier,
        server_handle,
        job_extension_receiver,
        port,
    }
}
