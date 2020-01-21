use clap::{App, Arg};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

use crate::model;
use crate::source;
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
    // TODO generate completion
    pub fn from_cmdline() -> Config {
        let mut config = Config::default();

        let matches = App::new(crate_name!())
            .about(crate_description!())
            .version(crate_version!())
            .setting(clap::AppSettings::VersionlessSubcommands)
            .arg(
                Arg::with_name("verbose")
                    .short("v")
                    .multiple(true)
                    .help("Increases message verbosity"),
            )
            .arg(
                Arg::with_name("statedir")
                    .short("d")
                    .takes_value(true)
                    .help("State directory"),
            )
            .subcommand(
                App::new("yt-upload")
                    .about("upload to YouTube")
                    .setting(clap::AppSettings::DisableVersion)
                    .arg(
                        Arg::with_name("title")
                            .long("title")
                            .takes_value(true)
                            .help("Title of the video")
                            .required(true),
                    )
                    .arg(
                        Arg::with_name("description")
                            .long("description")
                            .takes_value(true)
                            .value_name("FILE")
                            .help("File containing video description")
                            .default_value("/dev/null"),
                    )
                    .arg(
                        Arg::with_name("input_file")
                            .help("input file")
                            .value_name("FILE")
                            .index(1)
                            .required(true),
                    ),
            )
            .subcommand(
                App::new("yt-playlist")
                    .about("create YouTube playlist")
                    .setting(clap::AppSettings::DisableVersion)
                    .arg(
                        Arg::with_name("title")
                            .long("title")
                            .takes_value(true)
                            .help("Title of the video")
                            .required(true),
                    )
                    .arg(
                        Arg::with_name("description")
                            .long("description")
                            .takes_value(true)
                            .value_name("FILE")
                            .help("File containing video description")
                            .default_value("/dev/null"),
                    )
                    .arg(
                        Arg::with_name("video_ids")
                            .help("IDs of videos in the playlist")
                            .index(1)
                            .multiple(true)
                            .required(true),
                    ),
            )
            .subcommand(
                App::new("fetch")
                    .about("download MP3 archive as well as metadata")
                    .setting(clap::AppSettings::DisableVersion)
                    .arg(
                        Arg::with_name("url")
                            .help("URL of the album")
                            .index(1)
                            .required(true),
                    ),
            )
            .subcommand(
                App::new("video")
                    .about("convert MP3 file to video with still image")
                    .setting(clap::AppSettings::DisableVersion)
                    .arg(
                        Arg::with_name("out")
                            .long("out")
                            .takes_value(true)
                            .value_name("FILE")
                            .help("File name of the result"),
                    )
                    .arg(
                        Arg::with_name("audio_file")
                            .help("input file")
                            .index(1)
                            .required(true)
                            .help("Audio file for the video"),
                    )
                    .arg(
                        Arg::with_name("image_file")
                            .index(2)
                            .required(true)
                            .help("Image to be used for video (aka album cover)"),
                    ),
            )
            .get_matches();

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

        config
    }

    fn mkdir_if_not_exists(p: &Path) {
        if !p.exists() {
            log::info!("Creating directory {:?}", p);
            std::fs::DirBuilder::new().mode(0o770).create(&p).unwrap();
        }
    }

    fn filename(self: &Config, basename: &str) -> PathBuf {
        let mut p = self.appdir.clone();
        Self::mkdir_if_not_exists(&p);
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
        Self::mkdir_if_not_exists(&dir);
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
            for video_id in &playlist.videos {
                yt.add_video_to_playlist(playlist_id.clone(), video_id.clone())?;
                log::debug!("done");
            }
        }
        Action::Fetch(url) => {
            let store = model::Store::new(config.db_path());
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
    }

    Ok(())
}
