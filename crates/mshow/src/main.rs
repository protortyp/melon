mod arg;
use arg::Args;
use chrono::{TimeZone, Utc};
use clap::Parser;
use colored::*;
use melon_common::{
    proto::{self, melon_scheduler_client::MelonSchedulerClient},
    JobStatus,
};
use prettytable::{Cell, Row, Table};
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
    let mut table = Table::new();

    // Add headers
    table.add_row(Row::new(vec![
        Cell::new("JOBID"),
        Cell::new("NAME"),
        Cell::new("USER"),
        Cell::new("STATUS"),
        Cell::new("SUBMIT DATE"),
        Cell::new("START DATE"),
        Cell::new("STOP DATE"),
        Cell::new("NODES"),
    ]));

    let job_status = JobStatus::from(job.status);
    let status: String = job_status.clone().into();

    let node = if job_status == JobStatus::Pending {
        "(PD)".to_string()
    } else {
        job.assigned_node.clone()
    };

    let script_name = job
        .script_path
        .split('/')
        .last()
        .unwrap_or(&job.script_path);

    // Add job data
    table.add_row(Row::new(vec![
        Cell::new(&job.id.to_string()),
        Cell::new(truncate_str(script_name, 15).as_str()),
        Cell::new(&job.user),
        Cell::new(&status),
        Cell::new(&format_timestamp(Some(job.submit_time))),
        Cell::new(&format_timestamp(job.start_time)),
        Cell::new(&format_timestamp(job.stop_time)),
        Cell::new(&node),
    ]));

    // Set table formatting
    table.set_format(*prettytable::format::consts::FORMAT_CLEAN);

    // Print the table
    table.printstd();
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.len() > max_chars {
        format!("{}...", &s[..max_chars - 3])
    } else {
        s.to_string()
    }
}

fn format_timestamp(timestamp: Option<u64>) -> String {
    timestamp
        .and_then(|t| {
            Utc.timestamp_opt(t as i64, 0)
                .single()
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        })
        .unwrap_or_else(|| "N/A".to_string())
}

#[allow(dead_code)]
fn color_status(status: JobStatus) -> ColoredString {
    match status {
        JobStatus::Completed => "Completed".green(),
        JobStatus::Failed => "Failed".red(),
        JobStatus::Pending => "Pending".yellow(),
        JobStatus::Running => "Running".blue(),
        JobStatus::Timeout => "Timeout".purple(),
    }
}

#[allow(dead_code)]
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
