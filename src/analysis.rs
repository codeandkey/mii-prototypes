/*
 * analysis.rs
 *
 * structures for analyzing modulefiles
 */

use crate::crawl;

use regex::{Regex, RegexBuilder};
use std::fs;
use std::io;

const LMOD_PATH_REG_SRC: &'static str =
    r#"^\s*prepend_path\s*\(\s*"PATH"\s*,\s*"([^"]+)"\s*(?:,\s*":"\s*)?\)\s*$"#;

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

    paths
        .into_iter()
        .map(|p| search_path(p))
        .flatten()
        .collect()
}

fn extract_lmod_paths(contents: &String) -> Vec<String> {
    lazy_static! {
        static ref LMOD_PATH_REG: Regex = RegexBuilder::new(LMOD_PATH_REG_SRC)
            .multi_line(true)
            .build()
            .unwrap();
    }

    LMOD_PATH_REG
        .captures_iter(contents)
        .map(|x| x[1].to_string())
        .collect()
}

fn search_path(path: String) -> Vec<String> {
    let mut output: Vec<String> = Vec::new();

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries {
            if let Ok(entry) = entry {
                if is_executable::is_executable(entry.path()) {
                    output.push(
                        entry
                            .path()
                            .file_name()
                            .unwrap()
                            .to_string_lossy()
                            .to_string(),
                    );
                }
            }
        }
    }

    output
}
