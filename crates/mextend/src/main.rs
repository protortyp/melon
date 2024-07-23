use arg::Args;
use clap::Parser;
mod arg;
use anyhow::Result;
use melon_common::proto::{self, melon_scheduler_client::MelonSchedulerClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let endpoint = format!("http://{}", args.api_endpoint);
    let job_id = args.job;
    let user = whoami::username();
    let time_in_mins = args.extension;
    let time_in_mins = (time_in_mins.as_secs() / 60) as u32;

    let mut client = MelonSchedulerClient::connect(endpoint.clone()).await?;
    let request = tonic::Request::new(proto::ExtendJobRequest {
        job_id,
        user,
        extension_mins: time_in_mins,
    });
    match client.extend_job(request).await {
        Ok(_) => println!(
            "Successfully extended the job runtime by {} minutes",
            time_in_mins
        ),
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
