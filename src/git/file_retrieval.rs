use std::path::Path;

use crate::error::Result;
use crate::git::commands::{run_git, run_git_expect_stdout};

pub fn show_file(
    repo: &Path,
    commit: &str,
    file_path: &str,
    timeout_secs: u64,
) -> Result<Option<String>> {
    let spec = format!("{}:{}", commit, file_path);
    let out = run_git(repo, timeout_secs, &["show", &spec])?;
    if out.ok() {
        return Ok(Some(out.stdout));
    }

    // Missing files are expected in add/delete/rename edge cases.
    if out.stderr.contains("exists on disk")
        || out.stderr.contains("does not exist")
        || out.stderr.contains("Path '")
    {
        return Ok(None);
    }

    let fallback = run_git_expect_stdout(repo, timeout_secs, &["cat-file", "-e", &spec]);
    match fallback {
        Ok(_) => Ok(Some(out.stdout)),
        Err(_) => Ok(None),
    }
}
