#[macro_use]
extern crate clap;
extern crate log;

mod config;
mod model;
mod source;
mod util;
mod youtube;

pub use crate::config::{run, Config}; // maybe merge w/ use in main.rs
