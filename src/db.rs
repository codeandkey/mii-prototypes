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

pub struct DB<'a> {
    conn: Connection,
    compare_module: Option<Statement<'a>>,
}

impl<'a> DB<'a> {
    pub fn initialize(db_path: &Path) {
        match Connection::open(db_path) {
            Ok(conn) => {
                /* initialize database tables */
                conn.execute("CREATE TABLE IF NOT EXISTS modules (id INT PRIMARY_KEY AUTO_INCREMENT, path TEXT UNIQUE, code TEXT, nonce INT, hash BIGINT)", NO_PARAMS).unwrap();
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
                    compare_module: None,
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

    pub fn init_statements(&'a mut self) {
        self.compare_module = Some(self.conn.prepare("UPDATE modules SET nonce=? code=? WHERE path=? AND hash=?").unwrap());
    }

    /*
     * compare_local_module checks if there is an up-to-date entry in the
     * local module database. returns TRUE if an analysis and update is required
     */

    pub fn compare_module(&mut self, local: &crawl::ModuleFile, nonce: u32) -> bool {
        self.conn.execute("UPDATE modules SET nonce=$1, code=$2 WHERE path=$3 AND hash=$4",
                          params![nonce, local.code, local.path.to_string_lossy(), local.hash]).unwrap() < 1
    }

    /*
     * update_module updates an existing module or adds a new one to the database
     */

    pub fn update_modules(&mut self, res: &Vec<analysis::Result>, nonce: u32) {
        let tx = self.conn.transaction().unwrap();

        {
            let mut stmt = tx.prepare("INSERT INTO modules VALUES (0, ?1, ?2, ?3, ?4) ON CONFLICT(path) DO UPDATE SET code=?2, nonce=?3, hash=?4").unwrap();

            for m in res {
                stmt.execute(params![m.file.path.to_string_lossy(), m.file.code, nonce, m.file.hash]).unwrap();
            }
        }

        tx.commit();
    }

    pub fn update_module(&self, local: &crawl::ModuleFile, res: &analysis::Result, nonce: u32) {
        self.conn.execute("INSERT INTO modules VALUES (0, $1, $2, $3, $4) ON CONFLICT(path) DO UPDATE SET code=$2, nonce=$3, hash=$4",
                          params![local.path.to_string_lossy(), local.code, nonce, local.hash]);
        debug!("updated module {} in local database", local.code);
    }

    /*
     * flush_orphans removes any module entry that fails the nonce test.
     * this will cover every module which no longer exists in the filesystem (orphaned entries)
     */

    fn flush_orphans(&self, nonce: u32) {
    }
}
