use clap_verbosity_flag::Verbosity;
use env_logger;
use structopt::StructOpt;

mod error;

use error::Result;

fn main() -> Result<()> {
    env_logger::init();
    let args = Cli::from_args();

    println!("{:?}", args);

    Ok(())
}

#[derive(Debug, StructOpt)]
struct Cli {
    #[structopt(flatten)]
    verbose: Verbosity,
}
