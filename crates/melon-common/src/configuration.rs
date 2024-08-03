use config::ConfigError;
use serde::de::DeserializeOwned;
use std::convert::TryInto;
use std::env;
use std::path::PathBuf;

pub fn get_configuration<T: DeserializeOwned + std::fmt::Display>() -> Result<T, ConfigError> {
    let configuration_directory = env::var("CONFIG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            env::current_dir()
                .expect("Failed to determine the current directory")
                .join("configuration")
        });

    let environment: Environment = env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT.");

    let environment_filename = format!("{}.yaml", environment.as_str());

    let settings = config::Config::builder()
        .add_source(config::File::from(
            configuration_directory.join("base.yaml"),
        ))
        .add_source(config::File::from(
            configuration_directory.join(environment_filename),
        ))
        // allow to overwrite configuration explicitly with environment variables
        // APP_DATABASE__HOST=185.13.12.1 to update database.host
        .add_source(
            config::Environment::with_prefix("APP")
                .prefix_separator("_")
                .separator("__"),
        )
        .build()?;

    let settings = settings.try_deserialize::<T>()?;

    Ok(settings)
}

pub enum Environment {
    Local,
    Production,
    CI,
}

impl Environment {
    /// Convert the enum instance to a static string reference.
    ///
    /// # Returns
    ///
    /// This method returns a static string slice that corresponds to the variant of the `Environment` enum.
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Production => "production",
            Environment::CI => "ci",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    /// Try to convert a string to an `Environment` variant.
    /// Case insensitive matching is performed and only "local", "ci", and "production" are accepted.
    ///
    /// # Parameters
    ///
    /// * `s`: A `String` that we attempt to match to an `Environment` variant.
    ///
    /// # Returns
    ///
    /// This function returns a `Result` which is `Ok` if a match is found and `Err` otherwise.
    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            "ci" => Ok(Self::CI),
            other => Err(format!(
                "{} is not a supported environment. Use either `local`, `ci` or `production`.",
                other
            )),
        }
    }
}
