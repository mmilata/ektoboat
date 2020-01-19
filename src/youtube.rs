use crate::util;
use std::error;
use std::fs;
use std::path::{Path, PathBuf};
use std::result::Result;
use std::vec::Vec;

use google_youtube3 as youtube3;
use hyper;
use hyper_rustls;
use yup_oauth2 as oauth2;
use yup_oauth2::{ApplicationSecret, Authenticator, AuthenticatorDelegate, DiskTokenStorage};
//use yup_hyper_mock as hyper_mock;
use youtube3::YouTube;

#[derive(Clone, Debug)]
pub struct VideoID(pub String);

impl std::fmt::Display for VideoID {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl VideoID {
    pub fn as_url(&self) -> String {
        format!("https://www.youtube.com/watch?v={}", self.0)
    }
}

#[derive(Clone, Debug)]
pub struct PlaylistID(pub String);

impl std::fmt::Display for PlaylistID {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl PlaylistID {
    pub fn as_url(&self) -> String {
        format!("https://www.youtube.com/playlist?list={}", self.0)
    }
}

#[derive(Clone, Debug)]
pub struct Video {
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub filename: PathBuf,
}

#[derive(Clone, Debug)]
pub struct Playlist {
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub videos: Vec<VideoID>,
}

// api quota increase request form: https://support.google.com/youtube/contact/yt_api_form?hl=en
pub struct YT {
    hub: google_youtube3::YouTube<
        hyper::Client,
        oauth2::Authenticator<EktoAuthenticatorDelegate, oauth2::DiskTokenStorage, hyper::Client>,
    >,
}

// scope is probably https://www.googleapis.com/auth/youtube.upload
impl YT {
    pub fn new(client_secret_path: &Path, token_storage_path: &Path) -> Result<YT, util::Error> {
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
                util::Error::new(&format!("{}: {}", client_secret_path.display(), e))
            })?;
        //FIXME ewww
        let token_storage =
            DiskTokenStorage::new(&token_storage_path.to_str().unwrap().to_string())?;
        // Provide your own `AuthenticatorDelegate` to adjust the way it operates and get feedback about
        // what's going on.
        let auth = Authenticator::new(
            &secret,
            EktoAuthenticatorDelegate,
            client(),
            token_storage,
            Some(oauth2::FlowType::InstalledInteractive),
        );
        let hub = YouTube::new(client(), auth);

        Ok(YT { hub: hub })
    }

    pub fn upload_video(&self, video: Video) -> Result<VideoID, util::Error> {
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

        // The Error enum provides details about what exactly happened.
        // You can also just use its `Debug`, `Display` or `Error` traits
        let (res, inserted_video) = result?;
        log::debug!("Video upload success: {:?}", res);
        log::debug!("Result: {:?}", inserted_video);

        let video_id = match inserted_video.id {
            None => return Err(util::Error::new("API did not return video id")),
            Some(s) => s,
        };

        Ok(VideoID(video_id))
    }

    pub fn create_playlist(&self, playlist: Playlist) -> Result<PlaylistID, util::Error> {
        let mut p = youtube3::Playlist::default();

        let mut pstatus = youtube3::PlaylistStatus::default();
        pstatus.privacy_status = Some("public".to_string());
        p.status = Some(pstatus);

        let mut psnippet = youtube3::PlaylistSnippet::default();
        psnippet.title = Some(playlist.title);
        psnippet.description = Some(playlist.description);
        psnippet.tags = Some(playlist.tags);
        p.snippet = Some(psnippet);

        let result = self.hub.playlists().insert(p).doit();

        let (res, inserted_playlist) = result?;
        log::debug!("Success: {:?}", res);
        log::debug!("Result: {:?}", inserted_playlist);

        match inserted_playlist.id {
            None => {
                return Err(util::Error::new("API did not return playlist id"));
            }
            Some(s) => Ok(PlaylistID(s)),
        }
    }

    pub fn add_video_to_playlist(
        &self,
        playlist_id: PlaylistID,
        video_id: VideoID,
    ) -> Result<(), util::Error> {
        log::info!("Adding to playlist");
        let mut pi = youtube3::PlaylistItem::default();

        let mut resid = youtube3::ResourceId::default();
        resid.kind = Some("youtube#video".to_string());
        resid.video_id = Some(video_id.0);

        let mut psnippet = youtube3::PlaylistItemSnippet::default();
        psnippet.playlist_id = Some(playlist_id.0);
        psnippet.resource_id = Some(resid);

        pi.snippet = Some(psnippet);

        let result = self.hub.playlist_items().insert(pi).doit();
        let (res, pi) = result?;

        log::debug!("Success: {:?}", res);
        log::debug!("Result: {:?}", pi);
        Ok(())
    }
}

struct EktoAuthenticatorDelegate;

impl AuthenticatorDelegate for EktoAuthenticatorDelegate {
    fn connection_error(&mut self, e: &hyper::Error) -> oauth2::Retry {
        log::error!("YouTube OAuth2 connection error: {}", e);
        oauth2::Retry::Abort
    }

    fn token_storage_failure(&mut self, is_set: bool, e: &dyn error::Error) -> oauth2::Retry {
        let _ = is_set;
        log::error!("YouTube OAuth2 token storage failure: {}", e);
        oauth2::Retry::Abort
    }

    fn token_refresh_failed(&mut self, error: &String, error_description: &Option<String>) {
        log::error!(
            "YouTube OAuth2 cannot get refresh token: {}: {}",
            error,
            error_description
                .as_ref()
                .unwrap_or(&"(no description)".to_string())
        );
    }

    fn present_user_url(&mut self, url: &String, need_code: bool) -> Option<String> {
        if need_code {
            log::error!(
                "Please direct your browser to {}, follow the instructions and enter the code displayed here: ",
                url
            );

            let mut code = String::new();
            std::io::stdin().read_line(&mut code).ok().map(|_| code)
        } else {
            // should never happen
            log::error!(
                "Please direct your browser to {} and follow the instructions displayed there.",
                url
            );
            None
        }
    }
}
