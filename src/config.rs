use std::path::PathBuf;

use crate::cli;
use crate::flow;
use crate::source;
use crate::store;
use crate::util;
use crate::video;
use crate::youtube;

pub enum Action {
    Help,
    Scrape(u32),
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

        if let Some(ref scrape_matches) = matches.subcommand_matches("scrape-ektoplazm") {
            let off: u32 = scrape_matches
                .value_of("offset")
                .unwrap()
                .parse()
                .expect("unsigned integer");
            config.action = Action::Scrape(off);
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

    pub fn filename(self: &Config, basename: &str) -> PathBuf {
        let mut p = self.appdir.clone();
        util::mkdir_if_not_exists(&p);
        p.push(basename);
        p
    }

    pub fn client_secret(&self) -> PathBuf {
        self.filename("client_secret.json")
    }

    pub fn db_path(&self) -> PathBuf {
        self.filename("state.sqlite3")
    }

    pub fn mp3_dir(&self) -> PathBuf {
        // TODO ~/.cache/ektoboat/mp3
        let dir = self.filename("mp3");
        util::mkdir_if_not_exists(&dir);
        dir
    }

    pub fn video_dir(&self) -> PathBuf {
        // TODO ~/.cache/ektoboat/video
        let dir = self.filename("video");
        util::mkdir_if_not_exists(&dir);
        dir
    }

    pub fn run(self) -> Result<(), util::Error> {
        let yt = || {
            youtube::YT::new(
                self.client_secret().as_path(),
                self.filename("youtube_token.json").as_path(),
            )
        };
        match &self.action {
            Action::Help => {
                // FIXME use different error type
                return Err(util::Error::new(
                    "You have to specify an action, use --help for help",
                ));
            }
            Action::Scrape(off) => {
                let mut store = store::Store::open(&self.db_path())?;

                for (i, x) in source::EktoplazmScraper::from_offset(*off).enumerate() {
                    // let (url, tracks, zipbytes) = x?;
                    // println!("{} {}\t{}\t{}", (i as u32)+off, tracks, zipbytes, url);
                    let url = x?;
                    println!("{} {}", (i as u32) + off, url);
                    store.queue_insert(&url)?;
                    // doesn't make much sense now that we don't query all urls
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                }
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
                let mut store = store::Store::open(&self.db_path())?;
                let album = source::fetch(url, &self.mp3_dir())?;
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
                let mut store = store::Store::open(&self.db_path())?;
                flow::run_url(url, &self, &mut store)?;
            }
            Action::Status(url) => {
                let mut store = store::Store::open(&self.db_path())?;
                match store.get_album(url)? {
                    None => {
                        println!("Not in database");
                    }
                    Some(album) => {
                        album.print();
                        println!("");
                        println!("Has all mp3s:   {:?}", album.has_mp3(&self.mp3_dir()));
                        println!("Has all videos: {:?}", album.has_video(&self.video_dir()));
                    }
                }
            }
        }

        Ok(())
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

