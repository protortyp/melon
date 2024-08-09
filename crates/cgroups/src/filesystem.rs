use std::collections::HashMap;
use std::fs;
use std::io::Result;
use std::io::{Error, ErrorKind};
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub trait FileSystem {
    fn create_dir_all(&self, path: &Path) -> Result<()>;
    fn write(&self, path: &Path, contents: &[u8]) -> Result<()>;
    fn append(&self, path: &Path, contents: &[u8]) -> Result<()>;
    fn read(&self, path: &Path) -> Result<Vec<u8>>;
}

pub struct RealFileSystem;

impl FileSystem for RealFileSystem {
    fn create_dir_all(&self, path: &Path) -> Result<()> {
        fs::create_dir_all(path)
    }

    fn write(&self, path: &Path, contents: &[u8]) -> Result<()> {
        fs::write(path, contents)
    }

    fn append(&self, path: &Path, contents: &[u8]) -> Result<()> {
        use std::fs::OpenOptions;
        use std::io::Write;

        let mut file = OpenOptions::new().append(true).open(path)?;
        file.write_all(contents)
    }

    fn read(&self, path: &Path) -> Result<Vec<u8>> {
        fs::read(path)
    }
}

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
}
