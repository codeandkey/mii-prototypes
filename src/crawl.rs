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
use walkdir::WalkDir;

#[derive(Clone)]
pub enum ModuleType {
    LMOD,
    TCL,
}

#[derive(Clone)]
pub struct ModuleFile {
    pub path: PathBuf,
    pub code: String,
    pub modtype: ModuleType,
    pub hash: Option<u32>, /* not filled at first. */
}

pub fn crawl_sync(modulepath: String) -> Vec<ModuleFile> {
    let mut output = Vec::new();

    let roots: Vec<String> = modulepath.split(':').map(|x| x.to_string()).collect();

    let (tx, rx): (Sender<Option<ModuleFile>>, Receiver<Option<ModuleFile>>) = channel();

    let walker = thread::spawn(move || {
        crawl_gen(roots, tx);
    });

    /* wait for file locations to come back through the channel */

    let mut num_module_files = 0;

    while let Some(loc) = rx.recv().unwrap() {
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
                    hash: Some(xx::hash32(data)),
                });
            },
        }
    }

    walker.join().expect("failed to join FS walker");

    if num_module_files == 0 {
        warn!("No module files found in MODULEPATH \"{}\". Check your configuration!", modulepath);
    }

    output
}

fn crawl_gen(roots: Vec<String>, tx: Sender<Option<ModuleFile>>) {
    for root in roots {
        crawl_dir(&Path::new(&root), &tx);
    }

    tx.send(None).expect("unexpected failure terminating walker stream");
}

fn crawl_dir(root: &Path, tx: &Sender<Option<ModuleFile>>) {
    let walker = WalkDir::new(root).into_iter();

    for entry in walker.filter_entry(|e| {
        e.file_name().to_str().map(|s| !s.starts_with(".")).unwrap_or(false)
    }) {
        if let Ok(entry) = entry {
            if entry.file_type().is_file() {
                let path = entry.path();

                let (mod_type, mod_code) = match path.extension() {
                    Some(ext) => match ext.to_str() {
                        Some("lua") => (ModuleType::LMOD, path.file_stem().unwrap()),
                        _ => (ModuleType::TCL, entry.file_name()),
                    },
                    None => (ModuleType::TCL, entry.file_name()), /* noice */
                };

                tx.send(Some(ModuleFile {
                    path: path.to_path_buf(),
                    code: mod_code.to_string_lossy().to_string(),
                    modtype: mod_type,
                    hash: None,
                })).expect("unexpected mpsc send fail");
            }
        }
    }
}
