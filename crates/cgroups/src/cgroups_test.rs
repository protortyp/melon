#[cfg(test)]
mod tests {
    use crate::error::CGroupsError;
    use crate::filesystem::FileSystem;
    use crate::CGroups;
    use std::collections::HashMap;
    use std::io::{Error, ErrorKind, Result};
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    pub struct MockFileSystem {
        files: Arc<Mutex<HashMap<PathBuf, Vec<u8>>>>,
    }

    impl MockFileSystem {
        pub fn new() -> Self {
            Self {
                files: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        fn set_running_processes(&self, pids: Vec<i32>) {
            let mut files = self.files.lock().unwrap();
            for pid in pids {
                files.insert(PathBuf::from(format!("/proc/{}/stat", pid)), vec![]);
            }
        }
    }

    impl FileSystem for MockFileSystem {
        fn create_dir_all(&self, path: &Path) -> Result<()> {
            let path = path.to_path_buf();
            let mut files = self.files.lock().unwrap();
            files.insert(path, Vec::new());
            Ok(())
        }

        fn write(&self, path: &Path, contents: &[u8]) -> Result<()> {
            let path = path.to_path_buf();
            let mut files = self.files.lock().unwrap();
            files.insert(path, contents.to_vec());
            Ok(())
        }

        fn append(&self, path: &Path, contents: &[u8]) -> Result<()> {
            let path = path.to_path_buf();
            let mut files = self.files.lock().unwrap();
            files.entry(path).or_default().extend_from_slice(contents);
            Ok(())
        }

        fn read(&self, path: &Path) -> Result<Vec<u8>> {
            let path = path.to_path_buf();
            let files = self.files.lock().unwrap();
            files
                .get(&path)
                .cloned()
                .ok_or_else(|| Error::new(ErrorKind::NotFound, "File not found"))
        }

        fn exists(&self, path: &Path) -> bool {
            let files = self.files.lock().unwrap();
            files.contains_key(&path.to_path_buf())
        }

        fn read_to_string(&self, path: &Path) -> Result<String> {
            let content = self.read(path)?;
            String::from_utf8(content).map_err(|e| Error::new(ErrorKind::InvalidData, e))
        }

        fn remove_dir(&self, path: &Path) -> Result<()> {
            let path = path.to_path_buf();
            let mut files = self.files.lock().unwrap();
            files.retain(|k, _| !k.starts_with(&path));
            Ok(())
        }
    }

    fn setup_mock_fs() -> MockFileSystem {
        MockFileSystem::new()
    }

    fn setup_cgroup(mock_fs: &MockFileSystem, name: &str) {
        let cgroup_path = PathBuf::from("/sys/fs/cgroup").join(name);
        mock_fs
            .files
            .lock()
            .unwrap()
            .insert(cgroup_path.clone(), Vec::new());
        mock_fs.files.lock().unwrap().insert(
            cgroup_path.join("cgroup.procs"),
            "1000\n2000\n3000".as_bytes().to_vec(),
        );
    }

    #[test]
    fn test_cgroups_builder() {
        let cgroup = CGroups::build()
            .name("test_cgroup")
            .with_cpu("0-1")
            .with_memory(1024 * 1024)
            .with_io("8:0 rbps=1048576")
            .build()
            .unwrap();

        assert_eq!(cgroup.name, "test_cgroup");
        assert_eq!(cgroup.cpus, Some("0-1".to_string()));
        assert_eq!(cgroup.memory, Some(1024 * 1024));
        assert_eq!(cgroup.io, Some("8:0 rbps=1048576".to_string()));
    }

    #[test]
    fn test_cgroups_builder_without_name() {
        let result = CGroups::build().build();
        assert!(matches!(result, Err(CGroupsError::InvalidCGroupName(_))));
    }

    #[test]
    fn test_cgroup_creation() {
        let mock_fs = setup_mock_fs();
        let cgroup = CGroups::build()
            .name("test_cgroup")
            .with_cpu("0-1")
            .with_memory(1024 * 1024)
            .with_io("8:0 rbps=1048576")
            .with_fs(mock_fs.clone())
            .build()
            .unwrap();

        assert!(cgroup.create().is_ok());

        assert!(mock_fs
            .read(Path::new("/sys/fs/cgroup/test_cgroup"))
            .is_ok());

        // verify settings
        let cpu_content = String::from_utf8(
            mock_fs
                .read(Path::new("/sys/fs/cgroup/test_cgroup/cpuset.cpus"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(cpu_content, "0-1");
        let memory_content = String::from_utf8(
            mock_fs
                .read(Path::new("/sys/fs/cgroup/test_cgroup/memory.max"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(memory_content, "1048576");
        let io_content = String::from_utf8(
            mock_fs
                .read(Path::new("/sys/fs/cgroup/test_cgroup/io.max"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(io_content, "8:0 rbps=1048576");
        let controllers_content = String::from_utf8(
            mock_fs
                .read(Path::new("/sys/fs/cgroup/cgroup.subtree_control"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(controllers_content, "+cpuset +memory +io");
    }

    #[test]
    fn test_cgroup_creation_with_partial_settings() {
        let mock_fs = setup_mock_fs();
        let cgroup = CGroups::build()
            .name("test_cgroup")
            .with_cpu("0-1")
            .with_fs(mock_fs.clone())
            .build()
            .unwrap();

        assert!(cgroup.create().is_ok());

        // verify settings
        let cpu_content = String::from_utf8(
            mock_fs
                .read(Path::new("/sys/fs/cgroup/test_cgroup/cpuset.cpus"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(cpu_content, "0-1");
        let controllers_content = String::from_utf8(
            mock_fs
                .read(Path::new("/sys/fs/cgroup/cgroup.subtree_control"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(controllers_content, "+cpuset");
        assert!(mock_fs
            .read(Path::new("/sys/fs/cgroup/test_cgroup/memory.max"))
            .is_err());
        assert!(mock_fs
            .read(Path::new("/sys/fs/cgroup/test_cgroup/io.max"))
            .is_err());
    }

    #[test]
    fn test_add_process() {
        let mock_fs = setup_mock_fs();
        let cgroup = CGroups::build()
            .name("test_cgroup")
            .with_fs(mock_fs.clone())
            .build()
            .unwrap();

        cgroup.create().unwrap();

        assert!(cgroup.add_process(1234).is_ok());
        assert!(cgroup.add_process(5678).is_ok());

        let procs_content = String::from_utf8(
            mock_fs
                .read(Path::new("/sys/fs/cgroup/test_cgroup/cgroup.procs"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(procs_content, "1234\n5678\n");
    }

    #[test]
    fn test_cgroup_creation_failure() {
        struct FailingMockFileSystem;

        impl FileSystem for FailingMockFileSystem {
            fn create_dir_all(&self, _path: &Path) -> Result<()> {
                Err(Error::new(ErrorKind::PermissionDenied, "Permission denied"))
            }
            fn write(&self, _path: &Path, _contents: &[u8]) -> Result<()> {
                Err(Error::new(ErrorKind::PermissionDenied, "Permission denied"))
            }
            fn append(&self, _path: &Path, _contents: &[u8]) -> Result<()> {
                Err(Error::new(ErrorKind::PermissionDenied, "Permission denied"))
            }
            fn read(&self, _path: &Path) -> Result<Vec<u8>> {
                Err(Error::new(ErrorKind::PermissionDenied, "Permission denied"))
            }
            fn exists(&self, _path: &Path) -> bool {
                false
            }
            fn read_to_string(&self, _path: &Path) -> Result<String> {
                Err(Error::new(ErrorKind::PermissionDenied, "Permission denied"))
            }
            fn remove_dir(&self, _path: &Path) -> Result<()> {
                Err(Error::new(ErrorKind::PermissionDenied, "Permission denied"))
            }
        }

        let cgroup = CGroups::build()
            .name("test_cgroup")
            .with_fs(FailingMockFileSystem)
            .build()
            .unwrap();

        let result = cgroup.create();
        assert!(matches!(result, Err(CGroupsError::CGroupCreationFailed(_))));
    }

    #[test]
    fn test_add_process_failure() {
        struct FailingMockFileSystem;

        impl FileSystem for FailingMockFileSystem {
            fn create_dir_all(&self, _path: &Path) -> Result<()> {
                Err(Error::new(ErrorKind::PermissionDenied, "Permission denied"))
            }
            fn write(&self, _path: &Path, _contents: &[u8]) -> Result<()> {
                Err(Error::new(ErrorKind::PermissionDenied, "Permission denied"))
            }
            fn append(&self, _path: &Path, _contents: &[u8]) -> Result<()> {
                Err(Error::new(ErrorKind::PermissionDenied, "Permission denied"))
            }
            fn read(&self, _path: &Path) -> Result<Vec<u8>> {
                Err(Error::new(ErrorKind::PermissionDenied, "Permission denied"))
            }
            fn exists(&self, _path: &Path) -> bool {
                false
            }
            fn read_to_string(&self, _path: &Path) -> Result<String> {
                Err(Error::new(ErrorKind::PermissionDenied, "Permission denied"))
            }
            fn remove_dir(&self, _path: &Path) -> Result<()> {
                Err(Error::new(ErrorKind::PermissionDenied, "Permission denied"))
            }
        }

        let cgroup = CGroups::build()
            .name("test_cgroup")
            .with_fs(FailingMockFileSystem)
            .build()
            .unwrap();

        let result = cgroup.add_process(1234);
        assert!(matches!(result, Err(CGroupsError::AddProcessFailed(_))));
    }

    #[test]
    fn test_remove_success() {
        let mock_fs = setup_mock_fs();
        setup_cgroup(&mock_fs, "test_cgroup");
        let cgroup = CGroups::build()
            .name("test_cgroup")
            .with_fs(mock_fs.clone())
            .build()
            .unwrap();

        assert!(cgroup.remove().is_ok());
        assert!(!mock_fs.exists(&PathBuf::from("/sys/fs/cgroup/test_cgroup")));
    }

    #[test]
    fn test_remove_cgroup_not_found() {
        let mock_fs = setup_mock_fs();
        let cgroup = CGroups::build()
            .name("non_existent_cgroup")
            .with_fs(mock_fs.clone())
            .build()
            .unwrap();

        let result = cgroup.remove();
        assert!(matches!(result, Err(CGroupsError::CGroupRemovalFailed(_))));
    }

    #[test]
    fn test_remove_with_running_processes() {
        let mock_fs = setup_mock_fs();
        setup_cgroup(&mock_fs, "test_cgroup");
        mock_fs.set_running_processes(vec![1000, 2000]);
        let cgroup = CGroups::build()
            .name("test_cgroup")
            .with_fs(mock_fs.clone())
            .build()
            .unwrap();

        let result = cgroup.remove();
        assert!(matches!(
            result,
            Err(CGroupsError::CGroupHasRunningProcesses)
        ));
    }

    #[test]
    fn test_remove_failed() {
        #[derive(Clone)]
        struct FailingMockFileSystem {
            files: Arc<Mutex<HashMap<PathBuf, Vec<u8>>>>,
        }

        impl FailingMockFileSystem {
            fn new() -> Self {
                Self {
                    files: Arc::new(Mutex::new(HashMap::new())),
                }
            }
        }

        impl FileSystem for FailingMockFileSystem {
            fn create_dir_all(&self, path: &Path) -> Result<()> {
                let path = path.to_path_buf();
                let mut files = self.files.lock().unwrap();
                files.insert(path, Vec::new());
                Ok(())
            }

            fn write(&self, path: &Path, contents: &[u8]) -> Result<()> {
                let path = path.to_path_buf();
                let mut files = self.files.lock().unwrap();
                files.insert(path, contents.to_vec());
                Ok(())
            }

            fn append(&self, path: &Path, contents: &[u8]) -> Result<()> {
                let path = path.to_path_buf();
                let mut files = self.files.lock().unwrap();
                files.entry(path).or_default().extend_from_slice(contents);
                Ok(())
            }

            fn read(&self, path: &Path) -> Result<Vec<u8>> {
                let path = path.to_path_buf();
                let files = self.files.lock().unwrap();
                files
                    .get(&path)
                    .cloned()
                    .ok_or_else(|| Error::new(ErrorKind::NotFound, "File not found"))
            }

            fn exists(&self, path: &Path) -> bool {
                let files = self.files.lock().unwrap();
                files.contains_key(&path.to_path_buf())
            }

            fn read_to_string(&self, path: &Path) -> Result<String> {
                let content = self.read(path)?;
                String::from_utf8(content).map_err(|e| Error::new(ErrorKind::InvalidData, e))
            }

            // only let directory removals fail
            fn remove_dir(&self, _path: &Path) -> Result<()> {
                Err(Error::new(ErrorKind::PermissionDenied, "Permission denied"))
            }
        }

        let mock_fs = FailingMockFileSystem::new();
        let cgroup_path = PathBuf::from("/sys/fs/cgroup/test_cgroup");
        mock_fs
            .files
            .lock()
            .unwrap()
            .insert(cgroup_path.clone(), Vec::new());

        mock_fs
            .files
            .lock()
            .unwrap()
            .insert(cgroup_path.join("cgroup.procs"), Vec::new());
        let cgroup = CGroups::build()
            .name("test_cgroup")
            .with_fs(mock_fs)
            .build()
            .unwrap();

        let result = cgroup.remove();
        assert!(matches!(result, Err(CGroupsError::CGroupRemovalFailed(_))));
    }
}
