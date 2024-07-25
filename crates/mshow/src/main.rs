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

            println!(
                "┌──────────────────────────────────────────────────────────────────────────────┐"
            );
            println!(
                "│ Job Information                                                              │"
            );
            println!(
                "├──────────────┬───────────────────────────────────────────────────────────────┤"
            );
            println!("│ ID           │ {:<61} │", job.id);
            println!("│ User         │ {:<61} │", job.user);
            println!("│ Status       │ {:<61} │", format!("{:?}", job.status));
            println!("│ Assigned Node│ {:<61} │", job.assigned_node);
            println!(
                "├──────────────┼───────────────────────────────────────────────────────────────┤"
            );
            println!("│ Script Path  │ {:<61} │", job.script_path);
            println!(
                "├──────────────┼───────────────────────────────────────────────────────────────┤"
            );
            println!("│ Submit Time  │ {:<61} │", job.submit_time);
            println!("│ Start Time   │ {:<61} │", job.start_time);
            println!("│ Stop Time    │ {:<61} │", job.stop_time);
            println!(
                "├──────────────┴───────────────────────────────────────────────────────────────┤"
            );
            println!(
                "│ Requested Resources                                                          │"
            );
            println!(
                "├──────────────┬───────────────────────────────────────────────────────────────┤"
            );
            if let Some(req_res) = &job.req_res {
                println!("│ CPU Count    │ {:<61} │", req_res.cpu_count);
                println!(
                    "│ Memory       │ {:<61} │",
                    format!("{} bytes", req_res.memory)
                );
                println!(
                    "│ Time         │ {:<61} │",
                    format!("{} seconds", req_res.time)
                );
            } else {
                println!("│ No requested resources information available                      │");
            }
            println!(
                "└──────────────┴───────────────────────────────────────────────────────────────┘"
            );
        }
        Err(e) => match e.code() {
            tonic::Code::NotFound => println!("Unknown job id {}", job_id),
            _ => println!("Unknown error: {}", e),
        },
    }

    Ok(())
}
