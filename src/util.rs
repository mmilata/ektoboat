use hyper;
use std::convert::From;
use std::error;

pub const USER_AGENT: &str = "ektoboat/1";

#[derive(Debug)]
pub struct Error {
    msg: String,
    source: Option<Box<dyn error::Error>>,
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
        Error {
            msg: "HTTP error".to_string(),
            source: Some(Box::new(err)),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error {
            msg: "IO error".to_string(),
            source: Some(Box::new(err)),
        }
    }
}

impl From<std::fmt::Error> for Error {
    fn from(err: std::fmt::Error) -> Self {
        Error {
            msg: "Formatting error".to_string(),
            source: Some(Box::new(err)),
        }
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(err: zip::result::ZipError) -> Self {
        Error {
            msg: "Error extracting ZIP file".to_string(),
            source: Some(Box::new(err)),
        }
    }
}

impl From<id3::Error> for Error {
    fn from(err: id3::Error) -> Self {
        Error {
            //msg: "Error reading ID3 tag".to_string(),
            msg: format!("Error reading ID3 tag: {}", err),
            source: Some(Box::new(err)),
        }
    }
}

impl From<google_youtube3::Error> for Error {
    fn from(err: google_youtube3::Error) -> Self {
        Error {
            msg: format!("YouTube error: {}", err),
            source: Some(Box::new(err)),
        }
    }
}

impl From<serde_json::error::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error {
            msg: format!("JSON error: {}", err),
            source: Some(Box::new(err)),
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        Error {
            msg: format!("SQLite error: {}", err),
            source: Some(Box::new(err)),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
