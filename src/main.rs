#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;
extern crate stderrlog;

use clap::{App, Arg};

fn init_logging(matches: &clap::ArgMatches) {
    let verbose = matches.occurrences_of("verbose") as usize;
    stderrlog::new()
        // To enable logging from extra crates just add another call to module() with the name of the crate.
        .module(module_path!())
        // .quiet(quiet)
        .verbosity(1 + verbose)
        .timestamp(stderrlog::Timestamp::Off)
        .color(stderrlog::ColorChoice::Never)
        .init()
        .unwrap();
}

fn main() {
    // TODO struct?
    // TODO generate completion
    let matches = App::new(crate_name!())
        .about(crate_description!())
        .version(crate_version!())
        .setting(clap::AppSettings::VersionlessSubcommands)
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .multiple(true)
                .help("Increases message verbosity"))
        .subcommand(
            App::new("youtube")
                .about("upload to youtube")
                .setting(clap::AppSettings::DisableVersion)
                .arg(
                    Arg::with_name("input_file")
                        .help("input file")
                        // .default_value("input.avi")
                        .index(1)
                        .required(true)
                ))
        .get_matches();

    init_logging(&matches);

    debug!("{} {}", crate_name!(), crate_version!());
    warn!("warn message");
    error!("error message");
    println!("Hello, world!");
}
