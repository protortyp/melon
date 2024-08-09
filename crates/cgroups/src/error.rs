use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CGroupsError {
    #[error("Operation requires root privileges")]
    NotRoot,

    #[error("Failed to create cgroup directory: {0}")]
    CGroupCreationFailed(#[source] io::Error),

    #[error("Failed to write to cgroup file: {0}")]
    CGroupWriteFailed(#[source] io::Error),

    #[error("Failed to read from cgroup file: {0}")]
    CGroupReadFailed(#[source] io::Error),

    #[error("Invalid cgroup name: {0}")]
    InvalidCGroupName(String),

    #[error("Invalid CPU specification: {0}")]
    InvalidCPUSpec(String),

    #[error("Invalid memory specification: {0}")]
    InvalidMemorySpec(String),

    #[error("Invalid I/O specification: {0}")]
    InvalidIOSpec(String),

    #[error("Failed to add process to cgroup: {0}")]
    AddProcessFailed(#[source] io::Error),

    #[error("Cgroup file not found: {0}")]
    CGroupFileNotFound(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl From<io::Error> for CGroupsError {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::PermissionDenied => CGroupsError::NotRoot,
            io::ErrorKind::NotFound => CGroupsError::CGroupFileNotFound(error.to_string()),
            _ => CGroupsError::Unknown(error.to_string()),
        }
    }
}

pub type Result<T> = std::result::Result<T, CGroupsError>;
