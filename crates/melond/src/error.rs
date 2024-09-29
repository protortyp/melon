use derive_more::From;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, From)]
pub enum Error {
    // Externals
    #[from]
    IoError(std::io::Error),

    #[from]
    TonicTransportError(tonic::transport::Error),

    #[from]
    SqliteError(rusqlite::Error),

    #[from]
    SerdeJsonError(serde_json::Error),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for Error {}
