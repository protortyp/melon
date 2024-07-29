mod arg;
use arg::Args;
use clap::Parser;
use melon_common::proto::{self, melon_scheduler_client::MelonSchedulerClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let job_id = args.job;

    let mut client = MelonSchedulerClient::connect(args.api_endpoint).await?;
    let request = tonic::Request::new(proto::GetJobInfoRequest { job_id });

    match client.get_job_info(request).await {
        Ok(response) => {
            let job = response.get_ref();
            // TODO:
            todo!("add output")
        }
        Err(e) => match e.code() {
            tonic::Code::NotFound => println!("Unknown job id {}", job_id),
            _ => println!("Unknown error: {}", e),
        },
    }

    Ok(())
}
