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

use crate::analysis;
use crate::crawl;

pub struct Module {
    code: String,
    bins: Vec<String>,
}

pub struct DB {
    conn: Connection,
}

impl DB {
    pub fn initialize(db_path: &Path) {
        match Connection::open(db_path) {
            Ok(conn) => {
                /* initialize database tables */
                conn.execute("CREATE TABLE IF NOT EXISTS modules (path TEXT UNIQUE, code TEXT, nonce INT, hash BIGINT)", NO_PARAMS).unwrap();
                conn.execute("CREATE TABLE IF NOT EXISTS bins (module_id INT, command TEXT)", NO_PARAMS).unwrap();
            },
            Err(e) => {
                panic!("Failed to open database file: {}", e);
            },
        }
    }

    pub fn new(db_path: &Path) -> DB {
        match Connection::open(db_path) {
            Ok(conn) => {
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
     * all connection operations work on one thread, and one object
     * multithreaded phases should be implemented elsewhere
     */

    /*
     * compare_modules checks if there is an up-to-date entry in the local db
     */

    pub fn compare_modules(&mut self, local: Vec<crawl::ModuleFile>, nonce: u32) -> Vec<crawl::ModuleFile> {
        let tx = self.conn.transaction().unwrap();
        let mut ret = Vec::new();

        {
            let mut stmt = tx.prepare("UPDATE modules SET nonce=?, code=? WHERE path=? AND hash=?").unwrap();
            ret = local.into_iter().filter(|x| stmt.execute(params![nonce, x.code, x.path.to_string_lossy(), x.hash]).unwrap() < 1).collect();
        }

        tx.commit();
        ret
    }

    /*
     * update_modules synchronizes local analyzed modules to the db
     */

    pub fn update_modules(&mut self, res: &Vec<analysis::Info>, nonce: u32) {
        let tx = self.conn.transaction().unwrap();

        {
            let mut stmt = tx.prepare("INSERT INTO modules (path, code, nonce, hash) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(path) DO UPDATE SET code=?2, nonce=?3, hash=?4").unwrap();
            let mut flush_bin_stmt = tx.prepare("DELETE FROM bins WHERE module_id=?").unwrap();
            let mut bin_stmt = tx.prepare("INSERT INTO bins VALUES (?, ?)").unwrap();

            for m in res {
                stmt.execute(params![m.file.path.to_string_lossy(), m.file.code, nonce, m.file.hash]).unwrap();

                let mod_id = tx.last_insert_rowid(); /* let's pray together that this works with an upsert clause */

                /* drop out-of-date bins from the db */
                flush_bin_stmt.execute(params![mod_id]).unwrap();

                /* add new bins back into the db */
                for bin in m.bins.iter() {
                    bin_stmt.execute(params![mod_id, bin]).unwrap();
                }
            }
        }

        tx.commit();
    }

    /*
     * flush_orphans removes any module entry that fails the nonce test.
     * this will cover every module which no longer exists in the filesystem (orphaned entries)
     *
     * returns number of orphaned modules
     */

    pub fn flush_orphans(&mut self, nonce: u32) -> usize {
        /*
         * TODO: this can almost assuredly be rewritten with fewer SQL statements,
         * possibly by using DELETE in conjunction with JOINS
         */

        let tx = self.conn.transaction().unwrap();
        let mut res = 0;

        {
            /*
             * first, find ids of modules we're deleting and drop
             * any orphaned bins
             */
            let mut stmt_loc = tx.prepare("SELECT rowid FROM modules WHERE nonce!=?").unwrap();
            let orphaned_iter: Vec<u32> = stmt_loc.query_map(params![nonce], |row| { Ok(row.get(0).unwrap()) }).unwrap().filter_map(Result::ok).collect();

            let mut stmt_drop = tx.prepare("DELETE FROM bins WHERE module_id=?").unwrap();
            for id in orphaned_iter {
                stmt_drop.execute(params![id]).unwrap();
            }

            /*
             * finally, drop the orphaned module entries
             */

            res = tx.execute("DELETE FROM modules WHERE nonce!=$1", params![nonce]).unwrap();
        }

        tx.commit();
        res
    }
}
