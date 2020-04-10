extern crate log;
extern crate stderrlog;

use std::process;

use ektoboat::{run, Config};

fn init_logging(config: &Config) {
    stderrlog::new()
        // To enable logging from extra crates just add another call to module() with the name of the crate.
        .module(module_path!())
        .verbosity(1 + config.verbose)
        .timestamp(stderrlog::Timestamp::Off)
        .color(stderrlog::ColorChoice::Never)
        .init()
        .unwrap();
}

fn main() {
    let config = Config::from_cmdline();

    init_logging(&config);

    if let Err(e) = run(config) {
        log::error!("Error: {}", e);
        process::exit(1);
    }
}
