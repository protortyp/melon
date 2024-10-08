use crate::error::Result;
use directories::ProjectDirs;
use melon_common::{log, Job, JobStatus, RequestedResources};
use rusqlite::{params, Connection, Result as SqliteResult};
use serde_json;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::{
    sync::{mpsc, Mutex, Notify},
    task::JoinHandle,
};

use crate::settings::DatabaseSettings;

/// Dedicated Database Reader and Writer
///
/// Receives finished [Job]s from the Scheduler and writes them to the database.
/// Stops its operation when it receives a shutdown request.
#[derive(Debug)]
pub struct DatabaseHandler {
    /// Finished Job Receiver
    rx: Arc<Mutex<mpsc::Receiver<Job>>>,

    /// Thread Handle
    handle: Option<JoinHandle<()>>,

    /// Thread Shutdown Notifier
    notifier: Arc<Notify>,

    /// Database Path
    db_path: String,
}

impl DatabaseHandler {
    #[tracing::instrument(level = "debug", name = "Create new DatabaseWriter", skip(rx))]
    pub fn new(rx: mpsc::Receiver<Job>, settings: &DatabaseSettings) -> Result<Self> {
        Ok(Self {
            rx: Arc::new(Mutex::new(rx)),
            notifier: Arc::new(Notify::new()),
            handle: None,
            db_path: settings.path.clone(),
        })
    }

    #[tracing::instrument(level = "debug", name = "Shut down DatabaseWriter", skip(self))]
    pub fn shutdown(&self) {
        self.notifier.notify_one();
    }

    #[tracing::instrument(level = "debug", name = "Create DatabaseWriter thread", skip(self))]
    pub fn run(&mut self) -> Result<()> {
        let notifier = self.notifier.clone();
        let rx = self.rx.clone();
        let conn = initialize_database(&self.db_path)?;
        let conn = Arc::new(Mutex::new(conn));

        let handle = tokio::spawn(async move {
            let span = tracing::span!(tracing::Level::DEBUG, "DatabaseWriter Thread");
            let _guard = span.enter();

            let mut rx = rx.lock().await;
            let conn = conn.lock().await;

            loop {
                tokio::select! {
                    _ = notifier.notified() => {
                        log!(info, "Shutting down Database Writer");
                        break;
                    }
                    Some(job) = rx.recv() => {
                        log!(debug, "Receive new finished job with id {}", job.id);

                        // TODO: retry on transient errors
                        if let Err(e) = insert_finished_job(&conn, &job) {
                            log!(error, "Error storing finished job with id {}: {}", job.id, e);
                        }
                    }
                }
            }
        });

        self.handle = Some(handle);
        Ok(())
    }

    #[tracing::instrument(level = "debug", name = "Get job from database", skip(self), fields(job_id = %job_id))]
    pub fn get_job_opt(&self, job_id: u64) -> Result<Option<Job>> {
        let conn = Connection::open(self.db_path.clone())?;

        let mut stmt = conn.prepare("SELECT * FROM jobs WHERE id = ?")?;
        let mut job_iter = stmt.query_map(params![job_id], |row| {
            Ok(Job {
                id: row.get(0)?,
                user: row.get(1)?,
                script_path: row.get(2)?,
                script_args: serde_json::from_str(&row.get::<_, String>(3)?).unwrap(),
                req_res: RequestedResources {
                    cpu_count: row.get(4)?,
                    memory: row.get(5)?,
                    time: row.get(6)?,
                },
                submit_time: row.get(7)?,
                start_time: row.get(8)?,
                stop_time: row.get(9)?,
                status: JobStatus::from(row.get::<_, i32>(10)?),
                assigned_node: row.get(11)?,
            })
        })?;

        Ok(job_iter.next().transpose()?)
    }

    pub fn get_highest_job_id(&self) -> Result<u64> {
        let conn = Connection::open(self.db_path.clone())?;

        let mut stmt = conn.prepare("SELECT MAX(id) FROM jobs")?;
        let max_id: Option<u64> = stmt.query_row([], |row| row.get(0))?;

        Ok(max_id.unwrap_or(0))
    }

    #[tracing::instrument(level = "debug", name = "Get all jobs from database", skip(self))]
    pub fn get_all_jobs(&self) -> Result<Vec<Job>> {
        let conn = Connection::open(self.db_path.clone())?;

        let mut stmt = conn.prepare("SELECT * FROM jobs")?;
        let job_iter = stmt.query_map([], |row| {
            Ok(Job {
                id: row.get(0)?,
                user: row.get(1)?,
                script_path: row.get(2)?,
                script_args: serde_json::from_str(&row.get::<_, String>(3)?).unwrap(),
                req_res: RequestedResources {
                    cpu_count: row.get(4)?,
                    memory: row.get(5)?,
                    time: row.get(6)?,
                },
                submit_time: row.get(7)?,
                start_time: row.get(8)?,
                stop_time: row.get(9)?,
                status: JobStatus::from(row.get::<_, i32>(10)?),
                assigned_node: row.get(11)?,
            })
        })?;

        let jobs: SqliteResult<Vec<Job>> = job_iter.collect();
        Ok(jobs?)
    }
}

#[tracing::instrument(level = "debug", name = "Insert finished job", skip(conn, job), fields(job_id = %job.id))]
fn insert_finished_job(conn: &Connection, job: &Job) -> Result<()> {
    let script_args = serde_json::to_string(&job.script_args)?;
    let status: i32 = job.status.clone().into();

    conn.execute(
        "INSERT INTO jobs \
         (id, user, script_path, script_args, cpu_count, memory, time, submit_time, start_time, stop_time, status, assigned_node) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            job.id,
            job.user,
            job.script_path,
            script_args,
            job.req_res.cpu_count,
            job.req_res.memory,
            job.req_res.time,
            job.submit_time,
            job.start_time,
            job.stop_time.expect("No stop time set"),
            status,
            job.assigned_node,
        ],
    )?;

    Ok(())
}

#[tracing::instrument(level = "debug", name = "Initialise database")]
fn initialize_database(db_path: &str) -> Result<Connection> {
    let db_path = PathBuf::from(db_path);

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(db_path)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS jobs (
            id INTEGER PRIMARY KEY,
            user TEXT NOT NULL,
            script_path TEXT NOT NULL,
            script_args TEXT NOT NULL,
            cpu_count INTEGER NOT NULL,
            memory INTEGER NOT NULL,
            time INTEGER NOT NULL,
            submit_time INTEGER NOT NULL,
            start_time INTEGER,
            stop_time INTEGER NOT NULL,
            status INTEGER NOT NULL,
            assigned_node TEXT
            )",
        [],
    )?;

    Ok(conn)
}

/// Get the path to the production databse
pub fn get_prod_database_path() -> String {
    let proj_dirs = ProjectDirs::from("com", "MelonOrganization", "Melon")
        .expect("Could not build database path");
    let data_dir = proj_dirs.data_dir();
    let path = data_dir.join("melon.db");
    path.to_str()
        .expect("Path contains invalid Unicode")
        .to_string()
}
