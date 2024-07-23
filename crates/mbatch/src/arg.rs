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

    /// Script path
    pub script: String,

    /// Script arguments
    #[arg(trailing_var_arg = true)]
    pub script_args: Vec<String>,
}
