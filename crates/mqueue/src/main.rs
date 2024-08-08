mod arg;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use arg::Args;
use clap::Parser;
use melon_common::{
    proto::{self, melon_scheduler_client::MelonSchedulerClient},
    Job, JobStatus,
};

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
        let job: Job = job.into();

        let name = if job.script_path.len() > 10 {
            job.script_path[..10].to_string()
        } else {
            job.script_path.clone()
        };
        let user = if job.user.len() > 8 {
            job.user[..8].to_string()
        } else {
            job.user.clone()
        };

        let node = match job.status {
            JobStatus::Pending => "pending".to_string(),
            _ => job
                .assigned_node
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
        };
        let time = calculate_job_time(&job);

        let status = match job.status {
            JobStatus::Completed => "C".to_string(),
            JobStatus::Failed => "F".to_string(),
            JobStatus::Pending => "PD".to_string(),
            JobStatus::Running => "R".to_string(),
            JobStatus::Timeout => "TO".to_string(),
        };

        println!(
            "{:>10} {:>11} {:>7} {:>3} {:>8}  {:<20}",
            job.id, name, user, status, time, node
        );
    }

    Ok(())
}

fn calculate_job_time(job: &Job) -> String {
    match job.status {
        JobStatus::Pending => "00:00:00".to_string(),
        JobStatus::Running => {
            if let Some(start_time) = job.start_time {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let duration = Duration::from_secs(now - start_time);
                format_duration(duration)
            } else {
                "00:00:00".to_string()
            }
        }
        JobStatus::Completed | JobStatus::Failed | JobStatus::Timeout => {
            if let (Some(start_time), Some(stop_time)) = (job.start_time, job.stop_time) {
                let duration = Duration::from_secs(stop_time - start_time);
                format_duration(duration)
            } else {
                "00:00:00".to_string()
            }
        }
    }
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}
