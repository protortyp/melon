use std::fs;
use std::io::Result;
use std::path::Path;

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
