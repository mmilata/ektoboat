extern crate google_youtube3 as youtube3;
extern crate hyper;
extern crate hyper_rustls;
//extern crate yup_hyper_mock as hyper_mock;
extern crate yup_oauth2 as oauth2;

use oauth2::{ApplicationSecret, Authenticator, DefaultAuthenticatorDelegate, DiskTokenStorage};
use std::default::Default;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::result::Result;
use std::vec::Vec;
use youtube3::YouTube;

// api quota increase request form: https://support.google.com/youtube/contact/yt_api_form?hl=en
pub struct YT {
    hub: google_youtube3::YouTube<
        hyper::Client,
        oauth2::Authenticator<
            oauth2::DefaultAuthenticatorDelegate,
            oauth2::DiskTokenStorage,
            hyper::Client,
        >,
    >,
}

#[derive(Debug)]
pub struct YTError {
    msg: String,
}

impl std::fmt::Display for YTError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl std::error::Error for YTError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl YTError {
    pub fn new(msg: String) -> YTError {
        YTError {
            msg: msg.to_string(),
        }
    }
}

#[derive(Clone)]
pub struct Video {
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub filename: PathBuf,
}

// scope is probably https://www.googleapis.com/auth/youtube.upload
impl YT {
    pub fn new(
        client_secret_path: &Path,
        token_storage_path: &Path,
    ) -> Result<YT, Box<dyn Error>> {
        let client = || {
                hyper::Client::with_connector(hyper::net::HttpsConnector::new(
                    hyper_rustls::TlsClient::new(),
                ))
                /*
                hyper::Client::with_connector(hyper_mock::TeeConnector {
                    connector: hyper::net::HttpsConnector::new(hyper_rustls::TlsClient::new()),
                })
                */
        };

        let secret: ApplicationSecret = oauth2::read_application_secret(client_secret_path)
            .map_err(|e| {
                log::error!("Looks like client_secret.json is missing. Please go to https://console.developers.google.com/apis/credentials, create OAuth Client ID, and save the credentials to {}", client_secret_path.display());
                YTError::new(format!("{}: {}", client_secret_path.display(), e))
            })?;
        //FIXME ewww
        let token_storage =
            DiskTokenStorage::new(&token_storage_path.to_str().unwrap().to_string())?;
        // Provide your own `AuthenticatorDelegate` to adjust the way it operates and get feedback about
        // what's going on.
        let auth = Authenticator::new(
            &secret,
            DefaultAuthenticatorDelegate,
            client(),
            token_storage,
            Some(oauth2::FlowType::InstalledInteractive),
        );
        let hub = YouTube::new(client(), auth);

        Ok(YT { hub: hub })
    }

    pub fn upload_video(&self, video: Video) -> Result<String, Box<dyn Error>> {
        let mut v = youtube3::Video::default();
        v.snippet = Some(youtube3::VideoSnippet {
            title: Some(video.title),
            description: Some(video.description),
            tags: Some(video.tags),
            default_audio_language: None,
            channel_id: None,
            published_at: None,
            live_broadcast_content: None,
            default_language: None,
            thumbnails: None,
            category_id: None,
            localized: None,
            channel_title: None,
        });
        let f = fs::File::open(video.filename)?;
        let result = self
            .hub
            .videos()
            .insert(v)
            .upload_resumable(f, "application/octet-stream".parse().unwrap());

        let inserted_video = match result {
            Err(e) => match e {
                // The Error enum provides details about what exactly happened.
                // You can also just use its `Debug`, `Display` or `Error` traits
                youtube3::Error::HttpError(_)
                | youtube3::Error::MissingAPIKey
                | youtube3::Error::MissingToken(_)
                | youtube3::Error::Cancelled
                | youtube3::Error::UploadSizeLimitExceeded(_, _)
                | youtube3::Error::Failure(_)
                | youtube3::Error::BadRequest(_)
                | youtube3::Error::FieldClash(_)
                | youtube3::Error::JsonDecodeError(_, _) => {
                    log::error!("Youtube error: {}", e);
                    return Err(Box::new(e));
                }
            },
            Ok((res, vid)) => {
                log::debug!("success: {:?}", res);
                log::debug!("result: {:?}", vid);
                vid
            }
        };

        let video_id = match inserted_video.id {
            None => {
                return Err(Box::new(YTError::new(
                    "API did not return video id".to_string(),
                )))
            }
            Some(s) => s,
        };

        Ok(video_id)
    }
}
