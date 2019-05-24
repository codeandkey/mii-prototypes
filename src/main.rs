#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;

mod analysis;
mod crawl;
mod db;

use rand;
use std::env;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    env::set_var("RUST_LOG", "mii");
    pretty_env_logger::init();

    info!("Initializing Mii engine..");

    info!("Performing crawl phase..");

    let crawl_time = SystemTime::now();
    let a = crawl::crawl_sync(None);
    debug!("Finished crawl phase in {} ms", SystemTime::now().duration_since(crawl_time).unwrap().as_millis());

    db::DB::initialize(&Path::new("neat"));

    let mut db = db::DB::new(&Path::new("neat"));

    let nonce = rand::random::<u32>();

    info!("Performing verify phase on {} entries..", a.len());
    let verify_time = SystemTime::now();

    let to_update = db.compare_modules(a, nonce);

    debug!("Finished verify phase in {} ms", SystemTime::now().duration_since(verify_time).unwrap().as_millis());
    info!("Performing analysis phase on {} modules..", to_update.len());
    
    let analysis_time = SystemTime::now();
    let analysis_results: Vec<analysis::Info> = to_update.into_iter().map(|x| analysis::analyze(x)).filter_map(Result::ok).collect();

    debug!("Finished analysis phase in {} ms", SystemTime::now().duration_since(analysis_time).unwrap().as_millis());
    info!("Performing update phase on {} modules..", analysis_results.len());

    let update_time = SystemTime::now();
    db.update_modules(&analysis_results, nonce);
    debug!("Finished update phase in {} ms", SystemTime::now().duration_since(update_time).unwrap().as_millis());

    info!("Performing orphan phase..");
    let orphan_time = SystemTime::now();
    let res = db.flush_orphans(nonce);
    debug!("Finished orphan phase ({} orphans killed) in {} ms", res, SystemTime::now().duration_since(orphan_time).unwrap().as_millis());

    for i in db.search_bin("ls".to_string(), false) {
        println!("{} provided by : {}", i.command, i.code);
    }
}
