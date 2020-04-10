#[macro_use]
extern crate clap;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate rusqlite;
extern crate log;

mod cli;
mod config;
mod model;
mod source;
mod store;
mod util;
mod video;
mod youtube;

pub use crate::config::{run, Config}; // maybe merge w/ use in main.rs
