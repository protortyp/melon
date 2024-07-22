mod arg;
use arg::Args;
use clap::Parser;
use melon::proto::{self, melon_scheduler_client::MelonSchedulerClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let endpoint = format!("http://{}", args.api_endpoint);

    let mut client = MelonSchedulerClient::connect(endpoint.clone()).await?;
    let req = proto::JobListRequest {};
    let request = tonic::Request::new(req);
    let res = client.list_jobs(request).await?;
    let jobs = res.get_ref();

    println!(
        "{:>10} {:>11} {:>7} {:>3} {:>8}  {:<20}",
        "JOBID", "NAME", "USER", "ST", "TIME", "NODES"
    );
    for job in &jobs.jobs {
        let name = if job.name.len() > 10 {
            job.name[..10].to_string()
        } else {
            job.name.clone()
        };
        let user = if job.user.len() > 8 {
            job.user[..8].to_string()
        } else {
            job.user.clone()
        };

        println!(
            "{:>10} {:>11} {:>7} {:>3} {:>8}  {:<20}",
            job.job_id, name, user, job.status, job.time, job.nodes
        );
    }

    Ok(())
}
