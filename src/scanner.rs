use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use walkdir::{DirEntry, WalkDir};

use crate::error::Result;

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub nested: bool,
    pub follow_symlinks: bool,
    pub skip_hidden: bool,
    pub include_bare: bool,
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| name.starts_with('.'))
}

fn is_repo_dir(path: &Path) -> bool {
    path.join(".git").is_dir()
}

fn is_bare_repo(path: &Path) -> bool {
    path.join("HEAD").is_file() && path.join("objects").is_dir() && path.join("refs").is_dir()
}

pub fn discover_repositories(root: &Path, options: &ScanOptions) -> Result<Vec<PathBuf>> {
    let mut repos = BTreeSet::new();
    let mut walker = WalkDir::new(root)
        .follow_links(options.follow_symlinks)
        .into_iter();

    while let Some(next) = walker.next() {
        let entry = match next {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        if options.skip_hidden && entry.depth() > 0 && is_hidden(&entry) {
            if entry.file_type().is_dir() {
                walker.skip_current_dir();
            }
            continue;
        }

        if !entry.file_type().is_dir() {
            continue;
        }

        let dir = entry.path();
        if dir.file_name().is_some_and(|name| name == ".git") {
            walker.skip_current_dir();
            continue;
        }

        let is_repo = is_repo_dir(dir) || (options.include_bare && is_bare_repo(dir));
        if is_repo {
            repos.insert(dir.to_path_buf());
            if !options.nested {
                walker.skip_current_dir();
            }
        }
    }

    Ok(repos.into_iter().collect())
}
