use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::git::commands::{run_git, run_git_expect_stdout};
use crate::types::FileStatus;

#[derive(Debug, Clone)]
pub struct DiffPair {
    pub label: String,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone)]
pub struct NameStatusEntry {
    pub status: FileStatus,
    pub old_path: Option<String>,
    pub new_path: String,
}

#[derive(Debug, Clone)]
pub struct GeneratedDiffArtifacts {
    pub patch_filename: String,
    pub changes_filename: String,
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
    pub name_status: BTreeMap<String, NameStatusEntry>,
}

pub fn commit_exists(repo: &Path, timeout_secs: u64, spec: &str) -> bool {
    run_git(
        repo,
        timeout_secs,
        &["rev-parse", "--verify", "--quiet", spec],
    )
    .map(|out| out.ok())
    .unwrap_or(false)
}

pub fn build_history_pairs(
    head_hash: &str,
    history_depth: u32,
    include_current_pair: bool,
) -> Vec<DiffPair> {
    let mut pairs = Vec::new();

    if include_current_pair {
        pairs.push(DiffPair {
            label: "N_vs_N-1".to_string(),
            from: format!("{}~1", head_hash),
            to: head_hash.to_string(),
        });
    }

    if history_depth >= 2 {
        pairs.push(DiffPair {
            label: "N-1_vs_N-2".to_string(),
            from: format!("{}~2", head_hash),
            to: format!("{}~1", head_hash),
        });
    }

    if history_depth > 2 {
        for idx in 2..history_depth {
            let from = format!("{}~{}", head_hash, idx + 1);
            let to = format!("{}~{}", head_hash, idx);
            pairs.push(DiffPair {
                label: format!("N-{}_vs_N-{}", idx, idx + 1),
                from,
                to,
            });
        }
    }

    pairs
}

pub fn generate_diff_artifacts(
    repo: &Path,
    diff_dir: &Path,
    timeout_secs: u64,
    pair: &DiffPair,
) -> Result<GeneratedDiffArtifacts> {
    fs::create_dir_all(diff_dir)?;

    let patch_filename = format!("diff_{}.patch", pair.label);
    let changes_filename = format!("changes_{}.txt", pair.label);
    let patch_path = diff_dir.join(&patch_filename);
    let changes_path = diff_dir.join(&changes_filename);

    let range = format!("{}..{}", pair.from, pair.to);

    let patch_output = run_git(repo, timeout_secs, &["diff", &range])?;
    fs::write(&patch_path, patch_output.stdout.as_bytes())?;

    let name_status_output = run_git(
        repo,
        timeout_secs,
        &["diff", "--name-status", &range],
    )?;

    let numstat_output =
        run_git_expect_stdout(repo, timeout_secs, &["diff", "--numstat", &range])
            .unwrap_or_default();

    let mut changes_content = String::new();
    if !numstat_output.is_empty() {
        changes_content.push_str("# numstat\n");
        changes_content.push_str(&numstat_output);
        changes_content.push('\n');
    }
    if !name_status_output.stdout.is_empty() {
        changes_content.push_str("\n# name-status\n");
        changes_content.push_str(&name_status_output.stdout);
        changes_content.push('\n');
    }

    fs::write(changes_path, changes_content.as_bytes())?;

    let (files_changed, insertions, deletions) = parse_numstat(&numstat_output);
    let name_status = parse_name_status(&name_status_output.stdout);

    Ok(GeneratedDiffArtifacts {
        patch_filename,
        changes_filename,
        files_changed,
        insertions,
        deletions,
        name_status,
    })
}

fn parse_numstat(text: &str) -> (u32, u32, u32) {
    let mut files_changed = 0_u32;
    let mut insertions = 0_u32;
    let mut deletions = 0_u32;

    for line in text.lines() {
        let mut parts = line.split('\t');
        let added = parts.next().unwrap_or_default();
        let removed = parts.next().unwrap_or_default();
        let path = parts.next().unwrap_or_default();
        if path.is_empty() {
            continue;
        }
        files_changed += 1;

        if let Ok(v) = added.parse::<u32>() {
            insertions += v;
        }
        if let Ok(v) = removed.parse::<u32>() {
            deletions += v;
        }
    }

    (files_changed, insertions, deletions)
}

fn parse_name_status(text: &str) -> BTreeMap<String, NameStatusEntry> {
    let mut map = BTreeMap::new();

    for line in text.lines() {
        let mut parts = line.split('\t');
        let status_token = parts.next().unwrap_or_default().trim();
        if status_token.is_empty() {
            continue;
        }

        let (status, old_path, new_path) =
            if status_token.starts_with('R') || status_token.starts_with('C') {
                let from = parts.next().unwrap_or_default().to_string();
                let to = parts.next().unwrap_or_default().to_string();
                let status = if status_token.starts_with('R') {
                    FileStatus::Renamed
                } else {
                    FileStatus::Copied
                };
                (status, Some(from), to)
            } else {
                let path = parts.next().unwrap_or_default().to_string();
                let status = match status_token.chars().next().unwrap_or('M') {
                    'A' => FileStatus::Added,
                    'D' => FileStatus::Deleted,
                    'M' => FileStatus::Modified,
                    _ => FileStatus::Unknown,
                };
                (status, None, path)
            };

        if !new_path.is_empty() {
            map.insert(
                new_path.clone(),
                NameStatusEntry {
                    status,
                    old_path,
                    new_path,
                },
            );
        }
    }

    map
}

pub fn safe_diff_pair(repo: &Path, timeout_secs: u64, pair: &DiffPair) -> bool {
    commit_exists(repo, timeout_secs, &pair.from) && commit_exists(repo, timeout_secs, &pair.to)
}

pub fn path_in_repo(base: &Path, child: &str) -> PathBuf {
    base.join(child)
}
