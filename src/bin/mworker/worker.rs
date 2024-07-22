use crate::arg::Args;
use crate::cgroups::CGroups;
use melon::proto::melon_scheduler_client::MelonSchedulerClient;
use melon::proto::melon_worker_server::{MelonWorker, MelonWorkerServer};
use melon::proto::{self, NodeInfo, NodeResources};
use melon::{JobResult, JobStatus};
use std::collections::HashMap;
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

    /// Daemon endpoint
    endpoint: String,

    /// Connection Status
    status: ConnectionStatus,

    /// Server thread handler
    pub server_handle: Option<Arc<Mutex<JoinHandle<()>>>>,

    /// Server thread notifier
    server_notifier: watch::Sender<()>,

    /// Heartbeat thread handler
    heartbeat_handle: Option<Arc<Mutex<JoinHandle<()>>>>,

    /// Heartbeat thread notifier
    heartbeat_notifier: Arc<Notify>,

    /// Running jobs
    running_jobs: Arc<Mutex<HashMap<u64, JoinHandle<JobResult>>>>,

    /// Polling thread handler
    polling_handle: Option<Arc<Mutex<JoinHandle<()>>>>,

    /// Polling notifier
    polling_notifier: Arc<Notify>,

    /// Deadline notifiers
    deadline_notifiers: Arc<Mutex<HashMap<u64, mpsc::Sender<Duration>>>>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        // stop heartbeat thread
        if let Some(_handle) = &self.heartbeat_handle {
            println!("Cleaning up heartbeat thread...");
            self.heartbeat_notifier.notify_one();
        }

        // stop job polling thread
        if let Some(_handle) = &self.polling_handle {
            println!("Cleaning up heartbeat thread...");
            self.polling_notifier.notify_one();
        }

        // stop server thread
        println!("Cleaning up server thread...");
        let _ = self.server_notifier.send(());

        // todo: abort all running jobs
    }
}

#[derive(Debug, Clone)]
enum ConnectionStatus {
    Connected,
    Disconnected,
}

impl Worker {
    pub fn new(args: &Args) -> Result<Self, Box<dyn std::error::Error>> {
        let endpoint = format!("http://{}", args.api_endpoint);
        let (server_notifier, _server_notifier_rx) = watch::channel(());

        Ok(Self {
            id: None,
            status: ConnectionStatus::Disconnected,
            port: args.port,
            endpoint,
            heartbeat_handle: None,
            heartbeat_notifier: Arc::new(Notify::new()),
            server_handle: None,
            server_notifier,
            running_jobs: Arc::new(Mutex::new(HashMap::new())),
            polling_handle: None,
            polling_notifier: Arc::new(Notify::new()),
            deadline_notifiers: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn start_polling(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let worker = self.clone();
        let notifier = self.polling_notifier.clone();

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = worker.poll_jobs().await {
                            eprintln!("Error polling jobs: {:?}", e);
                        }
                    }
                    _ = notifier.notified() => {
                        println!("Polling task stopping.");
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
    async fn poll_jobs(&self) -> Result<(), Box<dyn std::error::Error>> {
        let endpoint = self.endpoint.clone();
        let mut jobs = self.running_jobs.lock().await;

        let mut completed_jobs = Vec::new();
        for (job_id, handle) in jobs.iter_mut() {
            if handle.is_finished() {
                completed_jobs.push(*job_id);
            }
        }

        for &job_id in &completed_jobs {
            if let Some(handle) = jobs.remove(&job_id) {
                match handle.await {
                    Ok(result) => {
                        // send the update to the server
                        let mut client = MelonSchedulerClient::connect(endpoint.clone()).await?;
                        let request = tonic::Request::new(result.into());
                        // FIXME: handle timeouts and disconnects
                        let _res = client.submit_job_result(request).await?;
                    }
                    Err(e) => {
                        let status = JobStatus::Failed(e.to_string());
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
        let mut notifiers = self.deadline_notifiers.lock().await;
        for &job_id in &completed_jobs {
            if notifiers.remove(&job_id).is_some() {
                println!("Remove deadline notifier for {}", job_id);
            }
        }

        Ok(())
    }

    pub async fn register_node(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Register node at master at {}", self.endpoint);
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

    pub async fn start_heartbeats(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Start sending heartbeats...");
        let worker = self.clone();
        let notifier = self.heartbeat_notifier.clone();
        let handle = tokio::spawn(async move {
            // FIXME: hardocded timer
            let mut interval = interval(Duration::from_secs(10));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = worker.send_heartbeat().await {
                            eprintln!("Error sending heartbeat: {:?}", e);
                        }
                    }
                    _ = notifier.notified() => {
                        println!("Heartbeat task stopping.");
                        return;
                    }
                }
            }
        });

        let handle = Some(Arc::new(Mutex::new(handle)));
        self.heartbeat_handle = handle;
        Ok(())
    }

    async fn send_heartbeat(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Send heartbeat!");
        let mut client = MelonSchedulerClient::connect(self.endpoint.clone().to_string()).await?;
        let node_id = self.id.clone().unwrap();
        let req = proto::Heartbeat { node_id };
        let req = tonic::Request::new(req);
        let res = client.send_heartbeat(req).await?;
        let _ = res.get_ref().ack;
        Ok(())
    }

    pub async fn start_server(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Start worker server...");
        let worker = self.clone();
        let mut shutdown_rx = self.server_notifier.subscribe();

        let handle = tokio::spawn(async move {
            let address: SocketAddr = format!("[::1]:{}", worker.port).parse().unwrap();
            let server = Server::builder()
                .add_service(MelonWorkerServer::new(worker))
                .serve_with_shutdown(address, async {
                    shutdown_rx.changed().await.ok();
                });

            if let Err(e) = server.await {
                eprintln!(" > Server error: {}", e);
            }
        });

        let handle = Some(Arc::new(Mutex::new(handle)));
        self.server_handle = handle;
        Ok(())
    }

    /// Spawn a thread to work on a given job
    ///
    /// # Notes
    ///
    /// Returns the thread handler to the calling scope.
    pub async fn spawn_job(
        &self,
        job: &proto::JobAssignment,
    ) -> Result<JoinHandle<JobResult>, Box<dyn std::error::Error>> {
        // spawn a new thread that works on the job
        println!("Spawn job handler");

        let job_id = job.job_id;
        let (tx, mut rx) = mpsc::channel::<Duration>(10);
        self.deadline_notifiers.lock().await.insert(job_id, tx);
        let initial_time_mins = job.req_res.unwrap().time as u64;

        let pth = job.script_path.clone();
        let args = job.script_args.clone();
        let resources = job.req_res.unwrap();

        let handle = tokio::spawn(async move {
            let pid = std::process::id();
            let _cgroup_guard = CGroups::create_group_guard(job_id, pid, resources).unwrap();

            let mut child = match Command::new(&pth)
                .args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(child) => child,
                Err(e) => return JobResult::new(job_id, JobStatus::Failed(e.to_string())),
            };

            let mut deadline = Instant::now() + Duration::from_secs(initial_time_mins * 60);
            let mut stdout = BufReader::new(child.stdout.take().unwrap());
            let mut stderr = BufReader::new(child.stderr.take().unwrap());

            let mut stdout_buf = String::new();
            let mut stderr_buf = String::new();

            loop {
                tokio::select! {
                    status_result = child.wait() => {
                        // read the segments
                        stdout.read_to_string(&mut stdout_buf).await.unwrap_or_else(|e| {
                            eprintln!("Failed to read stdout: {}", e);
                            0
                        });
                        stderr.read_to_string(&mut stderr_buf).await.unwrap_or_else(|e| {
                            eprintln!("Failed to read stderr: {}", e);
                            0
                        });


                        match status_result {
                            Ok(status) => {
                                if status.success() {
                                    // capture the output
                                    return JobResult::new(job_id, JobStatus::Completed);
                                } else {
                                    // capture error output
                                    let error_msg = format!("Process exited with status: {}. Stderr: {}", status, stderr_buf);
                                    return JobResult::new(job_id, JobStatus::Failed(error_msg));
                                }
                            },
                            Err(e) => {
                                return JobResult::new(job_id, JobStatus::Failed(e.to_string()));
                            }
                        }
                    },
                    _ = tokio::time::sleep_until(deadline) => {
                        // reached timeout deadline
                        if let Err(e) = child.kill().await {
                            eprintln!("Failed to kill process: {}", e);
                        }
                        return JobResult::new(job_id, JobStatus::Timeout);
                    },
                    Some(extension) = rx.recv() => {
                        // extend the deadline
                        println!("Receive deadline extension for job by {} minutes", extension.as_secs() / 60);
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
    async fn assign_job(
        &self,
        request: tonic::Request<proto::JobAssignment>,
    ) -> Result<tonic::Response<proto::WorkerJobResponse>, tonic::Status> {
        println!("Receive job submission");

        let handle = self
            .spawn_job(request.get_ref())
            .await
            .expect("Could not spawn job task");
        let mut jobs = self.running_jobs.lock().await;
        jobs.insert(request.get_ref().job_id, handle);

        let res = proto::WorkerJobResponse { ack: true };
        let res = tonic::Response::new(res);
        Ok(res)
    }

    async fn cancel_job(
        &self,
        request: tonic::Request<proto::CancelJobRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        println!("Receive cancel job request");
        let req = request.get_ref();
        let id = req.job_id;
        let mut running_jobs = self.running_jobs.lock().await;
        if let Some(handle) = running_jobs.get(&id) {
            // if job is not finished, cancel the job first
            if !handle.is_finished() {
                handle.abort();
            }
            running_jobs.remove(&id);
            return Ok(tonic::Response::new(()));
        }

        Err(tonic::Status::not_found("Not found!"))
    }

    async fn extend_job(
        &self,
        request: tonic::Request<proto::ExtendJobRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        println!("Receive job extension request");
        let req = request.get_ref();
        let id = req.job_id;
        let time_in_mins = req.extension_mins;
        let notifiers = self.deadline_notifiers.lock().await;
        if let Some(tx) = notifiers.get(&id) {
            match tx.send(Duration::from_secs(time_in_mins as u64 * 60)).await {
                Ok(_) => {
                    println!("Successfully sent the request via channels");
                    return Ok(tonic::Response::new(()));
                }
                Err(e) => return Err(tonic::Status::unknown(format!("Unkown error {}", e))),
            }
        }

        Err(tonic::Status::not_found("Job ID not found"))
    }
}
