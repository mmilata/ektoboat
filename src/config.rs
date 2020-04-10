use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

use crate::cli;
use crate::source;
use crate::store;
use crate::store::Store;
use crate::util;
use crate::video;
use crate::youtube;

pub enum Action {
    Help,
    YTUpload(youtube::Video),
    YTPlaylist(youtube::Playlist),
    Fetch(String),
    Video {
        input: PathBuf,
        image: PathBuf,
        output: PathBuf,
    },
    URL(String),
    Status(String),
}

pub struct Config {
    pub verbose: usize,
    pub appdir: PathBuf,
    pub action: Action,
    // yt credentials
    // path to db // or handle?
}

impl Config {
    // XXX logging is not set up at this point
    pub fn from_cmdline() -> Config {
        let mut config = Config::default();
        let matches = cli::build_cli().get_matches();

        config.verbose = matches.occurrences_of("verbose") as usize;

        if matches.is_present("statedir") {
            config.appdir = PathBuf::from(matches.value_of("statedir").unwrap());
        }

        if let Some(ref youtube_matches) = matches.subcommand_matches("yt-upload") {
            config.action = Action::YTUpload(youtube::Video {
                title: youtube_matches.value_of("title").unwrap().to_string(),
                description: std::fs::read_to_string(
                    youtube_matches.value_of("description").unwrap(),
                )
                .expect("read description"),
                tags: std::vec::Vec::new(),
                filename: PathBuf::from(youtube_matches.value_of("input_file").unwrap()),
            });
        }
        if let Some(ref playlist_matches) = matches.subcommand_matches("yt-playlist") {
            config.action = Action::YTPlaylist(youtube::Playlist {
                title: playlist_matches.value_of("title").unwrap().to_string(),
                description: std::fs::read_to_string(
                    playlist_matches.value_of("description").unwrap(),
                )
                .expect("read description"),
                tags: std::vec::Vec::new(),
                videos: playlist_matches
                    .values_of("video_ids")
                    .unwrap()
                    .map(|s| youtube::VideoID(s.to_string()))
                    .collect(),
            });
        }
        if let Some(ref fetch_matches) = matches.subcommand_matches("fetch") {
            config.action = Action::Fetch(fetch_matches.value_of("url").unwrap().to_string());
        }
        if let Some(ref video_matches) = matches.subcommand_matches("video") {
            let infile = PathBuf::from(video_matches.value_of("audio_file").unwrap());
            let outfile = match video_matches.value_of("out") {
                Some(o) => PathBuf::from(o),
                None => {
                    let mut p = PathBuf::from(std::env::current_dir().unwrap());
                    p.push(infile.file_name().expect("audio file name"));
                    p.set_extension("avi");
                    p
                }
            };
            config.action = Action::Video {
                input: infile,
                output: outfile,
                image: PathBuf::from(video_matches.value_of("image_file").unwrap()),
            };
        }
        if let Some(ref url_matches) = matches.subcommand_matches("url") {
            config.action = Action::URL(url_matches.value_of("url").unwrap().to_string());
        }
        if let Some(ref status_matches) = matches.subcommand_matches("status") {
            config.action = Action::Status(status_matches.value_of("url").unwrap().to_string());
        }

        config
    }

    fn filename(self: &Config, basename: &str) -> PathBuf {
        let mut p = self.appdir.clone();
        mkdir_if_not_exists(&p);
        p.push(basename);
        p
    }

    pub fn client_secret(&self) -> PathBuf {
        self.filename("client_secret.json")
    }

    pub fn db_path(&self) -> PathBuf {
        self.filename("state.json")
    }

    pub fn mp3_dir(&self) -> PathBuf {
        // TODO ~/.cache/ektoboat/mp3
        let dir = self.filename("mp3");
        mkdir_if_not_exists(&dir);
        dir
    }

    pub fn video_dir(&self) -> PathBuf {
        // TODO ~/.cache/ektoboat/video
        let dir = self.filename("video");
        mkdir_if_not_exists(&dir);
        dir
    }
}

impl Default for Config {
    fn default() -> Config {
        let mut appdir = PathBuf::from(&std::env::var("HOME").unwrap_or("/".to_string()));
        appdir.push(".ektoboat");

        Config {
            verbose: 0,
            appdir: appdir,
            action: Action::Help,
        }
    }
}

pub fn run(config: Config) -> Result<(), util::Error> {
    let yt = || {
        youtube::YT::new(
            config.client_secret().as_path(),
            config.filename("youtube_token.json").as_path(),
        )
    };
    match &config.action {
        Action::Help => {
            // FIXME use different error type
            return Err(util::Error::new(
                "You have to specify an action, use --help for help",
            ));
        }
        Action::YTUpload(video) => {
            println!("{}", yt()?.upload_video(video.clone())?.as_url());
        }
        Action::YTPlaylist(playlist) => {
            let yt = yt()?;
            let playlist_id = yt.create_playlist(playlist.clone())?;
            println!("{}", playlist_id.as_url());
        }
        Action::Fetch(url) => {
            let mut store = store::JsonStore::open(&config.db_path())?;
            let album = source::fetch(url, &config.mp3_dir())?;
            store.save(&album)?;
        }
        Action::Video {
            input,
            image,
            output,
        } => {
            video::convert_file(input, image, output)?;
            println!("{:?}", output.canonicalize()?);
        }
        Action::URL(url) => {
            let mut store = store::JsonStore::open(&config.db_path())?;
            run_url(url, &config, &mut store)?;
        }
        Action::Status(url) => {
            let mut store = store::JsonStore::open(&config.db_path())?;
            match store.get_album(url)? {
                None => {
                    println!("Not in database");
                }
                Some(album) => {
                    album.print();
                    println!("");
                    println!("Has all mp3s:   {:?}", album.has_mp3(&config.mp3_dir()));
                    println!("Has all videos: {:?}", album.has_video(&config.video_dir()));
                }
            }
        }
    }

    Ok(())
}

fn run_url(url: &str, config: &Config, store: &mut store::JsonStore) -> Result<(), util::Error> {
    log::info!("Processing {}", url);
    let mut album = match store.get_album(url)? {
        None => source::fetch(url, &config.mp3_dir())?,
        Some(album) => {
            if album.has_mp3(&config.mp3_dir()) {
                album
            } else {
                log::warn!("Album has missing audio files, re-fetching");
                source::fetch(url, &config.mp3_dir())?
            }
        }
    };
    store.save(&album)?;

    let album_video_dir = album.dirname(&config.video_dir());
    if !album.has_video(&config.video_dir()) {
        let cover_img = video::find_cover(&album.dirname(&config.mp3_dir()))?;
        let album_mp3_dir = album.dirname(&config.mp3_dir());
        mkdir_if_not_exists(&album_video_dir);

        for mut tr in &mut album.tracks {
            let basename = tr.mp3_file.as_ref().ok_or("MP3 file missing")?;
            let mut mp3_file = album_mp3_dir.clone();
            mp3_file.push(basename);

            let mut video_file = album_video_dir.clone();
            let basename = basename.with_extension("avi");
            video_file.push(basename.clone());

            video::convert_file(&mp3_file, &cover_img, &video_file)?;

            tr.video_file = Some(basename);
        }
        store.save(&album)?;
    }

    let yt = youtube::YT::new(
        config.client_secret().as_path(),
        config.filename("youtube_token.json").as_path(),
    )?;
    // generate descriptions first, can't use reference to album inside the for loop
    let descriptions = album
        .tracks
        .iter()
        .map(|t| source::description(&album, t))
        .collect::<Result<Vec<_>, _>>()?;
    let album_tags = album.tags.clone();
    for (mut tr, desc) in album.tracks.iter_mut().zip(descriptions.into_iter()) {
        if let Some(yt_id) = &tr.youtube_id {
            log::debug!(
                "Track {} already has youtube id {}",
                tr.title,
                yt_id.as_url()
            );
            continue;
        }

        let mut video_file = album_video_dir.clone();
        video_file.push(tr.video_file.as_ref().ok_or("Video file missing")?);
        let args = youtube::Video {
            title: format!("{} - {}", tr.artist, tr.title),
            description: desc,
            tags: album_tags.clone(),
            filename: video_file,
        };
        tr.youtube_id = Some(yt.upload_video(args)?);
        //TODO!!! store.save(&album)?;
    }
    store.save(&album)?;
    //TODO: might delete videos here

    if album.youtube_id.is_none() && album.tracks.iter().all(|t| t.youtube_id.is_some()) {
        let args = youtube::Playlist {
            title: youtube::playlist_title(&album.title, &album.artist, &album.year, &album.tags),
            description: String::new(), // the description is not really visible
            tags: album_tags,
            videos: album
                .tracks
                .iter()
                .map(|t| t.youtube_id.clone().expect("Video ID missing"))
                .collect(),
        };
        album.youtube_id = Some(yt.create_playlist(args)?);
        store.save(&album)?;
    }

    Ok(())
}

fn mkdir_if_not_exists(p: &Path) {
    if !p.exists() {
        log::info!("Creating directory {:?}", p);
        std::fs::DirBuilder::new().mode(0o770).create(&p).unwrap();
    }
}
