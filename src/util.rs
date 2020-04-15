use std::convert::From;
use std::error;

use google_youtube3 as youtube3;

pub const USER_AGENT: &str = "ektoboat/1";

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Error {
    msg: String,
    source: Option<Box<dyn error::Error + 'static>>,
    retry_later: bool,
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
            retry_later: false,
        }
    }

    fn wrap<T: std::error::Error + 'static>(msg: &str, source: T) -> Error {
        Error {
            msg: msg.to_string(),
            source: Some(Box::new(source)),
            retry_later: false,
        }
    }
}

impl From<&str> for Error {
    fn from(msg: &str) -> Self {
        Error {
            msg: msg.to_string(),
            source: None,
            retry_later: false,
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

impl From<youtube3::Error> for Error {
    fn from(err: youtube3::Error) -> Self {
        let mut retry = false;

        if let youtube3::Error::BadRequest(youtube3::ErrorResponse { ref error }) = err {
            if error.errors.iter().any(|e| e.reason == "quotaExceeded") {
                retry = true;
            }
        }

        let mut e = Error::wrap("YouTube error", err);
        e.retry_later = retry;
        e
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

pub fn retry<T, F>(retries: u32, t: chrono::Duration, f: F) -> Result<T>
where
    F: Fn() -> Result<T>,
{
    let mut res = f();
    for i in 0..retries {
        match &res {
            Err(e) if e.retry_later => {
                log::debug!("Retriable error: {:?}", e);
            },
            _ => return res,
        }

        log::info!("Retriable error ({} retries left), sleeping {}s", retries - i,  t.num_seconds());
        std::thread::sleep(t.to_std().expect("valid duration"));
        res = f();
    }
    res
}
