use std::fmt;

use serde_aux::field_attributes::deserialize_number_from_string;

#[derive(serde::Deserialize, Clone, Debug)]
pub struct Settings {
    pub application: ApplicationSettings,
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct ApplicationSettings {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
}

impl fmt::Display for Settings {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Settings:\n  Application:\n{}", self.application)
    }
}

impl fmt::Display for ApplicationSettings {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "    Host: {}\n    Port: {}", self.host, self.port)
    }
}
