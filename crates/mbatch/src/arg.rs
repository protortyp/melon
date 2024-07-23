use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Port
    #[arg(short, long, default_value_t = 8080)]
    pub port: u16,

    /// Script path
    pub script: String,

    /// Script arguments
    #[arg(trailing_var_arg = true)]
    pub script_args: Vec<String>,
}
