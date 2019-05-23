/*
 * analysis.rs
 *
 * structures for analyzing modulefiles
 */

use crate::crawl;

pub struct Result {
    pub file: crawl::ModuleFile,
    pub bins: Vec<String>,
}
