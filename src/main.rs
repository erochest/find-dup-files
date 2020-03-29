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

// # Planning
//
// Workers:
// - *directory walker* sends files to read to the *file reader*;
// - *file reader* reads a chunk of the file and sends it to one of the *hash workers*;
// - *hash workers* take a file chunk and add it to the hash, queuing a request for the next chunk;
// - *database worker* inserts completed hash information into the database.
//
// Messages:
// - *open file*
// - *next chunk*
// - *hash chunk*
// - *save hash*
