/*
 * analysis.rs
 *
 * structures for analyzing modulefiles
 */

use crate::crawl;

use regex::{Regex, RegexBuilder};
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt; /* for modelines */
use std::path::{Path, PathBuf};

const lmod_path_reg_src: &'static str = r#"^\s*prepend_path\s*\(\s*"PATH"\s*,\s*"([^"]+)"\s*(?:,\s*":"\s*)?\)\s*$"#;
//const lmod_path_reg_src: &'static str = r#"^\s*(prepend_path)\s*\(.*$"#;

pub struct Info {
    pub file: crawl::ModuleFile,
    pub bins: Vec<String>,
}

pub fn analyze(file: crawl::ModuleFile) -> Result<Info, io::Error> {
    let contents = fs::read_to_string(&file.path)?;
    let bins = analyze_bins(&contents, &file.modtype);

    Ok(Info {
        file: file,
        bins: bins,
    })
}

fn analyze_bins(contents: &String, modtype: &crawl::ModuleType) -> Vec<String> {
    let paths = match modtype {
        crawl::ModuleType::LMOD => extract_lmod_paths(contents),
        _ => Vec::new(),
    };

    paths.into_iter().map(|p| search_path(p)).flatten().collect()
}

fn extract_lmod_paths(contents: &String) -> Vec<String> {
    lazy_static! {
        static ref lmod_path_reg: Regex = RegexBuilder::new(lmod_path_reg_src).multi_line(true).build().unwrap();
    }

    lmod_path_reg.captures_iter(contents).map(|x| x[1].to_string()).collect()
}

fn search_path(path: String) -> Vec<String> {
    let mut output: Vec<String> = Vec::new();

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries {
            if let Ok(entry) = entry {
                if let Ok(file_type) = entry.file_type() {
                    if is_executable::is_executable(entry.path()) {
                        output.push(entry.path().file_name().unwrap().to_string_lossy().to_string());
                    }
                }
            }
        } 
    }

    output
}
