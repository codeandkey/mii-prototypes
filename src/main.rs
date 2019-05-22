#[macro_use]
extern crate log;

mod crawl;
mod db;

use std::env;
use std::path::Path;

fn main() {
    env::set_var("RUST_LOG", "mii");
    pretty_env_logger::init();

    info!("Initializing Mii engine..");

    let a = crawl::crawl_sync(None);
    let db = db::DB::new(&Path::new("neat"));

    for module in a {
        info!("{} : {} : {}", module.code, module.hash.unwrap(), module.path.display());
    }
}
