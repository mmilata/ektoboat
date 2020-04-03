use clap::{App, Arg};

pub fn build_cli() -> clap::App<'static, 'static> {
    App::new(crate_name!())
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
        .subcommand(
            App::new("url")
                .about("process source URL - download, convert to videos, upload to youtube")
                .setting(clap::AppSettings::DisableVersion)
                .arg(Arg::with_name("url").index(1).required(true)),
        )
        .subcommand(
            App::new("status")
                .about("show URL status")
                .setting(clap::AppSettings::DisableVersion)
                .arg(Arg::with_name("url").index(1).required(true)),
        )
}
