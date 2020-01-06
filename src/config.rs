use clap::{App, Arg};
use std::error::Error;
use std::path::PathBuf;

use crate::youtube;

pub enum Action {
    Help,
    YTUpload(youtube::Video),
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
                    //.default_value(config.appdir.to_str().unwrap())
                    .help("State directory"),
            )
            .subcommand(
                App::new("youtube")
                    .about("upload to youtube")
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
                            //.default_value("input.avi")
                            .index(1)
                            .required(true),
                    ),
            )
            .get_matches();

        //debug!("{} {}", crate_name!(), crate_version!());

        config.verbose = matches.occurrences_of("verbose") as usize;

        if matches.is_present("statedir") {
            config.appdir = PathBuf::from(matches.value_of("statedir").unwrap());
        }

        if let Some(ref youtube_matches) = matches.subcommand_matches("youtube") {
            config.action = Action::YTUpload(youtube::Video {
                title: youtube_matches.value_of("title").unwrap().to_string(),
                description: std::fs::read_to_string(youtube_matches.value_of("description").unwrap()).expect("read description"),
                tags: std::vec::Vec::new(),
                filename: PathBuf::from(youtube_matches.value_of("input_file").unwrap()),
            });
        }

        config
    }

    pub fn client_secret(&self) -> PathBuf {
        let mut p = self.appdir.clone();
        p.push("client_secret.json");
        p
    }

    pub fn filename(self: &Config, basename: &str) -> PathBuf {
        let mut p = self.appdir.clone();
        p.push(basename);
        p
    }
}

impl Default for Config {
    fn default() -> Config {
        let mut appdir = PathBuf::from(&std::env::var("HOME").unwrap_or("/".to_string()));
        appdir.push(".ektobot");

        Config {
            verbose: 0,
            appdir: appdir,
            action: Action::Help,
        }
    }
}

pub fn run(config: Config) -> Result<(), Box<dyn Error>> {
    match &config.action {
        Action::Help => {
            return Err(Box::new(youtube::YTError::new(
                "You have to specify an action, use --help for help".to_string(),
            )));
        }
        Action::YTUpload(video) => {
            let yt = youtube::YT::new(
                config.client_secret().as_path(),
                config.filename("youtube_token.json").as_path(),
            )?;
            println!("{}", yt.upload_video(video.clone())?);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name() {
        ()
    }
}
