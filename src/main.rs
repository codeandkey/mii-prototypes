#[macro_use]
extern crate log;

mod crawl;

use std::env;

fn main() {
    env::set_var("RUST_LOG", "mii");
    pretty_env_logger::init();

    info!("Initializing Mii engine..");

    let a = crawl::crawl_sync(None);

    for module in a {
        info!("{} : {} : {}", module.code, module.hash.unwrap(), module.path.display());
    }
}
