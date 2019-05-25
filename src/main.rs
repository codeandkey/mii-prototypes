#[macro_use] extern crate clap;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;

mod analysis;
mod crawl;
mod db;
mod engine;

use std::env;
use std::fs::DirBuilder;
use std::path::Path;

fn main() {
    let matches = clap_app!(mii =>
        (version: "0.1")
        (author: "Justin Stanley <jtst@iastate.edu>")
        (about: "Module Inverted Index")
        (@arg debug: -d --debug "Enable verbose logging to stderr")
        (@arg datadir: -s --datadir +takes_value "Override data directory")
        (@subcommand sync =>
            (about: "Synchronize module index")
        )
        (@subcommand build =>
            (about: "Rebuild module index")
        )
        (@subcommand exact =>
            (about: "Search for an exact command")
            (@arg command: +required "Command to search")
        )
        (@subcommand glob =>
            (about: "Search for similar commands")
            (@arg command: +required "Command hint")
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

    if let Some(matches) = matches.subcommand_matches("sync") {
        ctrl.sync_light();
    }

    if let Some(matches) = matches.subcommand_matches("build") {
        ctrl.destroy_db();
        println!("[mii] Rebuilding index..");
        ctrl.sync_light();
    }

    if let Some(matches) = matches.subcommand_matches("exact") {
        let res = ctrl.search_bin_exact(matches.value_of("command").unwrap().to_string());

        println!("[");
        for r in res {
            println!("    {{\"{}\":\"{}\"}},", r.code, r.command);
        }
        println!("]");
    }

    if let Some(matches) = matches.subcommand_matches("glob") {
        let res = ctrl.search_bin_fuzzy(matches.value_of("command").unwrap().to_string());

        println!("[");
        for r in res {
            println!("    {{\"{}\":\"{}\"}},", r.code, r.command);
        }
        println!("]");
    }
}
