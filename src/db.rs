/*
 * db.rs
 *
 * structure for interacting with the local database
 * database write performance could potentially benefit from multiple
 * writers on different threads -- however, we'll make this implementation
 * focus primarily on multithreading the module parser.
 *
 * that alone with the differential module analysis should increase performance by
 * a great amount.
 */

use rusqlite::{Connection, Result, NO_PARAMS, params, Statement};
use std::path::{Path, PathBuf};

use crate::crawl;

pub struct Module {
    code: String,
    bins: Vec<String>,
}

pub struct DB {
    conn: Connection,
}

impl DB {
    pub fn new(db_path: &Path) -> DB {
        match Connection::open(db_path) {
            Ok(conn) => {
                /* initialize database tables */
                conn.execute("CREATE TABLE IF NOT EXISTS modules (id INT PRIMARY_KEY AUTO_INCREMENT, path TEXT UNIQUE, code TEXT, nonce INT, hash BIGINT)", NO_PARAMS).unwrap();
                conn.execute("CREATE TABLE IF NOT EXISTS bins (module_id INT, command TEXT)", NO_PARAMS).unwrap();

                DB {
                    conn: conn,
                }
            },
            Err(e) => {
                panic!("Failed to open database file: {}", e);
            },
        }
    }

    /*
     * sync()
     *
     * synchronizes the database with a list of local modules
     * performs module analysis ONLY when modules need to be updated, or when
     * they do not yet exist in the datbase.
     * removes modules which no longer exist on disk from the database. this
     * is accomplished via 3 "phases" leaving the orphaned module entries in
     * a unique state.
     *
     * (0) A random nonce is chosen.
     * (1) Where the path and hash are identical, update modules rows with the nonce.
     * (2) ...
     */
    pub fn sync(modules: Vec<crawl::ModuleFile>) {
    }

    fn add_module(&self, module: Module) {
    }
}
