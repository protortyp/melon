mod arg;
use arg::Args;
use clap::Parser;
use melon_common::proto::{self, melon_scheduler_client::MelonSchedulerClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let endpoint = format!("http://{}", args.api_endpoint);

    let job_id = args.job;
    let user = whoami::username();

    let mut client = MelonSchedulerClient::connect(endpoint.clone()).await?;
    let request = tonic::Request::new(proto::CancelJobRequest { job_id, user });
    match client.cancel_job(request).await {
        Ok(_) => println!("Successfully canceled job {}", job_id),
        Err(e) => match e.code() {
            tonic::Code::NotFound => println!("Unknown job id {}", job_id),
            tonic::Code::PermissionDenied => {
                println!("Not authorized to cancel job id {}", job_id)
            }
            _ => println!("Unknown error!"),
        },
    }

    Ok(())
}
