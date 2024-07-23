use clap::Parser;
use std::net::SocketAddr;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// API Endpoint
    #[arg(short = 'a', long = "api_endpoint", default_value = "[::1]:8080")]
    pub api_endpoint: SocketAddr,
}
