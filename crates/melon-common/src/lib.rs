use chrono::{DateTime, Utc};
use proto::{JobAssignment, JobSubmission};
use std::time::Instant;
pub mod error;
pub mod telemetry;
pub mod proto {
    tonic::include_proto!("melon");
}

#[derive(Clone, Debug)]
pub struct Job {
    /// The unique ID, created by the scheduler
    pub id: u64,

    /// The user that submitted the job
    pub user: String,

    /// The path to the script to execute
    pub script_path: String,

    /// The script arguments
    pub script_args: Vec<String>,

    /// The requested resources
    pub req_res: RequestedResources,

    /// The time the job was submitted
    pub submit_time: DateTime<Utc>,

    /// Start time
    pub start_time: Option<DateTime<Utc>>,

    /// Stop time
    pub stop_time: Option<DateTime<Utc>>,

    /// The job status
    pub status: JobStatus,

    /// The id of the compute node that is working on this job
    pub assigned_node: Option<String>,
}

impl Job {
    pub fn new(
        id: u64,
        user: String,
        script_path: String,
        script_args: Vec<String>,
        req_res: RequestedResources,
    ) -> Self {
        Self {
            id,
            user,
            script_path,
            script_args,
            req_res,
            submit_time: Utc::now(),
            start_time: None,
            stop_time: None,
            status: JobStatus::Pending,
            assigned_node: None,
        }
    }

    pub fn extend_time(&mut self, extenion_in_mins: u32) {
        self.req_res.time += extenion_in_mins;
    }
}

impl From<&mut Job> for JobSubmission {
    fn from(val: &mut Job) -> Self {
        JobSubmission {
            user: val.user.clone(),
            script_path: val.script_path.clone(),
            req_res: Some(val.req_res.to_resources()),
            script_args: val.script_args.clone(),
        }
    }
}

impl From<&mut Job> for JobAssignment {
    fn from(val: &mut Job) -> Self {
        JobAssignment {
            job_id: val.id,
            user: val.user.clone(),
            script_path: val.script_path.clone(),
            req_res: Some(val.req_res.to_resources()),
            script_args: val.script_args.clone(),
        }
    }
}

/// Requested resources for a job.
#[derive(Clone, Debug)]
pub struct RequestedResources {
    pub cpu_count: u8,
    pub memory: u64,
    pub time: u32,
}

impl RequestedResources {
    pub fn new(cpu_count: u8, memory: u64, time: u32) -> Self {
        Self {
            cpu_count,
            memory,
            time,
        }
    }

    pub fn from_proto(proto: proto::Resources) -> Self {
        Self {
            cpu_count: proto.cpu_count as u8,
            memory: proto.memory,
            time: proto.time,
        }
    }

    pub fn to_resources(&self) -> proto::Resources {
        proto::Resources {
            cpu_count: self.cpu_count as u32,
            memory: self.memory,
            time: self.time,
        }
    }
}

/// Available Resources on a worker node.
#[derive(Clone, Debug)]
pub struct NodeResources {
    pub cpu_count: u8,
    pub memory: u64,
}

impl NodeResources {
    pub fn new(cpu_count: u8, memory: u64) -> Self {
        Self { cpu_count, memory }
    }

    pub fn empty() -> Self {
        Self {
            cpu_count: 0,
            memory: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub enum JobStatus {
    Completed,
    Failed(String),
    Pending,
    Running,
    Timeout,
}

/// A compute node instance.
#[derive(Clone, Debug)]
pub struct Node {
    /// Unique ID, created by the master node upon registration
    pub id: String,

    /// Endpoint of the compute node
    pub endpoint: String,

    /// Total Available Resources
    pub avail_resources: NodeResources,

    /// Resources that are currently in use
    pub used_resources: NodeResources,

    /// Last heartbeat
    pub last_heartbeat: Instant,

    /// Reachability status
    pub status: NodeStatus,
}

impl Node {
    pub fn new(id: String, address: String, avail_res: NodeResources, status: NodeStatus) -> Self {
        Self {
            id,
            endpoint: address,
            avail_resources: avail_res,
            status,
            used_resources: NodeResources::empty(),
            last_heartbeat: Instant::now(),
        }
    }

    pub fn set_status(&mut self, status: NodeStatus) {
        self.status = status;
    }

    /// Reduce available resources
    pub fn reduce_avail_resources(&mut self, res: &RequestedResources) {
        self.used_resources.cpu_count += res.cpu_count;
        self.used_resources.memory += res.memory;
    }

    /// Free up available resources
    pub fn free_avail_resource(&mut self, res: &RequestedResources) {
        self.used_resources.cpu_count -= res.cpu_count;
        self.used_resources.memory -= res.memory;
    }

    /// Update heartbeat
    pub fn update_heartbeat(&mut self) {
        self.last_heartbeat = Instant::now();
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum NodeStatus {
    Available,
    Offline,
}

#[derive(Clone, Debug)]
pub struct JobResult {
    /// The [Job] id
    pub id: u64,

    /// The job status (either completed or failed)
    pub status: JobStatus,
}

impl JobResult {
    pub fn new(id: u64, status: JobStatus) -> Self {
        Self { id, status }
    }
}

impl From<JobResult> for proto::JobResult {
    fn from(value: JobResult) -> Self {
        let (status, message) = match value.status {
            JobStatus::Completed => (0, String::new()),
            JobStatus::Failed(msg) => (1, msg),
            JobStatus::Running => (2, String::new()),
            _ => unreachable!("Unreachable job status"),
        };

        Self {
            job_id: value.id,
            status,
            message,
        }
    }
}

impl From<&proto::JobResult> for JobResult {
    fn from(value: &proto::JobResult) -> Self {
        let status = match value.status {
            0 => JobStatus::Completed,
            1 => JobStatus::Failed(value.message.clone()),
            2 => JobStatus::Pending,
            3 => JobStatus::Running,
            _ => panic!("Unknown status"),
        };

        JobResult {
            id: value.job_id,
            status,
        }
    }
}

impl From<Job> for proto::JobInfo {
    fn from(job: Job) -> Self {
        let run_time = if let Some(start) = job.start_time {
            let elapsed = Utc::now().signed_duration_since(start);
            let days = elapsed.num_days();
            let hours = elapsed.num_hours() % 24;
            let mut minutes = elapsed.num_minutes() % 60;
            if minutes == 0 && (days > 0 || hours > 0) {
                minutes = 1;
            }
            format!("{}-{:02}-{:02}", days, hours, minutes)
        } else {
            "0-00-00".to_string()
        };

        let nodes = if let Some(node) = job.assigned_node {
            node
        } else {
            "(PD)".to_string()
        };

        let status = match job.status {
            JobStatus::Pending => "PD".to_string(),
            JobStatus::Running => "R".to_string(),
            _ => unreachable!(),
        };

        Self {
            job_id: job.id,
            user: job.user.clone(),
            name: job.script_path.clone(),
            status,
            time: run_time,
            nodes,
        }
    }
}
