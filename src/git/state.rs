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
    let hash = run_git_expect_stdout(repo, timeout_secs, &["rev-parse", "HEAD"])?;
    let short_hash = run_git_expect_stdout(repo, timeout_secs, &["rev-parse", "--short", "HEAD"])?;
    let full_message = run_git_expect_stdout(repo, timeout_secs, &["log", "-1", "--pretty=%B"])?;
    let message = full_message
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    let author = run_git_expect_stdout(repo, timeout_secs, &["log", "-1", "--pretty=%an <%ae>"])?;
    let ts = run_git_expect_stdout(repo, timeout_secs, &["log", "-1", "--pretty=%ct"])?;
    let ts_i = ts.parse::<i64>().unwrap_or(0);
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
    let hash = run_git_expect_stdout(repo, timeout_secs, &["rev-parse", spec])?;
    let short_hash = run_git_expect_stdout(repo, timeout_secs, &["rev-parse", "--short", spec])?;

    let pretty_ref = spec.to_string();
    let full_message = run_git_expect_stdout(
        repo,
        timeout_secs,
        &["log", "-1", "--pretty=%B", &pretty_ref],
    )?;
    let message = full_message
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    let author = run_git_expect_stdout(
        repo,
        timeout_secs,
        &["log", "-1", "--pretty=%an <%ae>", &pretty_ref],
    )?;
    let ts = run_git_expect_stdout(
        repo,
        timeout_secs,
        &["log", "-1", "--pretty=%ct", &pretty_ref],
    )?;
    let ts_i = ts.parse::<i64>().unwrap_or(0);

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
