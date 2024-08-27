use serde_aux::field_attributes::deserialize_number_from_string;
use std::fmt;

#[derive(serde::Deserialize, Clone, Debug)]
pub struct Settings {
    pub application: ApplicationSettings,
    pub database: DatabaseSettings,
    pub api: ApiSettings,
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct ApplicationSettings {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct ApiSettings {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct DatabaseSettings {
    pub path: String,
}

impl fmt::Display for Settings {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Settings:\n  Application:\n{} \n Database:\n{} \n API:\n{}",
            self.application, self.database, self.api
        )
    }
}

impl fmt::Display for ApplicationSettings {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "    Host: {}\n    Port: {}", self.host, self.port)
    }
}

impl fmt::Display for DatabaseSettings {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "    Path: {}", self.path)
    }
}

impl fmt::Display for ApiSettings {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "    Host: {}\n    Port: {}", self.host, self.port)
    }
}
