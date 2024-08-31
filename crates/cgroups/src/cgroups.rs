use crate::error::{CGroupsError, Result};
use crate::filesystem::{FileSystem, RealFileSystem};
use melon_common::log;
use std::path::{Path, PathBuf};

const BASE_CGROUP_PATH: &str = "/sys/fs/cgroup/melon";

/// # CGroups V2 Management Module
///
/// This module provides a high-level interface for managing Linux Control Groups (cgroups).
/// It allows for easy creation and manipulation of cgroups, including setting CPU, memory,
/// and I/O constraints, as well as adding processes to these groups.
#[derive(Default)]
pub struct CGroupsBuilder {
    name: Option<String>,
    cpus: Option<String>,
    memory: Option<u64>,
    io: Option<String>,
    fs: Option<Box<dyn FileSystem>>,
}

impl CGroupsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_fs<F: FileSystem + 'static>(mut self, fs: F) -> Self {
        self.fs = Some(Box::new(fs));
        self
    }

    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    pub fn with_cpu(mut self, cpus: &str) -> Self {
        self.cpus = Some(cpus.to_string());
        self
    }

    pub fn with_memory(mut self, memory_bytes: u64) -> Self {
        self.memory = Some(memory_bytes);
        self
    }

    pub fn with_io(mut self, io: &str) -> Self {
        self.io = Some(io.to_string());
        self
    }

    pub fn build(self) -> Result<CGroups> {
        let name = self
            .name
            .ok_or_else(|| CGroupsError::InvalidCGroupName("Group name is required".to_string()))?;
        Ok(CGroups {
            name,
            cpus: self.cpus,
            memory: self.memory,
            io: self.io,
            fs: self.fs.unwrap_or_else(|| Box::new(RealFileSystem)),
        })
    }
}

pub struct CGroups {
    /// The cgroup name
    pub name: String,
    /// The allocated CPUs, eg. 0,1,4
    pub cpus: Option<String>,
    /// The memory in bytes
    pub memory: Option<u64>,
    /// The io limits
    pub io: Option<String>,
    /// Filesystem for testing
    fs: Box<dyn FileSystem>,
}

impl Drop for CGroups {
    fn drop(&mut self) {
        // todo: handle errors
        match self.remove() {
            Ok(_) => {
                log!(info, "Removed cgroup {}", self.name);
            }
            Err(CGroupsError::CGroupRemovalFailed(e)) => {
                log!(error, "Could not remove cgroup {}", e.to_string());
            }
            Err(CGroupsError::NotRoot) => {
                log!(
                    error,
                    "Could not remove cgroup {} without sudo privileges!",
                    self.name,
                );
            }
            Err(e) => {
                log!(
                    error,
                    "Could not remove cgroup {} due to error {}",
                    self.name,
                    e.to_string()
                );
            }
        }
    }
}

impl CGroups {
    pub fn build() -> CGroupsBuilder {
        CGroupsBuilder::new()
    }

    #[tracing::instrument(level = "info", name = "Create new cgroup" skip(self))]
    pub fn create(&self) -> Result<()> {
        let path = PathBuf::from(BASE_CGROUP_PATH).join(&self.name);
        self.fs.create_dir_all(&path).map_err(|e| {
            let error_msg = format!("Failed to create directory at {:?}: {}", path, e);
            log!(error, "{}", error_msg);
            CGroupsError::CGroupCreationFailed(e)
        })?;

        let mut controllers = Vec::new();
        if self.cpus.is_some() {
            controllers.push("+cpuset");
        }
        if self.memory.is_some() {
            controllers.push("+memory");
        }
        if self.io.is_some() {
            controllers.push("+io");
        }

        if !controllers.is_empty() {
            self.fs
                .write(
                    &path.join("cgroup.subtree_control"),
                    controllers.join(" ").as_bytes(),
                )
                .map_err(|e| {
                    log!(
                        error,
                        "Could not enable controllers {:?}: {}",
                        controllers,
                        e
                    );
                    CGroupsError::CGroupWriteFailed(e)
                })?;
        }

        // Write individual controller settings
        if let Some(cpus) = &self.cpus {
            self.fs
                .write(&path.join("cpuset.cpus"), cpus.as_bytes())
                .map_err(|e| {
                    log!(error, "Could not write cpuset {}: {}", cpus, e.to_string());
                    CGroupsError::CGroupWriteFailed(e)
                })?;
        }

        if let Some(memory) = self.memory {
            self.fs
                .write(&path.join("memory.max"), memory.to_string().as_bytes())
                .map_err(|e| {
                    log!(
                        error,
                        "Could not write memory {}: {}",
                        memory,
                        e.to_string()
                    );
                    CGroupsError::CGroupWriteFailed(e)
                })?;
        }

        if let Some(io) = &self.io {
            self.fs
                .write(&path.join("io.max"), io.as_bytes())
                .map_err(|e| {
                    log!(error, "Could not write IO {}: {}", io, e.to_string());
                    CGroupsError::CGroupWriteFailed(e)
                })?;
        }

        Ok(())
    }

    #[tracing::instrument(level = "info", name = "Add process to cgroup" skip(self))]
    pub fn add_process(&self, pid: u32) -> Result<()> {
        let path = PathBuf::from(BASE_CGROUP_PATH)
            .join(&self.name)
            .join("cgroup.procs");
        self.fs
            .append(&path, format!("{}\n", pid).as_bytes())
            .map_err(CGroupsError::AddProcessFailed)?;
        Ok(())
    }

    #[tracing::instrument(level = "info", name = "Remove cgroup" skip(self))]
    pub fn remove(&self) -> Result<()> {
        let path = PathBuf::from(BASE_CGROUP_PATH).join(&self.name);

        if !self.fs.exists(&path) {
            log!(error, "Cgroup path does not exist {:?}", path);
            return Err(CGroupsError::CGroupRemovalFailed(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Cgroup does not exist",
            )));
        }

        // ceck if there are any running processes
        if self.has_running_processes(&path)? {
            log!(error, "Cgroup has a running process!");
            return Err(CGroupsError::CGroupHasRunningProcesses);
        }

        // remove the cgroup directory
        self.fs.remove_dir(&path).map_err(|e| {
            log!(error, "Could not remove directory: {}", e.to_string());
            CGroupsError::CGroupRemovalFailed(e)
        })?;

        Ok(())
    }

    fn process_exists(&self, pid: i32) -> bool {
        let proc_stat_path = PathBuf::from(format!("/proc/{}/stat", pid));
        self.fs.exists(&proc_stat_path)
    }

    fn has_running_processes(&self, path: &Path) -> Result<bool> {
        let procs_path = path.join("cgroup.procs");

        let procs = self
            .fs
            .read_to_string(&procs_path)
            .map_err(CGroupsError::CGroupReadFailed)?;

        for pid in procs.split_whitespace() {
            if let Ok(pid) = pid.parse::<i32>() {
                if self.process_exists(pid) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }
}
