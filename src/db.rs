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

use rusqlite::{Connection, Result, NO_PARAMS, params};
use std::path::Path;

use crate::analysis;
use crate::crawl;

pub struct BinResult {
    pub code: String,
    pub command: String,
}

pub struct DB {
    conn: Connection,
}

impl DB {
    pub fn initialize(db_path: &Path) {
        match Connection::open(db_path) {
            Ok(conn) => {
                /* initialize database tables */
                conn.execute("CREATE TABLE IF NOT EXISTS modules (path TEXT UNIQUE, code TEXT, nonce INT, hash BIGINT, bins TEXT)", NO_PARAMS).unwrap();
            },
            Err(e) => {
                panic!("Failed to open database file {}: {}", db_path.display(), e);
            },
        }
    }

    pub fn new(db_path: &Path) -> DB {
        match Connection::open(db_path) {
            Ok(conn) => {
                conn.pragma_update(None, "journal_mode", &"WAL").unwrap();

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
        let ret;

        {
            let mut stmt = tx.prepare("UPDATE modules SET nonce=?, code=? WHERE path=? AND hash=?").unwrap();
            ret = local.into_iter().filter(|x| stmt.execute(params![nonce, x.code, x.path.to_string_lossy(), x.hash]).unwrap() < 1).collect();
        }

        tx.commit().expect("transaction failed");
        ret
    }

    /*
     * update_modules synchronizes local analyzed modules to the db
     */

    pub fn update_modules(&mut self, res: &Vec<analysis::Info>, nonce: u32) {
        let tx = self.conn.transaction().unwrap();

        {
            let mut stmt = tx.prepare("INSERT INTO modules VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(path) DO UPDATE SET code=?2, nonce=?3, hash=?4, bins=?5").unwrap();

            for m in res {
                stmt.execute(params![m.file.path.to_string_lossy(), m.file.code, nonce, m.file.hash, m.bins.join(":")]).unwrap();
            }
        }

        tx.commit().expect("transaction failed");
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
        let res;

        {
            /*
             * drop the orphaned module entries
             */

            res = tx.execute("DELETE FROM modules WHERE nonce!=$1", params![nonce]).unwrap();
        }

        tx.commit().expect("transaction failed");
        res
    }

    /*
     * search_bin searches the database for a command
     */

    pub fn search_bin(&self, command: String) -> Vec<BinResult> {
        let cmd_param = format!("%{}%", command);
        let mut stmt = self.conn.prepare("SELECT bins, code FROM modules WHERE bins LIKE ?").unwrap();

        stmt.query_map(params![cmd_param], |row| {
            let row_bin_col: String = row.get(0).unwrap();
            let row_bins: Vec<String> = row_bin_col.split(":").map(|x| x.to_string()).collect();

            if row_bins.contains(&command) {
                return Ok(Some(BinResult {
                    command: command.clone(),
                    code: row.get(1).unwrap(),
                }));
            }

            Ok(None)
        }).unwrap().filter_map(Result::ok).filter_map(|x| x).collect()
    }

    /*
     * search_bin_fuzzy searches the database for similar commands
     */

    pub fn search_bin_fuzzy(&self, command: String) -> Vec<BinResult> {
        let cmd_param = format!("%{}%", command);
        let mut stmt = self.conn.prepare("SELECT bins, code FROM modules WHERE bins LIKE ?").unwrap();

        let vecs: Vec<Vec<BinResult>> = stmt.query_map(params![cmd_param], |row| {
            let row_bin_col: String = row.get(0).unwrap();
            let row_bins: Vec<String> = row_bin_col.split(":").map(|x| x.to_string()).collect();

            let mut out = Vec::new();
            let row_code: String = row.get(1).unwrap();

            for bin in row_bins {
                if bin.contains(&command) {
                    out.push(BinResult {
                        command: bin,
                        code: row_code.clone(),
                    });
                }
            }

            Ok(out)
        }).unwrap().filter_map(Result::ok).collect();

        vecs.into_iter().flatten().collect()
    }

    /*
     * purge() clears out the whole module table
     */
    pub fn purge(&self) {
        self.conn.execute("DELETE FROM modules", NO_PARAMS).unwrap();
    }
}
