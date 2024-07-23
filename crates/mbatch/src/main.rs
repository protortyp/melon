use arg::Args;
use clap::Parser;
mod arg;
use anyhow::Result;
use mbatch::parse_mbatch_comments;
use melon_common::proto::melon_scheduler_client::MelonSchedulerClient;
use melon_common::proto::JobSubmission;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut client = MelonSchedulerClient::connect(args.api_endpoint).await?;

    let res = parse_mbatch_comments(&args.script)?;
    let req = JobSubmission {
        user: whoami::username(),
        script_path: args.script,
        req_res: Some(res),
        script_args: args.script_args,
    };
    let request = tonic::Request::new(req);
    let response = client.submit_job(request).await?;

    println!("Started job with id: {:?}", response.get_ref().job_id);
    Ok(())
}
