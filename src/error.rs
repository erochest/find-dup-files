use std::convert::From;
use std::error;
use std::fmt;
use std::io;
use std::result;

use crossbeam::channel::{RecvError, SendError};

pub type Result<R> = result::Result<R, Error>;

#[derive(Debug)]
pub enum Error {
    IoError(io::Error),
    WalkDirError(walkdir::Error),
    StorageError(rusqlite::Error),
    MessagingError(String),
}

use Error::*;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IoError(ref err) => err.fmt(f),
            WalkDirError(ref err) => err.fmt(f),
            StorageError(ref err) => err.fmt(f),
            MessagingError(ref desc) => write!(f, "{}", desc),
        }
    }
}

impl error::Error for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        IoError(err)
    }
}

impl From<walkdir::Error> for Error {
    fn from(err: walkdir::Error) -> Error {
        WalkDirError(err)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Error {
        StorageError(err)
    }
}

impl<T> From<SendError<T>> for Error {
    fn from(err: SendError<T>) -> Error {
        MessagingError(err.to_string())
    }
}

impl From<RecvError> for Error {
    fn from(err: RecvError) -> Error {
        MessagingError(err.to_string())
    }
}
