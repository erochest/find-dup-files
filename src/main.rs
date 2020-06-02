use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::result;
use std::thread;

use clap_verbosity_flag::Verbosity;
use crossbeam::channel;
use data_encoding::HEXLOWER;
use env_logger::Builder;
use log::{debug, info, trace, Level};
use ring::digest;
use rusqlite::{params, Connection};
use structopt::StructOpt;
use tempfile::NamedTempFile;
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

    let (hash_worker_send, hash_worker_recv): (
        channel::Sender<HashChunk>,
        channel::Receiver<HashChunk>,
    ) = channel::bounded(16);
    let (db_worker_send, db_worker_recv) = channel::bounded(16);
    let (wait_send, wait_recv) = channel::bounded(16);

    let db_temp_file = if let Some(_) = args.storage {
        None
    } else {
        Some(NamedTempFile::new()?)
    };
    let db_path: Option<PathBuf> = args
        .storage
        .or_else(|| db_temp_file.map(|tf| tf.path().into()));
    let create_db_path = db_path.clone();
    let cxn = &create_db_path
        .map(Connection::open)
        .unwrap_or_else(Connection::open_in_memory)?;
    create_database(&cxn)?;

    spawn_hash_worker(hash_worker_recv, db_worker_send.clone());
    spawn_store_hash_worker(db_path.clone(), db_worker_recv, wait_send);

    for entry in WalkDir::new(&args.directory) {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.len() == 0 || !metadata.is_file() {
            continue;
        }

        hash_reader(PathBuf::from(entry.path()), args.read_buffer, hash_worker_send.clone())?;
    }
    hash_worker_send.send(HashChunk::Done)?;

    // Block on `Done`.
    wait_recv.recv()?;

    let hash_paths = read_hash_paths(&cxn)?;
    report_duplicate_files(hash_paths);

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

fn hash_reader(path: PathBuf, capacity: usize, sender: channel::Sender<HashChunk>) -> Result<()> {
    let mut file = File::open(path.clone())?;
    let mut n = 0;

    loop {
        let mut buffer = Vec::with_capacity(capacity);
        buffer.resize(capacity, 0);
        let size = file.read(buffer.as_mut_slice())?;
        if size == 0 {
            debug!("sending FileDone {:?}", path);
            sender.send(HashChunk::FileDone(path.clone()))?;
            break;
        }
        buffer.resize(size, 0);

        debug!("sending HashChunk {} {:?}", n, path);
        sender.send(HashChunk::HashChunk(path.clone(), n, Vec::from(buffer)))?;
        n += 1;
    }

    Ok(())
}

struct WorkingContext {
    n: usize,
    context: digest::Context,
}

fn spawn_hash_worker(receiver: channel::Receiver<HashChunk>, sender: channel::Sender<StoreHash>) {
    debug!("spawning hash worker");
    thread::spawn(move || {
        let mut in_process: HashMap<PathBuf, WorkingContext> = HashMap::new();

        loop {
            match receiver.recv().unwrap() {
                HashChunk::HashChunk(ref path, n, ref buffer) => {
                    debug!("received HashChunk {} {:?}", n, path);
                    in_process.entry(path.clone())
                        .and_modify(|working_context| {
                            assert!(working_context.n == (n - 1));
                            (*working_context).n = n;
                            working_context.context.update(buffer);
                        })
                        .or_insert_with(|| {
                            assert!(n == 0);
                            let mut context = digest::Context::new(&digest::SHA256);
                            context.update(buffer);
                            WorkingContext { n, context }
                        });
                },

                HashChunk::FileDone(path) => {
                    debug!("received FileDone {:?}", path);
                    if let Some(working_context) = in_process.remove(&path) {
                        let digest = working_context.context.finish();
                        debug!("sending StoreHash {:?}", path);
                        sender.send(StoreHash::StoreHash(path, digest)).unwrap();
                    }
                },

                HashChunk::Done => {
                    debug!("received HashChunk::Done");
                    debug!("sending StoreHash::Done");
                    sender.send(StoreHash::Done).unwrap();
                    break;
                },
            }
        }
    });
}

enum HashChunk {
    HashChunk(PathBuf, usize, Vec<u8>),
    FileDone(PathBuf),
    Done,
}

enum StoreHash {
    StoreHash(PathBuf, digest::Digest),
    Done,
}

enum ProcessEnd {
    Done,
}

fn spawn_store_hash_worker(
    db_path: Option<PathBuf>,
    db_worker_recv: channel::Receiver<StoreHash>,
    wait_send: channel::Sender<ProcessEnd>,
) {
    debug!("spawning store hash worker");
    thread::spawn(move || {
        let cxn = db_path
            .map(Connection::open)
            .unwrap_or_else(Connection::open_in_memory)
            .unwrap();
        loop {
            match db_worker_recv.recv().unwrap() {
                StoreHash::StoreHash(ref path, ref digest) => {
                    debug!("received StoreHash {:?}", path);
                    store_hash(&cxn, path, digest).unwrap()
                }
                StoreHash::Done => {
                    debug!("received StoreHash::Done");
                    wait_send.send(ProcessEnd::Done).unwrap();
                    break;
                }
            }
        }
    });
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

    let hash_paths = stmt
        .query_map(params![], |row| {
            let hash_id: u32 = row.get(0)?;
            let path: String = row.get(1)?;
            trace!("retrieving {} / {}", hash_id, path);
            Ok((hash_id, path))
        })?
        .collect::<result::Result<Vec<(u32, String)>, _>>()?;

    Ok(hash_paths)
}

fn report_duplicate_files(hash_paths: Vec<(u32, String)>) {
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

    if path_buffer.len() > 1 {
        path_buffer.sort();
        info!("{}", path_buffer.join("\t"));
    }
}

// # Planning
//
// Workers:
// - [ ] *directory walker* [singleton] sends files to read to the *file reader*;
// - [ ] *file reader* [singleton] reads a chunk of the file and sends it to one of the *hash workers*;
// - [x] *hash workers* [pool] take a file chunk and add it to the hash, queuing a request for the next chunk;
// - [x] *database worker* [singleton] inserts completed hash information into the database.
//
// Messages:
// - *open file*
// - *next chunk*
// - *hash chunk*
// - *save hash*
