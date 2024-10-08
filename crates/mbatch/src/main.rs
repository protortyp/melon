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
    let script_path = std::path::Path::new(&args.script);
    // convert to absolute path if relative
    let absolute_script_path = if script_path.is_relative() {
        std::env::current_dir()?.join(script_path)
    } else {
        script_path.to_path_buf()
    };

    let res = parse_mbatch_comments(&absolute_script_path.to_string_lossy())?;
    let req = JobSubmission {
        user: whoami::username(),
        script_path: absolute_script_path.to_string_lossy().into_owned(),
        req_res: Some(res.into()),
        script_args: args.script_args,
    };
    let request = tonic::Request::new(req);
    let response = client.submit_job(request).await?;

    println!("Started job with id: {:?}", response.get_ref().job_id);
    Ok(())
}
