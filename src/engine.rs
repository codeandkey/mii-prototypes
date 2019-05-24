/*
 * engine.rs
 *
 * mii engine interface
 * nothing else needs to be touched from the entry point (main.rs)
 *
 * abstracts away sync phases and introduces multithreading optimizations
 */

use crate::analysis;
use crate::crawl;
use crate::db;

use std::env;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

pub struct Engine {
    db_path: String,
    db_conn: db::DB,
    modulepath: String,
}

impl Engine {
    pub fn new(modulepath: String, db_path: String) -> Engine {
        db::DB::initialize(Path::new(&db_path));

        Engine {
            db_conn: db::DB::new(Path::new(&db_path)),
            db_path: db_path,
            modulepath: modulepath,
        }
    }

    /*
     * sync_light() performs a diff synchronization between the disk and the db.
     * it goes through all 5 phases and performs threading optimizations when
     * possible. the function returns once the sync is completed.
     *
     * sync_light() conditionally performs analysis; if the local filesystem is validated then
     * no changes will be made to the db.
     *
     * It is recommended that the sync_light() method is called on every login.
     * This will verify the integrity of the module index and is very fast when
     * no work has to be done. (It's still pretty good on rebuilds too though)
     */
    pub fn sync_light(&mut self) {
        let nonce = rand::random::<u32>();

        /* crawl phase: singlethreaded */

        debug!("Starting crawl phase.");
        let files = crawl::crawl_sync(self.modulepath.clone());

        /* verify phase: multithreaded! */

        debug!("Starting verify phase ({})..", files.len());
        let (tx, rx): (mpsc::Sender<Vec<crawl::ModuleFile>>, mpsc::Receiver<Vec<crawl::ModuleFile>>) = mpsc::channel();

        /* 
         * each worker thread will perform verify ops and send back modules requiring updates
         * through the mpsc.
         */

        let verify_chunk_size = files.len() / num_cpus::get() + 1;
        let mut workers = Vec::new();

        for chunk in files.chunks(verify_chunk_size) {
            let db_copy = self.db_path.clone();
            let chunk_copy = chunk.to_owned(); /* necessary as of right now. probably slow with large number of modules... */
            let tx_copy = tx.clone();

            workers.push(thread::spawn(move || {
                let mut db = db::DB::new(Path::new(&db_copy));
                tx_copy.send(db.compare_modules(chunk_copy.to_vec(), nonce)).unwrap();
            }));
        }

        debug!("Waiting for {} verify workers..", workers.len());

        let mut verify_results = Vec::new();

        /* 
         * seems hacky -- but we know exactly how many messages to expect through the mpsc
         * each worker will send exactly one result batch.
         *
         * TODO: any panics in worker threads will stop everything. the total numebr of messages
         * will never be received and the engine will hang indefinitely (no bueno). there should be 
         * proper callbacks or at least polling for panics from the main thread
         */
        for _ in workers.iter() {
            verify_results.extend(rx.recv().unwrap());
        }

        for worker in workers {
            worker.join();
        }

        debug!("Starting analysis phase ({})..", verify_results.len());

        let analysis_chunk_size = verify_results.len() / num_cpus::get() + 1;
        let mut analysis_workers = Vec::new();

        for chunk in verify_results.chunks(analysis_chunk_size) {
            let db_copy = self.db_path.clone();
            let chunk_copy = chunk.to_owned();

            analysis_workers.push(thread::spawn(move || {
                let mut db = db::DB::new(Path::new(&db_copy));
                db.update_modules(&chunk_copy.to_vec().into_iter().map(|x| analysis::analyze(x)).filter_map(Result::ok).collect(), nonce);
            }));
        }

        debug!("Waiting for {} analysis workers..", analysis_workers.len());

        for worker in analysis_workers {
            worker.join();
        }

        debug!("Starting orphan phase..");
        self.db_conn.flush_orphans(nonce);

        debug!("All done!");
    }

    pub fn search_bin_exact(&self, cmd: String) {}
}
