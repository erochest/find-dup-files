use std::path::PathBuf;
use std::result;
use std::str::FromStr;

use clap_verbosity_flag::Verbosity;
use env_logger::Builder;
use log::{info, Level};
use structopt::StructOpt;
use walkdir::WalkDir;

mod error;

use error::Result;

fn main() -> Result<()> {
    let args = Cli::from_args();

    Builder::new()
        .filter_level(args.verbose.log_level().unwrap_or(Level::Warn).to_level_filter())
        .init();

    for entry in WalkDir::new(&args.directory) {
        info!("{}", entry?.path().display());
    }

    Ok(())
}

#[derive(Debug, StructOpt)]
struct Cli {
    #[structopt(flatten)]
    verbose: Verbosity,

    #[structopt(short, long, parse(from_os_str))]
    directory: PathBuf,

    #[structopt(long)]
    action: Option<Action>,
}

#[derive(Debug)]
enum Action {
    List,
}

impl FromStr for Action {
    type Err = error::Error;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        match s {
            "list" => Ok(Action::List),
            _ => Err(error::Error::CliParseError(format!("Invalid actuon: {}", s))),
        }
    }
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
