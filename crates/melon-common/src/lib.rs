use proto::JobSubmission;
use std::time::Instant;
use utils::get_current_timestamp;
pub mod configuration;
pub mod error;
pub mod telemetry;
use serde::{Deserialize, Serialize};
pub mod utils;

pub mod proto {
    tonic::include_proto!("melon");
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
    pub submit_time: u64,

    /// Start time
    pub start_time: Option<u64>,

    /// Stop time
    pub stop_time: Option<u64>,

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
            submit_time: get_current_timestamp(),
            start_time: None,
            stop_time: None,
            status: JobStatus::Pending,
            assigned_node: None,
        }
    }

    pub fn extend_time(&mut self, extension_in_mins: u32) {
        self.req_res.time += extension_in_mins;
    }
}

impl From<&Job> for proto::Job {
    fn from(job: &Job) -> Self {
        proto::Job {
            id: job.id,
            user: job.user.clone(),
            script_path: job.script_path.clone(),
            script_args: job.script_args.clone().into_iter().collect(),
            req_res: Some(job.req_res.into()),
            submit_time: job.submit_time,
            start_time: job.start_time,
            stop_time: job.stop_time,
            status: proto::JobStatus::from(job.status.clone()).into(),
            assigned_node: job.assigned_node.clone().unwrap_or_default(),
        }
    }
}

impl From<&proto::Job> for Job {
    fn from(job: &proto::Job) -> Self {
        Job {
            id: job.id,
            user: job.user.clone(),
            script_path: job.script_path.clone(),
            script_args: job.script_args.clone().into_iter().collect(),
            req_res: job.req_res.unwrap().into(),
            submit_time: job.submit_time,
            start_time: job.start_time,
            stop_time: job.stop_time,
            status: JobStatus::from(job.status()),
            assigned_node: if job.assigned_node.is_empty() {
                None
            } else {
                Some(job.assigned_node.clone())
            },
        }
    }
}

impl From<&mut Job> for JobSubmission {
    fn from(val: &mut Job) -> Self {
        JobSubmission {
            user: val.user.clone(),
            script_path: val.script_path.clone(),
            req_res: Some(val.req_res.into()),
            script_args: val.script_args.clone(),
        }
    }
}

impl From<&mut Job> for proto::JobAssignment {
    fn from(val: &mut Job) -> Self {
        proto::JobAssignment {
            job_id: val.id,
            user: val.user.clone(),
            script_path: val.script_path.clone(),
            req_res: Some(val.req_res.into()),
            script_args: val.script_args.clone(),
        }
    }
}

/// Requested resources for a job.
#[derive(Clone, Debug, Copy, Deserialize, Serialize)]
pub struct RequestedResources {
    pub cpu_count: u32,
    pub memory: u64,
    pub time: u32,
}

impl From<RequestedResources> for proto::RequestedResources {
    fn from(req_res: RequestedResources) -> Self {
        proto::RequestedResources {
            cpu_count: req_res.cpu_count,
            memory: req_res.memory,
            time: req_res.time,
        }
    }
}

impl From<&mut RequestedResources> for proto::RequestedResources {
    fn from(req_res: &mut RequestedResources) -> Self {
        proto::RequestedResources {
            cpu_count: req_res.cpu_count,
            memory: req_res.memory,
            time: req_res.time,
        }
    }
}

impl From<proto::RequestedResources> for RequestedResources {
    fn from(res: proto::RequestedResources) -> Self {
        RequestedResources {
            cpu_count: res.cpu_count,
            memory: res.memory,
            time: res.time,
        }
    }
}

impl RequestedResources {
    pub fn new(cpu_count: u32, memory: u64, time: u32) -> Self {
        Self {
            cpu_count,
            memory,
            time,
        }
    }
}

/// Available Resources on a worker node.
#[derive(Clone, Debug)]
pub struct NodeResources {
    pub cpu_count: u32,
    pub memory: u64,
}

impl NodeResources {
    pub fn new(cpu_count: u32, memory: u64) -> Self {
        Self { cpu_count, memory }
    }

    pub fn empty() -> Self {
        Self {
            cpu_count: 0,
            memory: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum JobStatus {
    Completed,
    Failed,
    Pending,
    Running,
    Timeout,
}

impl From<JobStatus> for proto::JobStatus {
    fn from(status: JobStatus) -> Self {
        match status {
            JobStatus::Completed => proto::JobStatus::Completed,
            JobStatus::Failed => proto::JobStatus::Failed,
            JobStatus::Pending => proto::JobStatus::Pending,
            JobStatus::Running => proto::JobStatus::Running,
            JobStatus::Timeout => proto::JobStatus::Timeout,
        }
    }
}

impl From<JobStatus> for i32 {
    fn from(status: JobStatus) -> Self {
        let status = proto::JobStatus::from(status);
        status.into()
    }
}

impl From<i32> for JobStatus {
    fn from(value: i32) -> Self {
        match value {
            x if x == proto::JobStatus::Completed as i32 => JobStatus::Completed,
            x if x == proto::JobStatus::Failed as i32 => JobStatus::Failed,
            x if x == proto::JobStatus::Pending as i32 => JobStatus::Pending,
            x if x == proto::JobStatus::Running as i32 => JobStatus::Running,
            x if x == proto::JobStatus::Timeout as i32 => JobStatus::Timeout,
            _ => panic!("Invalid JobStatus value: {}", value),
        }
    }
}

impl From<proto::JobStatus> for JobStatus {
    fn from(status: proto::JobStatus) -> Self {
        match status {
            proto::JobStatus::Completed => JobStatus::Completed,
            proto::JobStatus::Failed => JobStatus::Failed,
            proto::JobStatus::Pending => JobStatus::Pending,
            proto::JobStatus::Running => JobStatus::Running,
            proto::JobStatus::Timeout => JobStatus::Timeout,
        }
    }
}

impl From<JobStatus> for String {
    fn from(status: JobStatus) -> Self {
        match status {
            JobStatus::Completed => "Completed".to_string(),
            JobStatus::Failed => "Failed".to_string(),
            JobStatus::Pending => "Pending".to_string(),
            JobStatus::Running => "Running".to_string(),
            JobStatus::Timeout => "Timeout".to_string(),
        }
    }
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
    fn from(result: JobResult) -> Self {
        proto::JobResult {
            job_id: result.id,
            status: (proto::JobStatus::from(result.status)).into(),
        }
    }
}

impl From<proto::JobResult> for JobResult {
    fn from(result: proto::JobResult) -> Self {
        JobResult {
            id: result.job_id,
            status: JobStatus::from(result.status),
        }
    }
}

impl From<&proto::JobResult> for JobResult {
    fn from(result: &proto::JobResult) -> Self {
        JobResult {
            id: result.job_id,
            status: JobStatus::from(result.status),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn job_conversion_roundtrip(id in 0u64.., user in ".*", script_path in ".*",
            script_args in proptest::collection::vec(".*", 0..10),
            cpu_count in 1u32..16, memory in 0u64..(1 << 30), time in 0u32..) {
            let req_res = RequestedResources::new(cpu_count, memory, time);
            let job = Job::new(id, user, script_path, script_args, req_res);

            let proto_job: proto::Job = (&job).into();
            let converted_job: Job = (&proto_job).into();

            assert_eq!(job.id, converted_job.id);
            assert_eq!(job.user, converted_job.user);
            assert_eq!(job.script_path, converted_job.script_path);
            assert_eq!(job.script_args, converted_job.script_args);
            assert_eq!(job.req_res.cpu_count, converted_job.req_res.cpu_count);
            assert_eq!(job.req_res.memory, converted_job.req_res.memory);
            assert_eq!(job.req_res.time, converted_job.req_res.time);
        }

        #[test]
        fn resource_reduction_and_free(cpu_count in 1u32..16, memory in 0u64..(1 << 30), time in 0u32..) {
            let mut node = Node::new("node-1".to_string(), "127.0.0.1".to_string(),
            NodeResources::new(cpu_count, memory), NodeStatus::Available);

            let req_res = RequestedResources::new(cpu_count / 2, memory / 2, time);
            node.reduce_avail_resources(&req_res);

            assert!(node.used_resources.cpu_count <= node.avail_resources.cpu_count);
            assert!(node.used_resources.memory <= node.avail_resources.memory);

            node.free_avail_resource(&req_res);

            assert_eq!(node.used_resources.cpu_count, 0);
            assert_eq!(node.used_resources.memory, 0);
        }
    }
}
