mod arg;
use arg::Args;
use clap::Parser;
use melon_common::proto::{self, melon_scheduler_client::MelonSchedulerClient};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let job_id = args.job;

    let mut client = MelonSchedulerClient::connect(args.api_endpoint).await?;
    let request = tonic::Request::new(proto::GetJobInfoRequest { job_id });

    match client.get_job_info(request).await {
        Ok(response) => {
            let job = response.get_ref();
            if args.parseable {
                print_job_json(job)?;
            } else {
                print_job_info(job);
            }
        }
        Err(e) => match e.code() {
            tonic::Code::NotFound => println!("Unknown job id {}", job_id),
            _ => println!("Unknown error: {}", e),
        },
    }

    Ok(())
}

fn print_job_json(job: &proto::Job) -> Result<(), Box<dyn std::error::Error>> {
    let job: melon_common::Job = job.into();
    let json = serde_json::to_string_pretty(&job)?;
    println!("{}", json);
    Ok(())
}

fn print_job_info(job: &proto::Job) {
    println!(
        "{:<5} {:<20} {:<10} {:<20} {:<20} NODES",
        "JOBID", "NAME", "USER", "STATUS", "TIME"
    );

    let status = match job.status {
        0 => "PD",
        1 => "R ",
        2 => "CO",
        _ => "UK",
    };

    let elapsed_time = calculate_elapsed_time(job);
    let node = if status == "PD" {
        "(PD)".to_string()
    } else {
        job.assigned_node.clone()
    };

    let script_name = job
        .script_path
        .split('/')
        .last()
        .unwrap_or(&job.script_path);
    let truncated_script_name = truncate_str(script_name, 18);
    let truncated_user = truncate_str(&job.user, 8);
    let truncated_node = truncate_str(&node, 20);

    println!(
        "{:<5} {:<20} {:<10} {:<20} {:<20} {}",
        job.id, truncated_script_name, truncated_user, status, elapsed_time, truncated_node
    );
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.len() > max_chars {
        format!("{}...", &s[..max_chars - 3])
    } else {
        s.to_string()
    }
}

fn calculate_elapsed_time(job: &proto::Job) -> String {
    let start = job.start_time.map(|t| UNIX_EPOCH + Duration::from_secs(t));
    let stop = job.stop_time.map(|t| UNIX_EPOCH + Duration::from_secs(t));
    let now = SystemTime::now();

    let duration = match job.status {
        2 => {
            // Completed
            match (start, stop) {
                (Some(s), Some(e)) => e.duration_since(s).unwrap_or_default(),
                _ => Duration::default(),
            }
        }
        1 => {
            // Running
            match start {
                Some(s) => now.duration_since(s).unwrap_or_default(),
                None => Duration::default(),
            }
        }
        _ => {
            // Pending or any other status
            Duration::default()
        }
    };

    let days = duration.as_secs() / 86400;
    let hours = (duration.as_secs() % 86400) / 3600;
    let minutes = (duration.as_secs() % 3600) / 60;

    format!("{}-{:02}-{:02}", days, hours, minutes)
}
