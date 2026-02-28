use std::path::Path;

use chrono::{TimeZone, Utc};

use crate::error::Result;
use crate::git::commands::run_git_expect_stdout;
use crate::types::CommitInfo;

#[derive(Debug, Clone)]
pub struct RepoState {
    pub commit: CommitInfo,
    pub branch: String,
    pub dirty: bool,
}

pub fn capture_repo_state(repo: &Path, timeout_secs: u64, detect_dirty: bool) -> Result<RepoState> {
    let log_output = run_git_expect_stdout(
        repo,
        timeout_secs,
        &["log", "-1", "--format=%H%n%h%n%an <%ae>%n%ct%n%B"],
    )?;

    let mut lines = log_output.lines();
    let hash = lines.next().unwrap_or_default().to_string();
    let short_hash = lines.next().unwrap_or_default().to_string();
    let author = lines.next().unwrap_or_default().to_string();
    let ts = lines.next().unwrap_or_default();
    let ts_i = ts.parse::<i64>().unwrap_or(0);
    let full_message: String = lines.collect::<Vec<_>>().join("\n").trim().to_string();
    let message = full_message
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();

    let branch = run_git_expect_stdout(repo, timeout_secs, &["rev-parse", "--abbrev-ref", "HEAD"])?;

    let dirty = if detect_dirty {
        !run_git_expect_stdout(repo, timeout_secs, &["status", "--porcelain"])?
            .trim()
            .is_empty()
    } else {
        false
    };

    let commit = CommitInfo {
        hash,
        short_hash,
        message,
        full_message,
        author,
        timestamp: Utc
            .timestamp_opt(ts_i, 0)
            .single()
            .unwrap_or_else(Utc::now)
            .to_rfc3339(),
    };

    Ok(RepoState {
        commit,
        branch,
        dirty,
    })
}

pub fn capture_commit(repo: &Path, timeout_secs: u64, spec: &str) -> Result<CommitInfo> {
    let log_output = run_git_expect_stdout(
        repo,
        timeout_secs,
        &["log", "-1", "--format=%H%n%h%n%an <%ae>%n%ct%n%B", spec],
    )?;

    let mut lines = log_output.lines();
    let hash = lines.next().unwrap_or_default().to_string();
    let short_hash = lines.next().unwrap_or_default().to_string();
    let author = lines.next().unwrap_or_default().to_string();
    let ts = lines.next().unwrap_or_default();
    let ts_i = ts.parse::<i64>().unwrap_or(0);
    let full_message: String = lines.collect::<Vec<_>>().join("\n").trim().to_string();
    let message = full_message
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();

    Ok(CommitInfo {
        hash,
        short_hash,
        message,
        full_message,
        author,
        timestamp: Utc
            .timestamp_opt(ts_i, 0)
            .single()
            .unwrap_or_else(Utc::now)
            .to_rfc3339(),
    })
}
