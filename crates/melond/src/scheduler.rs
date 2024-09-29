use crate::db::DatabaseHandler;
use crate::error::Result;
use crate::settings::Settings;
use melon_common::proto::melon_scheduler_server::MelonScheduler;
use melon_common::proto::melon_worker_client::MelonWorkerClient;
use melon_common::utils::get_current_timestamp;
use melon_common::{log, proto, JobResult, JobStatus, RequestedResources};
use melon_common::{Job, Node, NodeStatus};
use nanoid::nanoid;
use std::time::Duration;
use std::time::Instant;
use std::{
    collections::{HashMap, VecDeque},
    sync::{atomic::AtomicU64, Arc},
};
use tokio::sync::mpsc::Sender;
use tokio::sync::{mpsc, Mutex, Notify};
use tokio::task::JoinHandle;
use tokio::time::interval;
use tonic::Status;

#[derive(Clone, Debug)]
pub struct Scheduler {
    /// Atomic counter for generating unique job IDs
    ///
    /// Used to:
    /// - Assign unique IDs to new jobs
    /// - Ensure job IDs are monotonically increasing
    /// - Initialized based on the highest job ID from the db
    job_ctr: Arc<AtomicU64>,

    /// Map of available worker nodes
    ///
    /// Key: Node ID
    /// Value: Node information
    nodes: Arc<Mutex<HashMap<String, Node>>>,

    /// Map of currently running jobs
    ///
    /// Key: Job ID
    /// Value: Job information
    running_jobs: Arc<Mutex<HashMap<u64, Job>>>,

    /// Queue of pending jobs waiting to be assigned to workers
    ///
    /// Jobs are processed in FIFO order
    pending_jobs: Arc<Mutex<VecDeque<Job>>>,

    /// Handle to the job scheduling thread for lifecycle management
    ///
    /// Used to:
    /// - Keep track of the scheduling thread
    /// - Gracefully shut down the scheduling mechanism
    handle: Option<Arc<Mutex<JoinHandle<()>>>>,

    /// Notifier to signal the scheduling thread to stop
    notifier: Arc<Notify>,

    /// Handle to the node health check thread for lifecycle management
    ///
    /// Used to:
    /// - Keep track of the health check thread
    /// - Gracefully shut down the health check mechanism
    health_handle: Option<Arc<Mutex<JoinHandle<()>>>>,

    /// Notifier to signal the health check thread to stop
    health_notifier: Arc<Notify>,

    /// Handler for database operations
    db: Arc<DatabaseHandler>,

    /// Channel sender for asynchronous database write operations
    db_tx: Arc<Sender<Job>>,
}

impl Drop for Scheduler {
    #[tracing::instrument(level = "debug", name = "Shut down scheduler...", skip(self))]
    fn drop(&mut self) {
        // stop heartbeat thread
        if let Some(_handle) = &self.handle {
            self.notifier.notify_one();
        }

        // stop node health thread
        if let Some(_handle) = &self.health_handle {
            self.health_notifier.notify_one();
        }

        // clear all pending jobs or save them to file
        // + abort all running jobs

        // shutdown db_writer
        self.db.shutdown();
    }
}

impl Scheduler {
    pub fn new(settings: &Settings) -> Self {
        // Spawn Database Writer
        let (db_tx, db_rx) = mpsc::channel::<Job>(100);
        let mut db_writer =
            DatabaseHandler::new(db_rx, &settings.database).expect("Could not init database write");
        db_writer.run().expect("Could not start database writer");
        let db_writer = Arc::new(db_writer);
        let db_tx = Arc::new(db_tx);

        let highest_job_id = db_writer
            .get_highest_job_id()
            .expect("Could not get highest job ID from database");

        let job_ctr = Arc::new(AtomicU64::new(highest_job_id + 1));

        Self {
            job_ctr,
            nodes: Arc::new(Mutex::new(HashMap::new())),
            running_jobs: Arc::new(Mutex::new(HashMap::new())),
            pending_jobs: Arc::new(Mutex::new(VecDeque::new())),
            handle: None,
            notifier: Arc::new(Notify::new()),
            health_handle: None,
            health_notifier: Arc::new(Notify::new()),
            db: db_writer,
            db_tx,
        }
    }

    /// Starts a dedicated task that periodically scans for pending jobs
    /// and assigns them to available workers. This function ensures efficient job
    /// distribution by continuously monitoring the job queue and worker availability.
    #[tracing::instrument(level = "debug", name = "Start up scheduler", skip(self))]
    pub async fn start(&mut self) -> Result<()> {
        let scheduler = self.clone();
        let notifier = self.notifier.clone();

        let handle = tokio::spawn(async move {
            let span = tracing::span!(tracing::Level::DEBUG, "Spawn pending jobs listener");
            let _guard = span.enter();

            // FIXME: hardocded timer
            let mut interval = interval(Duration::from_millis(250));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let mut pending_jobs = scheduler.pending_jobs.lock().await;

                        let mut to_remove = vec![];

                        // assign jobs to nodes if they're available
                        for (index, job) in pending_jobs.iter_mut().enumerate() {
                            // log!(info, "Check job {}", index);
                            if let Some(node_id) = scheduler.find_available_node(&job.req_res).await {
                                let mut nodes = scheduler.nodes.lock().await;
                                let node = nodes.get_mut(&node_id).unwrap();

                                // submit the job to the node
                                // FIXME: handle fails
                                if let Ok(mut client) = MelonWorkerClient::connect(node.endpoint.clone()).await{
                                    let req = tonic::Request::new(job.into());
                                    // if it worked, reduce the available resources
                                    if (client.assign_job(req).await).is_ok() {
                                        // submission was successful => compute node started working
                                        // reduce the available compute resources of the node
                                        node.reduce_avail_resources(&job.req_res);

                                        // set the node id of the job
                                        job.assigned_node = Some(node_id);

                                        // mark the job for removal
                                        to_remove.push(index);

                                    }
                                }
                            }
                        }

                        // move submitted jobs to running jobs list
                        let mut running_jobs = scheduler.running_jobs.lock().await;
                        for index in to_remove.iter().rev() {
                            let mut job = pending_jobs.remove(*index).expect("Job should exist");
                            job.start_time = Some(get_current_timestamp());
                            job.status = JobStatus::Running;
                            let job_id = job.id;

                            running_jobs.insert(job_id, job);
                        }
                    }

                    _ = notifier.notified() => {
                        log!(info, "Stopping scheduler job assignment tasks...");
                        return;
                    }
                }
            }
        });

        let handle = Some(Arc::new(Mutex::new(handle)));
        self.handle = handle;
        Ok(())
    }

    #[tracing::instrument(level = "debug", name = "Start health polling", skip(self))]
    pub async fn start_health_polling(&mut self) -> Result<()> {
        let scheduler = self.clone();
        let notifier = self.health_notifier.clone();

        let handle = tokio::spawn(async move {
            // FIXME: hardocded timer
            let mut interval = interval(Duration::from_secs(30));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = scheduler.poll_node_health().await {
                            log!(error,"Error polling node health: {:?}", e);
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
        self.health_handle = handle;
        Ok(())
    }

    /// Checks the health status of all registered compute nodes.
    /// Marks nodes as offline if they haven't sent a heartbeat in the last 60 seconds.
    #[tracing::instrument(level = "debug", name = "Poll node health", skip(self))]
    async fn poll_node_health(&self) -> Result<()> {
        // regularly check which compute nodes have not called back in a while
        // mark those nodes as unavailable
        let mut nodes = self.nodes.lock().await;
        for (_, node) in nodes.iter_mut() {
            let now = Instant::now();
            if now.duration_since(node.last_heartbeat) > Duration::from_secs(60) {
                node.status = NodeStatus::Offline;
            }
        }
        Ok(())
    }

    /// Finds an available node for a given resource requirement.
    #[tracing::instrument(
        level = "debug",
        name = "Find available node",
        skip(self),
        fields(
            cpu_count = %res.cpu_count,
            memory = %res.memory,
            time = %res.time
        )
    )]
    async fn find_available_node(&self, res: &RequestedResources) -> Option<String> {
        let nodes = self.nodes.lock().await;

        for (node_id, node) in nodes.iter() {
            // log!(info, "Check node_id {}", node_id);
            if node.status != NodeStatus::Available {
                continue;
            }

            let available_cpu = node
                .avail_resources
                .cpu_count
                .saturating_sub(node.used_resources.cpu_count);
            let available_memory = node
                .avail_resources
                .memory
                .saturating_sub(node.used_resources.memory);

            if available_cpu >= res.cpu_count && available_memory >= res.memory {
                return Some(node_id.clone());
            }
        }
        None
    }
}

#[tonic::async_trait]
impl MelonScheduler for Scheduler {
    #[tracing::instrument(level="debug", name = "Receive job submission", skip(self), fields(script_path = %request.get_ref().script_path))]
    async fn submit_job(
        &self,
        request: tonic::Request<proto::JobSubmission>,
    ) -> core::result::Result<tonic::Response<proto::MasterJobResponse>, tonic::Status> {
        log!(debug, "get job sub request");
        let sub = request.get_ref();

        // create new job
        let job_id = self
            .job_ctr
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let res = sub.req_res.expect("No resources given");
        let resources = res.into();
        let new_job = Job::new(
            job_id,
            sub.user.clone(),
            sub.script_path.clone(),
            sub.script_args.clone(),
            resources,
        );

        // push job to pending jobs queue
        let pending_jobs = self.pending_jobs.clone();
        let mut pending_jobs = pending_jobs.lock().await;
        pending_jobs.push_back(new_job); // FIFO

        // return created job id
        let response = proto::MasterJobResponse { job_id };
        log!(debug, "response. {:?}", response);
        Ok(tonic::Response::new(response))
    }

    /// Register a new node in a master.
    #[tracing::instrument(level="info", name = "Register new compute node", skip(self, request), fields(address = %request.get_ref().address))]
    async fn register_node(
        &self,
        request: tonic::Request<proto::NodeInfo>,
    ) -> core::result::Result<tonic::Response<proto::RegistrationResponse>, tonic::Status> {
        let req = request.get_ref();
        let resources = req.resources.unwrap();
        let resources = melon_common::NodeResources::new(resources.cpu_count, resources.memory);

        let id = nanoid!();
        let node = Node::new(
            id.clone(),
            req.address.clone(),
            resources,
            NodeStatus::Available,
        );
        let res = proto::RegistrationResponse {
            node_id: id.clone(),
        };
        let response = tonic::Response::new(res);

        let mut nodes = self.nodes.lock().await;
        nodes.insert(id, node);

        Ok(response)
    }

    #[tracing::instrument(level="debug", name = "Receive heartbeat", skip(self, request), fields(node_id = %request.get_ref().node_id))]
    async fn send_heartbeat(
        &self,
        request: tonic::Request<proto::Heartbeat>,
    ) -> core::result::Result<tonic::Response<()>, tonic::Status> {
        let mut nodes = self.nodes.lock().await;
        let node_id = &request.get_ref().node_id;

        match nodes.get_mut(node_id) {
            Some(node) => {
                // compute node is registered
                node.set_status(NodeStatus::Available);
                node.update_heartbeat();
            }
            None => {
                // compute node is not registered => reject
                return Err(tonic::Status::unauthenticated("Node is not registered"));
            }
        }

        let res = tonic::Response::new(());
        Ok(res)
    }

    #[tracing::instrument(level = "info", name = "Receive job results", skip(self, request))]
    async fn submit_job_result(
        &self,
        request: tonic::Request<proto::JobResult>,
    ) -> core::result::Result<tonic::Response<()>, tonic::Status> {
        let req = request.get_ref();
        let result: JobResult = req.into();

        let job_id = result.id;
        let mut jobs = self.running_jobs.lock().await;
        if let Some(job) = jobs.get(&result.id) {
            let res = &job.req_res;
            let node_id = job.assigned_node.as_ref().expect("Expect assigned node id");

            // free up resources from the compute node
            let mut nodes = self.nodes.lock().await;
            let node = nodes.get_mut(node_id).expect("Expect node to exist");
            node.free_avail_resource(res);

            // remove job from tracking map
            let mut job = jobs.remove(&job_id).unwrap();

            // send the finished job to the database writer for permanent storage
            job.stop_time = Some(get_current_timestamp());
            job.status = result.status;

            let tx = self.db_tx.clone();
            // FIXME: hardcoded timeout
            if let Err(e) = tx.send(job).await {
                log!(
                    error,
                    "Could not send job {} to database writer: {}",
                    job_id,
                    e
                );
            }

            // ack
            let res = tonic::Response::new(());
            Ok(res)
        } else {
            Err(tonic::Status::not_found("Job not found"))
        }
    }

    #[tracing::instrument(level = "debug", name = "List all jobs", skip(self, _request))]
    async fn list_jobs(
        &self,
        _request: tonic::Request<()>,
    ) -> core::result::Result<tonic::Response<proto::JobListResponse>, tonic::Status> {
        let pending_jobs = self.pending_jobs.lock().await;
        let running_jobs = self.running_jobs.lock().await;

        // Accumulate pending and running jobs
        let mut jobs: Vec<proto::Job> = pending_jobs.iter().map(|j| j.into()).collect();
        jobs.extend(running_jobs.values().map(|j| j.into()));

        // Fetch finished jobs from the database
        match self.db.get_all_jobs() {
            Ok(finished_jobs) => {
                jobs.extend(finished_jobs.iter().map(|j| j.into()));
            }
            Err(e) => {
                log!(error, "Error fetching finished jobs from database: {}", e);
                return Err(tonic::Status::internal("Failed to fetch finished jobs"));
            }
        }

        let response = proto::JobListResponse { jobs };
        let response = tonic::Response::new(response);
        Ok(response)
    }

    #[tracing::instrument(
        level = "info",
        name = "Receive cancellation request",
        skip(self, request),
        fields(job_id = %request.get_ref().job_id, user=%request.get_ref().user)
    )]
    async fn cancel_job(
        &self,
        request: tonic::Request<proto::CancelJobRequest>,
    ) -> core::result::Result<tonic::Response<()>, tonic::Status> {
        let req = request.get_ref();
        let id = req.job_id;
        let user = req.user.clone();

        // check in pending jobs
        let mut pending_jobs = self.pending_jobs.lock().await;
        if let Some(pos) = pending_jobs.iter().position(|job| job.id == id) {
            if pending_jobs[pos].user != user {
                return Err(Status::permission_denied(
                    "Not authorized to cancel this job",
                ));
            }
            pending_jobs.remove(pos);
            return Ok(tonic::Response::new(()));
        }

        // check in running jobs
        let mut running_jobs = self.running_jobs.lock().await;
        if let Some(job) = running_jobs.get(&id) {
            if job.user != user {
                return Err(Status::permission_denied(
                    "Not authorized to cancel this job",
                ));
            }

            // send cancellation request to the assigned node
            let node = &job.assigned_node.clone().unwrap();
            let mut nodes = self.nodes.lock().await;
            if let Some(node) = nodes.get_mut(node) {
                // send the cancellation request to the assigned node
                let mut client = MelonWorkerClient::connect(node.endpoint.clone())
                    .await
                    .map_err(|e| Status::unknown(format!("Error connecting to node: {}", e)))?;
                let worker_request = proto::CancelJobRequest {
                    job_id: id,
                    user: user.clone(),
                };

                client.cancel_job(worker_request).await?;

                // free up the node resources to mark availability
                let res = job.req_res;
                node.free_avail_resource(&res);
            }

            running_jobs.remove(&id);
            return Ok(tonic::Response::new(()));
        }

        // no job found
        Err(Status::not_found("Job not found"))
    }

    #[tracing::instrument(
        level = "info",
        name = "Receive time extension request",
        skip(self, request),
        fields(job_id = %request.get_ref().job_id, user=%request.get_ref().user, extension_mins=%request.get_ref().extension_mins)
    )]
    async fn extend_job(
        &self,
        request: tonic::Request<proto::ExtendJobRequest>,
    ) -> core::result::Result<tonic::Response<()>, tonic::Status> {
        let req = request.get_ref();
        let id = req.job_id;
        let user = req.user.clone();
        let time_in_mins = req.extension_mins;

        // first check the pending jobs
        let mut pending_jobs = self.pending_jobs.lock().await;
        if let Some(pos) = pending_jobs.iter().position(|job| job.id == id) {
            if pending_jobs[pos].user != user {
                return Err(Status::permission_denied(
                    "Not authorized to cancel this job",
                ));
            }

            // adjust the deadline
            let job = pending_jobs.get_mut(pos).expect("exists for sure");
            job.req_res.time += time_in_mins;

            return Ok(tonic::Response::new(()));
        }

        // check running jobs
        let mut running_jobs = self.running_jobs.lock().await;
        if let Some(job) = running_jobs.get_mut(&id) {
            if job.user != user {
                return Err(Status::permission_denied(
                    "Not authorized to cancel this job",
                ));
            }

            let node = &job.assigned_node.clone().unwrap();
            let mut nodes = self.nodes.lock().await;
            if let Some(node) = nodes.get_mut(node) {
                let mut client = MelonWorkerClient::connect(node.endpoint.clone())
                    .await
                    .map_err(|e| Status::unknown(format!("Error connecting to node: {}", e)))?;
                let worker_request = proto::ExtendJobRequest {
                    job_id: req.job_id,
                    user: user.clone(),
                    extension_mins: req.extension_mins,
                };
                client.extend_job(worker_request).await?;

                // adjust the job resources
                job.extend_time(time_in_mins);

                return Ok(tonic::Response::new(()));
            }
        }

        Err(tonic::Status::not_found("Couldn't find job id"))
    }

    #[tracing::instrument(
        level = "info",
        name = "Get job by job id",
        skip(self, request),
        fields(job_id = %request.get_ref().job_id)
    )]
    async fn get_job_info(
        &self,
        request: tonic::Request<proto::GetJobInfoRequest>,
    ) -> core::result::Result<tonic::Response<proto::Job>, tonic::Status> {
        let req = request.get_ref();
        let id = req.job_id;

        // check in running jobs => O(1)
        let running_jobs = self.running_jobs.lock().await;
        if let Some(job) = running_jobs.get(&id) {
            log!(debug, "Found job with id {} in running jobs", id);
            return Ok(tonic::Response::new(job.into()));
        }

        // check in pending jobs
        let pending_jobs = self.pending_jobs.lock().await;
        if let Some(pos) = pending_jobs.iter().position(|job| job.id == id) {
            log!(debug, "Found job with id {} in pending jobs", id);
            let job = pending_jobs.get(pos).expect("exists for sure");
            return Ok(tonic::Response::new(job.into()));
        }

        // check finished jobs in database
        match self.db.get_job_opt(id) {
            Ok(Some(job)) => {
                log!(debug, "Found job with id {} in database", id);
                Ok(tonic::Response::new((&job).into()))
            }
            Ok(None) => {
                log!(debug, "Could not find job with id {} anywhere", id);
                Err(tonic::Status::not_found(format!("Job ID not found {}", id)))
            }
            Err(e) => {
                log!(
                    error,
                    "Unexpected error when looking for job with id {} in database: {}",
                    id,
                    e
                );
                Err(tonic::Status::unknown(format!("Unexpected Error {}", e)))
            }
        }
    }
}
