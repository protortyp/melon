use crate::arg::Args;
use crate::core_mask::CoreMask;
#[cfg(feature = "cgroups")]
use cgroups::CGroups;
use dashmap::DashMap;
use melon_common::proto::melon_scheduler_client::MelonSchedulerClient;
use melon_common::proto::melon_worker_server::{MelonWorker, MelonWorkerServer};
use melon_common::proto::{self, NodeInfo, NodeResources};
use melon_common::{log, JobResult, JobStatus};
use std::net::SocketAddr;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::System;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, watch, Mutex, Notify};
use tokio::task::JoinHandle;
use tokio::time::{interval, Instant};
use tonic::transport::Server;

#[derive(Debug, Clone)]
pub struct Worker {
    /// The unique worker ID assigned by the master node
    id: Option<String>,

    /// Internal server port
    port: u16,

    /// Endpoint of the master node/scheduler
    endpoint: String,

    /// Current connection status to the master node
    status: ConnectionStatus,

    /// Notifier to signal the server thread to shut down
    server_notifier: watch::Sender<()>,

    /// Handle to the heartbeat thread for lifecycle management
    ///
    /// Used to:
    /// - Keep track of the heartbeat thread
    /// - Gracefully shut down the heartbeat mechanism
    heartbeat_handle: Option<Arc<Mutex<JoinHandle<()>>>>,

    /// Notifier to signal the heartbeat thread to stop
    heartbeat_notifier: Arc<Notify>,

    /// Map of currently running jobs
    ///
    /// Key: Job ID
    /// Value: Handle to the job's execution thread
    running_jobs: Arc<DashMap<u64, JoinHandle<JobResult>>>,

    /// Handle to the job polling thread for lifecycle management
    ///
    /// Used to:
    /// - Keep track of the polling thread
    /// - Gracefully shut down the polling mechanism
    polling_handle: Option<Arc<Mutex<JoinHandle<()>>>>,

    /// Notifier to signal the polling thread to stop
    polling_notifier: Arc<Notify>,

    /// Map of deadline extension notifiers for running jobs
    ///
    /// Key: Job ID
    /// Value: Channel to send deadline extensions
    deadline_notifiers: Arc<DashMap<u64, mpsc::Sender<Duration>>>,

    /// CoreMask for managing CPU core allocation
    ///
    /// Represents the available CPU cores on the worker node.
    /// It's used to:
    /// - Allocate cores to new jobs
    /// - Track which cores are in use
    /// - Free cores when jobs complete
    core_mask: Arc<Mutex<CoreMask>>,

    /// Map of job-specific core masks
    ///
    /// Key: Job ID
    /// Value: Bitmask representing the cores allocated to the job
    job_masks: Arc<DashMap<u64, u64>>,
}

impl Drop for Worker {
    #[tracing::instrument(level = "info", name = "Shut down mworker", skip(self))]
    fn drop(&mut self) {
        if let Some(_handle) = &self.heartbeat_handle {
            log!(info, "Cleaning up heartbeat thread");
            self.heartbeat_notifier.notify_one();
        }

        // stop job polling thread
        if let Some(_handle) = &self.polling_handle {
            log!(info, "Cleaning up polling thread");
            self.polling_notifier.notify_one();
        }

        // stop server thread
        log!(info, "Cleaning up server thread");
        let _ = self.server_notifier.send(());
    }
}

#[derive(Debug, Clone)]
enum ConnectionStatus {
    Connected,
    Disconnected,
}

impl Worker {
    #[tracing::instrument(level = "info", name = "Build new worker...", skip(args))]
    pub fn new(args: &Args) -> Result<Self, Box<dyn std::error::Error>> {
        let endpoint = format!("http://{}", args.api_endpoint);
        let (server_notifier, _server_notifier_rx) = watch::channel(());

        let total_cores = num_cpus::get(); // cpuset considers logical cores
        let core_mask = Arc::new(Mutex::new(CoreMask::new(total_cores as u32)));
        let job_masks = Arc::new(DashMap::new());

        log!(info, "Set up worker with {} logical cores", total_cores);

        Ok(Self {
            id: None,
            status: ConnectionStatus::Disconnected,
            port: args.port,
            endpoint,
            heartbeat_handle: None,
            heartbeat_notifier: Arc::new(Notify::new()),
            server_notifier,
            running_jobs: Arc::new(DashMap::new()),
            polling_handle: None,
            polling_notifier: Arc::new(Notify::new()),
            deadline_notifiers: Arc::new(DashMap::new()),
            core_mask,
            job_masks,
        })
    }

    #[tracing::instrument(level = "info", name = "Start polling" skip(self))]
    pub async fn start_polling(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let worker = self.clone();
        let notifier = self.polling_notifier.clone();

        let handle = tokio::spawn(async move {
            let span = tracing::span!(tracing::Level::INFO, "Polling thread");
            let _guard = span.enter();

            let mut interval = interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = worker.poll_jobs().await {
                            log!(error, "Error polling jobs: {:?}", e);
                        }
                    }
                    _ = notifier.notified() => {
                        log!(info, "Polling task stopping.");
                        return;
                    }
                }
            }
        });

        let handle = Some(Arc::new(Mutex::new(handle)));
        self.polling_handle = handle;
        Ok(())
    }

    /// Checks for finished jobs
    ///
    /// If there are any finished jobs, submit the job result to the
    /// master node and remove the job from our internal data structure.
    ///
    /// # TODOS
    ///
    /// - [ ] handle timeouts when sending the result to the master
    #[tracing::instrument(level = "debug", name = "Poll jobs" skip(self))]
    async fn poll_jobs(&self) -> Result<(), Box<dyn std::error::Error>> {
        let endpoint = self.endpoint.clone();
        let jobs = self.running_jobs.clone();
        let mut completed_jobs = Vec::new();
        for entry in jobs.iter_mut() {
            let job_id = *entry.key();
            let handle = entry.value();
            if handle.is_finished() {
                log!(info, "JOB ID is finished {}", job_id);
                completed_jobs.push(job_id);
            }
        }

        for &job_id in &completed_jobs {
            if let Some((_, handle)) = jobs.remove(&job_id) {
                match handle.await {
                    Ok(result) => {
                        log!(info, "Received job result {:?}", result);

                        // send the update to the server
                        let mut client = MelonSchedulerClient::connect(endpoint.clone()).await?;
                        let request = tonic::Request::new(result.into());
                        // FIXME: handle timeouts and disconnects
                        let _res = client.submit_job_result(request).await?;
                    }
                    Err(e) => {
                        log!(error, "Job execution failed: {}", e);
                        let status = JobStatus::Failed;
                        let result = JobResult::new(job_id, status);
                        let mut client = MelonSchedulerClient::connect(endpoint.clone()).await?;
                        let request = tonic::Request::new(result.into());
                        // FIXME: handle timeouts and disconnects
                        let _res = client.submit_job_result(request).await?;
                    }
                }
            }
        }

        // remove the notifiers
        for &job_id in &completed_jobs {
            if self.deadline_notifiers.remove(&job_id).is_some() {
                log!(info, "Remove deadline notifier for {}", job_id);
            }
        }

        Ok(())
    }

    #[tracing::instrument(level = "info", name = "Register node at daemon" skip(self))]
    pub async fn register_node(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log!(info, "Register node at master at {}", self.endpoint);
        let mut client = MelonSchedulerClient::connect(self.endpoint.clone().to_string()).await?;
        let resources = get_node_resources();
        let req = NodeInfo {
            address: format!("http://[::1]:{}", self.port),
            resources: Some(resources),
        };
        let request = tonic::Request::new(req);
        let res = client.register_node(request).await?;
        let res = res.get_ref();
        self.id = Some(res.node_id.clone());
        self.status = ConnectionStatus::Connected;
        Ok(())
    }

    #[tracing::instrument(level = "debug", name = "Start hearbeat loop" skip(self))]
    pub async fn start_heartbeats(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let worker = self.clone();
        let notifier = self.heartbeat_notifier.clone();
        let handle = tokio::spawn(async move {
            let span = tracing::span!(tracing::Level::INFO, "Heartbeat thread");
            let _guard = span.enter();

            // FIXME: hardocded timer
            let mut interval = interval(Duration::from_secs(10));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = worker.send_heartbeat().await {
                            log!(error, "Error sending heartbeat: {:?}", e);
                        }
                    }
                    _ = notifier.notified() => {
                        log!(info, "Heartbeat task stopping.");
                        return;
                    }
                }
            }
        });

        let handle = Some(Arc::new(Mutex::new(handle)));
        self.heartbeat_handle = handle;
        Ok(())
    }

    #[tracing::instrument(level = "debug", name = "Send heartbeat" skip(self))]
    async fn send_heartbeat(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = MelonSchedulerClient::connect(self.endpoint.clone().to_string()).await?;
        let node_id = self.id.clone().unwrap();
        let req = proto::Heartbeat { node_id };
        let req = tonic::Request::new(req);
        let _ = client.send_heartbeat(req).await?;
        Ok(())
    }

    #[tracing::instrument(level = "info", name = "Start worker server" skip(self))]
    pub async fn start_server(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let worker = self.clone();
        let mut shutdown_rx = self.server_notifier.subscribe();

        let address: SocketAddr = format!("[::1]:{}", worker.port).parse().unwrap();
        let server = Server::builder()
            .add_service(MelonWorkerServer::new(worker))
            .serve_with_shutdown(address, async {
                shutdown_rx.changed().await.ok();
            });

        if let Err(e) = server.await {
            log!(error, " Server error: {}", e);
        }
        Ok(())
    }

    /// Spawn a thread to work on a given job
    ///
    /// # Notes
    ///
    /// Returns the thread handler to the calling scope.
    #[tracing::instrument(level = "info", name = "Spawn new job" skip(self, job))]
    pub async fn spawn_job(
        &self,
        job: &proto::JobAssignment,
    ) -> Result<JoinHandle<JobResult>, Box<dyn std::error::Error>> {
        // spawn a new thread that works on the job
        let job_id = job.job_id;
        let (tx, mut rx) = mpsc::channel::<Duration>(10);
        self.deadline_notifiers.insert(job_id, tx);
        let initial_time_mins = job.req_res.expect("Could not get resources").time as u64;
        let pth = job.script_path.clone();
        let args = job.script_args.clone();
        let resources = job.req_res.unwrap();
        let cores_needed = resources.cpu_count;

        log!(
            info,
            "Spawn script at: {}, args: {:?}, resources: {:?}, cores needed: {}",
            pth,
            args,
            resources,
            cores_needed
        );

        let allocated_mask = {
            let mut core_mask = self.core_mask.lock().await;
            core_mask.allocate(cores_needed).ok_or_else(|| {
                log!(error, "Resources are exhausted!");
                tonic::Status::resource_exhausted("Not enough cores available")
            })?
        };
        // store allocated mask
        self.job_masks.insert(job_id, allocated_mask);

        let core_mask = self.core_mask.clone();
        let job_masks = self.job_masks.clone();
        let handle = tokio::spawn(async move {
            let span = tracing::span!(tracing::Level::INFO, "Spawn jobs result listener");
            let _guard = span.enter();

            // let cgroup = Arc::new(Mutex::new(None));
            // let cgroup_clone = Arc::clone(&cgroup);

            let mut child = match Command::new(&pth)
                .args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(child) => child,
                Err(e) => {
                    log!(error, "Could not spawn command {}", e);
                    return JobResult::new(job_id, JobStatus::Failed);
                }
            };

            #[cfg(feature = "cgroups")]
            let child_pid = match child.id() {
                Some(id) => id,
                None => return JobResult::new(job_id, JobStatus::Failed),
            };

            #[cfg(feature = "cgroups")]
            let core_string = CoreMask::mask_to_string(allocated_mask);

            #[cfg(feature = "cgroups")]
            let cgroup = match CGroups::build()
                .name(&format!("melon_{}", child_pid))
                .with_cpu(&core_string)
                .with_memory(resources.memory)
                .build()
            {
                Ok(group) => group,
                Err(e) => {
                    log!(
                        error,
                        "Could not build cgroup for job {} on process id {} due to error {}",
                        job_id,
                        child_pid,
                        e.to_string()
                    );
                    return JobResult::new(job_id, JobStatus::Failed);
                }
            };

            #[cfg(feature = "cgroups")]
            if let Err(e) = cgroup.create() {
                log!(
                    error,
                    "Could not create cgroup for job {} on process id {} due to error {}",
                    job_id,
                    child_pid,
                    e.to_string()
                );
                return JobResult::new(job_id, JobStatus::Failed);
            }

            let mut deadline = Instant::now() + Duration::from_secs(initial_time_mins * 60);
            let mut stdout = BufReader::new(child.stdout.take().unwrap());
            let mut stderr = BufReader::new(child.stderr.take().unwrap());

            let mut stdout_buf = String::new();
            let mut stderr_buf = String::new();

            loop {
                tokio::select! {
                    status_result = child.wait() => {
                        log!(info, "Got child result!");
                        // read the segments
                        stdout.read_to_string(&mut stdout_buf).await.unwrap_or_else(|e| {
                            log!(error, "Failed to read stdout: {}", e);
                            0
                        });
                        stderr.read_to_string(&mut stderr_buf).await.unwrap_or_else(|e| {
                            log!(error, "Failed to read stderr: {}", e);
                            0
                        });


                        {
                            // free up core mask
                            if let Some((_, mask)) = job_masks.remove(&job_id) {
                                let mut core_mask = core_mask.lock().await;
                                core_mask.free(mask);
                            }
                        }

                        match status_result {
                            Ok(status) => {
                                if status.success() {
                                    // capture the output
                                    log!(info, "Job was a success");
                                    return JobResult::new(job_id, JobStatus::Completed);
                                } else {
                                    // capture error output
                                    let error_msg = format!("Process exited with status: {}. Stderr: {}", status, stderr_buf);
                                    log!(info, "Job was not successfull: {}", error_msg);
                                    return JobResult::new(job_id, JobStatus::Failed);
                                }
                            },
                            Err(_) => {
                                log!(error, "Something wrong with the result!");
                                return JobResult::new(job_id, JobStatus::Failed);
                            }
                        }
                    },
                    _ = tokio::time::sleep_until(deadline) => {
                        log!(info, "Deadline hit! Start cancel");
                        // reached timeout deadline
                        if let Err(e) = child.kill().await {
                            log!(error, "Failed to kill process: {}", e);
                        }
                        return JobResult::new(job_id, JobStatus::Timeout);
                    },
                    Some(extension) = rx.recv() => {
                        // extend the deadline
                        log!(info, "Receive deadline extension for job by {} minutes", extension.as_secs() / 60);
                        deadline += extension;
                    }
                }
            }
        });

        Ok(handle)
    }
}

fn get_node_resources() -> NodeResources {
    let mut system = System::new_all();
    system.refresh_all();

    let cpu_count = system.cpus().len() as u32;
    let memory = system.total_memory() * 1024;
    NodeResources { cpu_count, memory }
}

#[tonic::async_trait]
impl MelonWorker for Worker {
    /// Receive a job from the master node
    #[tracing::instrument(level = "info", name = "Get job assignment" skip(self,request))]
    async fn assign_job(
        &self,
        request: tonic::Request<proto::JobAssignment>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let handle = self
            .spawn_job(request.get_ref())
            .await
            .expect("Could not spawn job task");
        self.running_jobs.insert(request.get_ref().job_id, handle);

        let res = tonic::Response::new(());
        Ok(res)
    }

    #[tracing::instrument(level = "info", name = "Get job cancellation request" skip(self,request))]
    async fn cancel_job(
        &self,
        request: tonic::Request<proto::CancelJobRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let req = request.get_ref();
        let id = req.job_id;
        if let Some((_, handle)) = self.running_jobs.remove(&id) {
            // if job is not finished, cancel the job first
            if !handle.is_finished() {
                handle.abort();
            }

            // free the cores
            let mut core_mask = self.core_mask.lock().await;
            if let Some((_, mask)) = self.job_masks.remove(&id) {
                core_mask.free(mask);
            }
            return Ok(tonic::Response::new(()));
        }

        Err(tonic::Status::not_found("Not found!"))
    }
    #[tracing::instrument(level = "info", name = "Get job extension request" skip(self,request))]
    async fn extend_job(
        &self,
        request: tonic::Request<proto::ExtendJobRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let req = request.get_ref();
        let id = req.job_id;
        let time_in_mins = req.extension_mins;
        if let Some(tx) = self.deadline_notifiers.get(&id) {
            match tx.send(Duration::from_secs(time_in_mins as u64 * 60)).await {
                Ok(_) => {
                    log!(info, "Successfully sent the job extension request");
                    Ok(tonic::Response::new(()))
                }
                Err(e) => Err(tonic::Status::internal(format!(
                    "Failed to send extension request: {}",
                    e
                ))),
            }
        } else {
            Err(tonic::Status::not_found("Job ID not found"))
        }
    }
}
