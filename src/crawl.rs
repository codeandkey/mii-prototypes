/*
 * crawl.rs
 *
 * optimized directory crawler
 * uses mpsc + two threads to scan directories and find modules
 *
 * first thread scans for files and computes module codes
 * second thread reads the module file contents and hashes them
 */

use fasthash::xx;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;

pub enum ModuleType {
    LMOD,
    TCL,
}

pub struct ModuleFile {
    pub path: PathBuf,
    pub code: String,
    pub modtype: ModuleType,
    pub hash: Option<u64>, /* not filled at first. */
}

pub fn crawl_sync(modulepath: Option<String>) -> Vec<ModuleFile> {
    let mut output = Vec::new();

    let modulepath = match modulepath {
        Some(path) => path,
        None => env::var("MODULEPATH").unwrap_or(String::new()),
    };

    let roots: Vec<String> = modulepath.split(':').map(|x| x.to_string()).collect();

    let (tx, rx): (Sender<Option<ModuleFile>>, Receiver<Option<ModuleFile>>) = channel();

    let walker = thread::spawn(move || {
        crawl_gen(roots, tx);
    });

    /* wait for file locations to come back through the channel */

    let mut num_module_files = 0;

    while let Some(loc) = rx.recv().unwrap() {
        info!("received module {} : {}", loc.code, loc.path.display());
        num_module_files += 1;

        match File::open(&loc.path).and_then(|mut f| {
            let mut contents = Vec::new();
            f.read_to_end(&mut contents)?;

            Ok(contents)
        }) {
            Err(e) => warn!("Error reading module file {}: {}", loc.path.display(), e),
            Ok(data) => {
                /* hash the data and add the module entry */
                output.push(ModuleFile {
                    path: loc.path,
                    code: loc.code,
                    modtype: loc.modtype,
                    hash: Some(xx::hash64(data)),
                });
            },
        }
    }

    debug!("Joining walker thread..");
    walker.join().expect("failed to join FS walker");
    debug!("Joined");

    if num_module_files == 0 {
        warn!("No module files found in MODULEPATH \"{}\". Check your configuration!", modulepath);
    }

    output
}

fn crawl_gen(roots: Vec<String>, tx: Sender<Option<ModuleFile>>) {
    for root in roots {
        crawl_dir(&Path::new(&root), Path::new(""), &tx);
    }

    tx.send(None).expect("unexpected failure terminating walker stream");
}

fn crawl_dir(root: &Path, pfx: &Path, tx: &Sender<Option<ModuleFile>>) {
    let rel = root.join(pfx);

    if let Ok(entries) = fs::read_dir(&rel) {
        for entry in entries {
            if let Ok(entry) = entry {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        /* directory -- continue crawling */
                        crawl_dir(root, &pfx.join(entry.file_name()), tx);
                    } else {
                        /* file -- grab extension and send back through tx */

                        let mod_type = match entry.path().extension() {
                            Some(ext) => match ext.to_str() {
                                Some("lua") => ModuleType::LMOD,
                                _ => ModuleType::TCL,
                            },
                            None => ModuleType::TCL,
                        };

                        let mod_code = match mod_type {
                            ModuleType::LMOD => pfx.join(entry.path().file_stem().unwrap()),
                            ModuleType::TCL => pfx.join(entry.file_name()),
                        };

                        tx.send(Some(ModuleFile {
                            path: entry.path(),
                            code: mod_code.to_str().unwrap().to_string(),
                            modtype: mod_type,
                            hash: None,
                        })).expect("unexpected mpsc send fail");
                    }
                }
            }
        }
    }
}
