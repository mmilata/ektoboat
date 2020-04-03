#[macro_use]
extern crate clap;

use std::os::unix::fs::DirBuilderExt;

use clap::Shell;

include!("src/cli.rs");

fn main() {
    println!("cargo:rerun-if-changed=src/cli.rs");

    let mut comp_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    comp_dir.push("shell-completion");
    let _whatevs = std::fs::DirBuilder::new().mode(0o770).create(&comp_dir);

    let mut app = build_cli();
    app.gen_completions(crate_name!(), Shell::Bash, &comp_dir);
    app.gen_completions(crate_name!(), Shell::Zsh, &comp_dir);
    app.gen_completions(crate_name!(), Shell::Fish, &comp_dir);
}
