use anyhow::Result;
use melon_common::{
    configuration::get_configuration,
    proto::{
        self, melon_scheduler_client::MelonSchedulerClient, Heartbeat, HeartbeatResponse, NodeInfo,
        NodeResources, RegistrationResponse,
    },
};
use melond::{application::Application, settings::Settings};
use tempdir::TempDir;
use tonic::Response;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct TestApp {
    pub address: String,
    #[allow(dead_code)]
    pub port: u16,
}

impl TestApp {
    pub async fn register_node(
        &self,
        info: NodeInfo,
    ) -> Result<Response<RegistrationResponse>, Box<dyn std::error::Error>> {
        let mut client = MelonSchedulerClient::connect(self.address.clone().to_string()).await?;
        let request = tonic::Request::new(info);
        let response = client.register_node(request).await?;
        Ok(response)
    }

    pub async fn send_heartbeat(
        &self,
        node_id: String,
    ) -> Result<Response<HeartbeatResponse>, Box<dyn std::error::Error>> {
        let mut client = MelonSchedulerClient::connect(self.address.clone().to_string()).await?;
        let req = Heartbeat { node_id };

        let request = tonic::Request::new(req);
        let response = client.send_heartbeat(request).await?;
        Ok(response)
    }

    pub async fn submit_job(
        &self,
        submission: proto::JobSubmission,
    ) -> Result<tonic::Response<proto::MasterJobResponse>, Box<dyn std::error::Error>> {
        let mut client = MelonSchedulerClient::connect(self.address.clone().to_string()).await?;
        let request = tonic::Request::new(submission);
        let response = client.submit_job(request).await?;
        Ok(response)
    }

    pub async fn list_jobs(
        &self,
    ) -> Result<tonic::Response<proto::JobListResponse>, Box<dyn std::error::Error>> {
        let mut client = MelonSchedulerClient::connect(self.address.clone().to_string()).await?;
        let request = tonic::Request::new(proto::JobListRequest {});
        let response = client.list_jobs(request).await?;
        Ok(response)
    }

    pub async fn submit_job_result(
        &self,
        result: proto::JobResult,
    ) -> Result<tonic::Response<proto::JobResultResponse>, Box<dyn std::error::Error>> {
        let mut client = MelonSchedulerClient::connect(self.address.clone().to_string()).await?;
        let request = tonic::Request::new(result);
        let response = client.submit_job_result(request).await?;
        Ok(response)
    }

    pub async fn cancel_job(
        &self,
        request: proto::CancelJobRequest,
    ) -> Result<tonic::Response<()>, Box<dyn std::error::Error>> {
        let mut client = MelonSchedulerClient::connect(self.address.clone().to_string()).await?;
        let request = tonic::Request::new(request);
        let response = client.cancel_job(request).await?;
        Ok(response)
    }

    pub async fn extend_job(
        &self,
        request: proto::ExtendJobRequest,
    ) -> Result<tonic::Response<()>, Box<dyn std::error::Error>> {
        let mut client = MelonSchedulerClient::connect(self.address.clone().to_string()).await?;
        let request = tonic::Request::new(request);
        let response = client.extend_job(request).await?;
        Ok(response)
    }

    pub async fn get_job_info(
        &self,
        request: proto::GetJobInfoRequest,
    ) -> Result<tonic::Response<proto::Job>, Box<dyn std::error::Error>> {
        let mut client = MelonSchedulerClient::connect(self.address.clone().to_string()).await?;
        let request = tonic::Request::new(request);
        let response = client.get_job_info(request).await?;
        Ok(response)
    }
}

fn configure_common_settings(c: &mut Settings) {
    let tmp_dir = TempDir::new(&Uuid::new_v4().to_string()).unwrap();
    let db_path = tmp_dir
        .path()
        .join("melon.db")
        .to_str()
        .unwrap()
        .to_string();
    c.application.port = 0;
    c.database.path = db_path;
}

pub async fn spawn_app() -> TestApp {
    configure_and_spawn_app(|c: &mut Settings| {
        configure_common_settings(c);
    })
    .await
}

async fn configure_and_spawn_app<F>(config_modifier: F) -> TestApp
where
    F: FnOnce(&mut Settings),
{
    let settings = {
        let mut s = get_configuration().expect("Failed to read config");
        config_modifier(&mut s);
        s
    };

    let application = Application::build(settings.clone())
        .await
        .expect("Failed to build application");
    let port = application.port();

    tokio::spawn(async move { application.run_until_stopped().await });

    TestApp {
        address: format!("http://{}:{}", settings.application.host, port),
        port,
    }
}

pub fn get_node_info(port: u16) -> NodeInfo {
    let resources = NodeResources {
        cpu_count: 8,
        memory: 4 * 1024 * 1024,
    };
    NodeInfo {
        address: format!("http://[::1]:{}", port),
        resources: Some(resources),
    }
}
