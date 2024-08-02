use anyhow::Result;
use melon_common::proto;
use melon_common::proto::melon_worker_server::MelonWorker;
use tokio::sync::mpsc::Sender;

pub struct MockWorker {
    tx: Sender<proto::JobAssignment>,
    cancel_tx: Sender<proto::CancelJobRequest>,
}

impl MockWorker {
    pub async fn new(
        tx: Sender<proto::JobAssignment>,
        cancel_tx: Sender<proto::CancelJobRequest>,
    ) -> Result<Self, anyhow::Error> {
        Ok(Self { tx, cancel_tx })
    }
}

#[tonic::async_trait]
impl MelonWorker for MockWorker {
    async fn assign_job(
        &self,
        request: tonic::Request<proto::JobAssignment>,
    ) -> Result<tonic::Response<proto::WorkerJobResponse>, tonic::Status> {
        let job_assignment = request.into_inner();
        self.tx
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
        self.cancel_tx
            .send(cancel_request)
            .await
            .map_err(|e| tonic::Status::internal(e.to_string()))?;
        Ok(tonic::Response::new(()))
    }

    async fn extend_job(
        &self,
        _request: tonic::Request<proto::ExtendJobRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        todo!()
    }
}
