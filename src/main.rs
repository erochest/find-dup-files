use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::result;

use clap_verbosity_flag::Verbosity;
use data_encoding::HEXLOWER;
use env_logger::Builder;
use log::{debug, info, Level};
use ring::digest;
use rusqlite::{params, Connection};
use structopt::StructOpt;
use walkdir::WalkDir;

mod error;

use error::Result;

fn main() -> Result<()> {
    let args = Cli::from_args();

    Builder::new()
        .filter_level(
            args.verbose
                .log_level()
                .unwrap_or(Level::Warn)
                .to_level_filter(),
        )
        .init();

    let cxn = args
        .storage
        .map(Connection::open)
        .unwrap_or_else(Connection::open_in_memory)?;
    create_database(&cxn)?;

    for entry in WalkDir::new(&args.directory) {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.len() == 0 || !metadata.is_file() {
            continue;
        }

        let mut file = File::open(entry.path())?;
        let digest = hash_reader(&mut file, args.read_buffer)?;

        store_hash(&cxn, entry.path(), &digest)?;
    }

    let hash_paths = read_hash_paths(&cxn)?;

    let mut current_hash_id = None;
    let mut path_buffer = vec![];
    for (hash_id, path) in hash_paths {
        if let Some(current) = current_hash_id {
            if current == hash_id {
                path_buffer.push(path);
            } else {
                if path_buffer.len() > 1 {
                    path_buffer.sort();
                    info!("{}", path_buffer.join("\t"));
                }

                current_hash_id = Some(hash_id);
                path_buffer = vec![path];
            }
        } else {
            current_hash_id = Some(hash_id);
            path_buffer.push(path);
        }
    }

    Ok(())
}

#[derive(Debug, StructOpt)]
struct Cli {
    #[structopt(flatten)]
    verbose: Verbosity,

    #[structopt(short, long, parse(from_os_str))]
    directory: PathBuf,

    #[structopt(
        short,
        long,
        default_value = "1024",
        help = "The size of the read buffer."
    )]
    read_buffer: usize,

    #[structopt(
        short,
        long,
        parse(from_os_str),
        help = "The location of the database to store file hashes. Defaults to in-memory."
    )]
    storage: Option<PathBuf>,
}

fn create_database(cxn: &Connection) -> Result<()> {
    cxn.execute(
        "CREATE TABLE hash (id INTEGER PRIMARY KEY, hash TEXT UNIQUE)",
        params![],
    )?;
    cxn.execute(
        "CREATE TABLE file (
            id INTEGER PRIMARY KEY,
            hash_id INTEGER,
            pathname TEKT UNIQUE
            )",
        params![],
    )?;

    Ok(())
}

fn hash_reader<R: Read>(reader: &mut R, capacity: usize) -> Result<digest::Digest> {
    let mut context = digest::Context::new(&digest::SHA256);
    let mut buffer = Vec::with_capacity(capacity);

    loop {
        buffer.resize(capacity, 0);
        let size = reader.read(buffer.as_mut_slice())?;
        if size == 0 {
            break;
        }
        buffer.resize(size, 0);

        context.update(&buffer);
    }

    Ok(context.finish())
}

fn store_hash(cxn: &Connection, path: &Path, digest: &digest::Digest) -> Result<()> {
    let pathname = path.to_string_lossy().to_string();
    let hash = HEXLOWER.encode(digest.as_ref());
    debug!("{}\t{}", pathname, hash);

    cxn.execute(
        "INSERT OR IGNORE INTO hash (hash) VALUES (?1)",
        params![hash],
    )?;
    let mut stmt = cxn.prepare("SELECT id FROM hash WHERE hash=?1")?;
    let hash_id: u32 = stmt.query_row(params![hash], |row| row.get(0))?;
    cxn.execute(
        "INSERT INTO file (hash_id, pathname) VALUES (?1, ?2)",
        params![hash_id, pathname],
    )?;

    Ok(())
}

fn read_hash_paths(cxn: &Connection) -> Result<Vec<(u32, String)>> {
    let mut stmt = cxn.prepare(
        "SELECT hash_id, pathname
        FROM file
        ORDER BY hash_id, pathname",
    )?;

    let hash_paths = stmt.query_map(params![], |row| {
        let hash_id: u32 = row.get(0)?;
        let path: String = row.get(1)?;
        Ok((hash_id, path))
    })?
    .collect::<result::Result<Vec<(u32, String)>, _>>()?;

    Ok(hash_paths)
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
