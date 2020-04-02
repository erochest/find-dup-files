use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::result;
use std::str::FromStr;

use clap_verbosity_flag::Verbosity;
use data_encoding::HEXLOWER;
use env_logger::Builder;
use log::{debug, info, Level};
use ring::digest;
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
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }

        let mut context = digest::Context::new(&digest::SHA256);
        let mut file = File::open(entry.path())?;
        let mut buffer = Vec::with_capacity(args.read_buffer);

        loop {
            buffer.resize(args.read_buffer, 0);
            let size = file.read(buffer.as_mut_slice())?;
            if size == 0 {
                break;
            }
            buffer.resize(size, 0);

            context.update(&buffer);
        }

        let digest = context.finish();

        info!("{}\t{}", entry.path().display(), HEXLOWER.encode(digest.as_ref()));
    }

    Ok(())
}

#[derive(Debug, StructOpt)]
struct Cli {
    #[structopt(flatten)]
    verbose: Verbosity,

    #[structopt(short, long, parse(from_os_str))]
    directory: PathBuf,

    #[structopt(long, help = "One of 'list', 'hash'.")]
    action: Option<Action>,

    #[structopt(short, long, default_value = "1024", help = "The size of the read buffer.")]
    read_buffer: usize,
}

#[derive(Debug)]
enum Action {
    List,
    Hash,
}

impl FromStr for Action {
    type Err = error::Error;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        match s {
            "list" => Ok(Action::List),
            "hash" => Ok(Action::Hash),
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
