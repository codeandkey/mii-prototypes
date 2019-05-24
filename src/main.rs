#[macro_use] extern crate clap;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;

mod analysis;
mod crawl;
mod db;
mod engine;

use rand;
use std::env;
use std::fs::DirBuilder;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let matches = clap_app!(mii =>
        (version: "0.1")
        (author: "Justin Stanley <jtst@iastate.edu>")
        (about: "Module Inverted Index")
        (@arg debug: -d --debug "Enable verbose logging to stderr")
        (@arg datadir: -s --datadir +takes_value "Override data directory")
        (@subcommand verify =>
            (about: "Verify and synchronize module index")
        )
        (@subcommand build =>
            (about: "Build a clean module index")
        )
        (@subcommand exact =>
            (about: "Search for an exact command")
            (@arg command: +required "Command to search")
        )
    ).get_matches();

    if matches.is_present("debug") {
        env::set_var("RUST_LOG", "mii");
        pretty_env_logger::init();
    }

    /*
     * before starting the engine, make sure the database dir is good to go
     */

    let datadir = match matches.value_of("datadir") {
        Some(x) => x.to_string(),
        None => dirs::data_local_dir().unwrap().join("mii").to_string_lossy().to_string(),
    };

    let datadir = Path::new(&datadir);

    if let Err(e) = DirBuilder::new().recursive(true).create(&datadir) {
        panic!("Failed to initialize data directory in {} : {}", datadir.display(), e.to_string());
    }

    let mut ctrl = engine::Engine::new(env::var("MODULEPATH").unwrap_or(String::new()), datadir.join("index.db"));

    if let Some(matches) = matches.subcommand_matches("verify") {
        ctrl.sync_light();
    }

    if let Some(matches) = matches.subcommand_matches("build") {
        panic!("not implemented yet");
    }

    if let Some(matches) = matches.subcommand_matches("exact") {
        let res = ctrl.search_bin_exact(matches.value_of("command").unwrap().to_string());

        println!("[");
        for r in res {
            println!("    {{\"{}\":\"{}\"}},", r.code, r.command);
        }
        println!("]");
    }
}
