use anyhow::Result;
use melon_common::proto;
use melon_common::proto::melon_worker_server::MelonWorker;
use tokio::sync::mpsc::Sender;

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
    ) -> Result<tonic::Response<proto::WorkerJobResponse>, tonic::Status> {
        let job_assignment = request.into_inner();
        self.job_assignment_sender
            .send(job_assignment)
            .await
            .map_err(|e| tonic::Status::internal(e.to_string()))?;

        let res = proto::WorkerJobResponse { ack: true };
        Ok(tonic::Response::new(res))
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
