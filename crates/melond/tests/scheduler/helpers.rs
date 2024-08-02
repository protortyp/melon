use anyhow::Result;
use melon_common::{
    configuration::get_configuration,
    proto::{
        melon_scheduler_client::MelonSchedulerClient, Heartbeat, HeartbeatResponse, NodeInfo,
        NodeResources, RegistrationResponse,
    },
};
use melond::{application::Application, settings::Settings};
use tonic::Response;

#[derive(Clone, Debug)]
pub struct TestApp {
    pub address: String,
    pub port: u16,
}

impl TestApp {
    pub async fn register_node(
        &self,
    ) -> Result<Response<RegistrationResponse>, Box<dyn std::error::Error>> {
        let mut client = MelonSchedulerClient::connect(self.address.clone().to_string()).await?;
        let resources = NodeResources {
            cpu_count: 8,
            memory: 4 * 1024 * 1024,
        };
        let req = NodeInfo {
            address: format!("http://[::1]:{}", self.port),
            resources: Some(resources),
        };
        let request = tonic::Request::new(req);
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
}

fn configure_common_settings(c: &mut Settings) {
    c.application.port = 0; // assign random port
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
