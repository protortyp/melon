#[cfg(test)]
mod tests {
    use crate::error::CGroupsError;
    use crate::filesystem::{FileSystem, MockFileSystem};
    use crate::CGroups;
    use std::io::{Error, ErrorKind, Result};
    use std::path::Path;

    fn setup_mock_fs() -> MockFileSystem {
        MockFileSystem::new()
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
                .read(Path::new(
                    "/sys/fs/cgroup/test_cgroup/memory.limit_in_bytes",
                ))
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
                .read(Path::new(
                    "/sys/fs/cgroup/test_cgroup/cgroup.subtree_control",
                ))
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
                .read(Path::new(
                    "/sys/fs/cgroup/test_cgroup/cgroup.subtree_control",
                ))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(controllers_content, "+cpuset");
        assert!(mock_fs
            .read(Path::new(
                "/sys/fs/cgroup/test_cgroup/memory.limit_in_bytes"
            ))
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
                unimplemented!()
            }
            fn append(&self, _path: &Path, _contents: &[u8]) -> Result<()> {
                unimplemented!()
            }
            fn read(&self, _path: &Path) -> Result<Vec<u8>> {
                unimplemented!()
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
                Ok(())
            }
            fn write(&self, _path: &Path, _contents: &[u8]) -> Result<()> {
                Ok(())
            }
            fn append(&self, _path: &Path, _contents: &[u8]) -> Result<()> {
                Err(Error::new(ErrorKind::PermissionDenied, "Permission denied"))
            }
            fn read(&self, _path: &Path) -> Result<Vec<u8>> {
                unimplemented!()
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
}
