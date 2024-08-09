use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// API Endpoint
    #[arg(
        short = 'a',
        long = "api_endpoint",
        default_value = "http://[::1]:8080"
    )]
    pub api_endpoint: String,

    /// The job id
    #[arg()]
    pub job: u64,

    #[arg(long = "parseable")]
    pub parseable: bool,
}
