use std::convert::From;
use std::error;
use std::fmt;
use std::io;
use std::result;

use walkdir;

pub type Result<R> = result::Result<R, Error>;

#[derive(Debug)]
pub enum Error {
    IoError(io::Error),
    WalkDirError(walkdir::Error),
    CliParseError(String),
}

use Error::*;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IoError(ref err) => err.fmt(f),
            WalkDirError(ref err) => err.fmt(f),
            CliParseError(ref msg) => write!(f, "CLI parse error: {}", msg),
        }
    }
}

impl error::Error for Error {
}

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
