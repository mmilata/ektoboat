use std::convert::From;
use std::error;

pub const USER_AGENT: &str = "ektoboat/1";

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Error {
    msg: String,
    source: Option<Box<dyn error::Error + 'static>>,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self.source {
            None => write!(f, "{}", self.msg),
            Some(src) => write!(f, "{}: {}", self.msg, src.to_string()),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None //FIXME
    }
}

impl Error {
    pub fn new(msg: &str) -> Error {
        Error {
            msg: msg.to_string(),
            source: None,
        }
    }

    fn wrap<T: std::error::Error + 'static>(msg: &str, source: T) -> Error {
        Error {
            msg: msg.to_string(),
            source: Some(Box::new(source)),
        }
    }
}

impl From<&str> for Error {
    fn from(msg: &str) -> Self {
        Error {
            msg: msg.to_string(),
            source: None,
        }
    }
}

impl From<hyper::Error> for Error {
    fn from(err: hyper::Error) -> Self {
        Error::wrap("HTTP error", err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::wrap("IO error", err)
    }
}

impl From<std::fmt::Error> for Error {
    fn from(err: std::fmt::Error) -> Self {
        Error::wrap("Formatting error", err)
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(err: zip::result::ZipError) -> Self {
        Error::wrap("Error extracting ZIP file", err)
    }
}

impl From<id3::Error> for Error {
    fn from(err: id3::Error) -> Self {
        Error::wrap("Error reading ID3 tag", err)
    }
}

impl From<google_youtube3::Error> for Error {
    fn from(err: google_youtube3::Error) -> Self {
        Error::wrap("YouTube error", err)
    }
}

impl From<serde_json::error::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::wrap("JSON error", err)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        Error::wrap("SQLite error", err)
    }
}

impl From<regex::Error> for Error {
    fn from(err: regex::Error) -> Self {
        Error::wrap("Regex error", err)
    }
}
