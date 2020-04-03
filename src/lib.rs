#[macro_use]
extern crate clap;
#[macro_use]
extern crate lazy_static;
extern crate log;

mod cli;
mod config;
mod model;
mod source;
mod util;
mod video;
mod youtube;

pub use crate::config::{run, Config}; // maybe merge w/ use in main.rs
