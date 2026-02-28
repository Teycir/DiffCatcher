use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use glob::Pattern;

use crate::cli::PullStrategy;
use crate::extraction::{ExtractionOptions, extract_from_patch};
use crate::git::commands::run_git;
use crate::git::diff::{DiffPair, build_history_pairs, generate_diff_artifacts, safe_diff_pair};
use crate::git::state::{capture_commit, capture_repo_state};
use crate::report::writer::repo_folder_name;
use crate::security::tagger::tag_file_changes;
use crate::types::{CommitInfo, DiffResult, RepoResult, RepoStatus, SecurityTagDefinition};

#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    pub root_dir: PathBuf,
    pub report_dir: PathBuf,
    pub timeout_secs: u64,
    pub pull_mode: bool,
    pub force_pull: bool,
    pub pull_strategy: PullStrategy,
    pub no_pull: bool,
    pub dry_run: bool,
    pub history_depth: u32,
    pub branch_filter: String,
    pub extraction: ExtractionOptions,
    pub no_security_tags: bool,
    pub include_detached: bool,
    pub include_test_security: bool,
    pub tag_definitions: Vec<SecurityTagDefinition>,
    pub verbose: bool,
}

pub fn process_repository(repo_path: &Path, cfg: &ProcessorConfig) -> RepoResult {
    let repo_name = repo_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repo")
        .to_string();

    let report_folder_name = repo_folder_name(&cfg.root_dir, repo_path);

    let mut pull_log = String::new();
    let mut errors = Vec::new();

    let pre_state = match capture_repo_state(repo_path, cfg.timeout_secs) {
        Ok(state) => state,
        Err(err) => {
            return RepoResult {
                repo_path: repo_path.to_path_buf(),
                repo_name,
                report_folder_name,
                branch: "unknown".to_string(),
                status: RepoStatus::FetchFailed {
                    error: err.to_string(),
                },
                pre_pull: None,
                post_pull: None,
                diffs: Vec::new(),
                pull_log,
                errors: vec![err.to_string()],
                timestamp: Utc::now(),
            };
        }
    };

    let branch = pre_state.branch.clone();

    if branch == "HEAD" && !cfg.include_detached {
        return RepoResult {
            repo_path: repo_path.to_path_buf(),
            repo_name,
            report_folder_name,
            branch,
            status: RepoStatus::Skipped {
                reason: "detached HEAD".to_string(),
            },
            pre_pull: Some(pre_state.commit),
            post_pull: None,
            diffs: Vec::new(),
            pull_log,
            errors,
            timestamp: Utc::now(),
        };
    }

    let branch_pattern = Pattern::new(&cfg.branch_filter).ok();
    if let Some(pattern) = branch_pattern {
        if !pattern.matches(&branch) {
            return RepoResult {
                repo_path: repo_path.to_path_buf(),
                repo_name,
                report_folder_name,
                branch,
                status: RepoStatus::Skipped {
                    reason: format!("branch '{}' does not match filter", pre_state.branch),
                },
                pre_pull: Some(pre_state.commit),
                post_pull: None,
                diffs: Vec::new(),
                pull_log,
                errors,
                timestamp: Utc::now(),
            };
        }
    }

    let pre_commit = pre_state.commit.clone();
    let mut post_commit = pre_commit.clone();
    let mut status = RepoStatus::UpToDate;

    if !cfg.dry_run && !cfg.no_pull {
        if cfg.pull_mode {
            if pre_state.dirty && !cfg.force_pull {
                status = RepoStatus::DirtySkipped;
                pull_log.push_str("dirty repo, pull skipped\n");
            } else {
                let mut stashed = false;
                if pre_state.dirty && cfg.force_pull {
                    if let Ok(stash_out) = run_git(
                        repo_path,
                        cfg.timeout_secs,
                        &["stash", "push", "-m", "git-patrol auto-stash"],
                    ) {
                        pull_log.push_str(&stash_out.stdout);
                        if !stash_out.stderr.is_empty() {
                            pull_log.push('\n');
                            pull_log.push_str(&stash_out.stderr);
                        }
                        stashed = stash_out.ok();
                    }
                }

                let pull_args = ["pull", cfg.pull_strategy.as_git_flag()];
                match run_git(repo_path, cfg.timeout_secs, &pull_args) {
                    Ok(out) => {
                        if !out.stdout.is_empty() {
                            pull_log.push_str(&out.stdout);
                            pull_log.push('\n');
                        }
                        if !out.stderr.is_empty() {
                            pull_log.push_str(&out.stderr);
                            pull_log.push('\n');
                        }

                        if out.ok() {
                            if let Ok(new_state) = capture_repo_state(repo_path, cfg.timeout_secs) {
                                post_commit = new_state.commit;
                                status = if pre_commit.hash == post_commit.hash {
                                    RepoStatus::UpToDate
                                } else {
                                    RepoStatus::Updated
                                };
                            }
                        } else {
                            let msg = if out.stderr.is_empty() {
                                out.stdout
                            } else {
                                out.stderr
                            };
                            status = RepoStatus::PullFailed { error: msg.clone() };
                            errors.push(msg);
                        }
                    }
                    Err(err) => {
                        status = RepoStatus::PullFailed {
                            error: err.to_string(),
                        };
                        errors.push(err.to_string());
                    }
                }

                if stashed {
                    if let Ok(pop_out) = run_git(repo_path, cfg.timeout_secs, &["stash", "pop"]) {
                        if !pop_out.stdout.is_empty() {
                            pull_log.push_str(&pop_out.stdout);
                            pull_log.push('\n');
                        }
                        if !pop_out.stderr.is_empty() {
                            pull_log.push_str(&pop_out.stderr);
                            pull_log.push('\n');
                        }
                    }
                }
            }
        } else {
            match run_git(repo_path, cfg.timeout_secs, &["fetch", "origin"]) {
                Ok(out) => {
                    if !out.stdout.is_empty() {
                        pull_log.push_str(&out.stdout);
                        pull_log.push('\n');
                    }
                    if !out.stderr.is_empty() {
                        pull_log.push_str(&out.stderr);
                        pull_log.push('\n');
                    }
                    if out.ok() {
                        let remote_ref = format!("origin/{}", branch);
                        match capture_commit(repo_path, cfg.timeout_secs, &remote_ref) {
                            Ok(commit) => {
                                post_commit = commit;
                                status = if pre_commit.hash == post_commit.hash {
                                    RepoStatus::UpToDate
                                } else {
                                    RepoStatus::Updated
                                };
                            }
                            Err(err) => {
                                errors.push(err.to_string());
                                status = RepoStatus::UpToDate;
                            }
                        }
                    } else {
                        let msg = if out.stderr.is_empty() {
                            out.stdout
                        } else {
                            out.stderr
                        };
                        status = RepoStatus::FetchFailed { error: msg.clone() };
                        errors.push(msg);
                    }
                }
                Err(err) => {
                    status = RepoStatus::FetchFailed {
                        error: err.to_string(),
                    };
                    errors.push(err.to_string());
                }
            }
        }
    }

    let mut diffs = Vec::new();
    if matches!(status, RepoStatus::Updated | RepoStatus::UpToDate) {
        let repo_report_dir = cfg.report_dir.join(&report_folder_name);
        let diff_dir = repo_report_dir.join("diffs");
        let _ = fs::create_dir_all(&diff_dir);

        let diff_pairs = build_pairs(repo_path, cfg, &pre_commit, &post_commit, &status);

        for pair in diff_pairs {
            if !safe_diff_pair(repo_path, cfg.timeout_secs, &pair) {
                if cfg.verbose {
                    errors.push(format!(
                        "skipping diff {} due to missing commits {}..{}",
                        pair.label, pair.from, pair.to
                    ));
                }
                continue;
            }

            match generate_diff_artifacts(repo_path, &diff_dir, cfg.timeout_secs, &pair) {
                Ok(artifacts) => {
                    let from_commit = capture_commit(repo_path, cfg.timeout_secs, &pair.from)
                        .unwrap_or_else(|_| fallback_commit(&pair.from));
                    let to_commit = capture_commit(repo_path, cfg.timeout_secs, &pair.to)
                        .unwrap_or_else(|_| fallback_commit(&pair.to));

                    let patch_path = diff_dir.join(&artifacts.patch_filename);
                    let patch_text = fs::read_to_string(&patch_path).unwrap_or_default();

                    let (mut file_changes, element_summary) = extract_from_patch(
                        &patch_text,
                        &artifacts.name_status,
                        &from_commit.hash,
                        &to_commit.hash,
                        &cfg.extraction,
                    );

                    let security_review = if cfg.no_security_tags {
                        None
                    } else {
                        tag_file_changes(
                            &mut file_changes,
                            &cfg.tag_definitions,
                            cfg.include_test_security,
                        )
                        .ok()
                    };

                    diffs.push(DiffResult {
                        label: pair.label,
                        from_commit,
                        to_commit,
                        files_changed: artifacts.files_changed,
                        insertions: artifacts.insertions,
                        deletions: artifacts.deletions,
                        file_changes,
                        element_summary,
                        security_review,
                        patch_filename: format!("diffs/{}", artifacts.patch_filename),
                        changes_filename: format!("diffs/{}", artifacts.changes_filename),
                        summary_json_filename: None,
                        summary_txt_filename: None,
                        summary_md_filename: None,
                        snippets_dir: None,
                    });
                }
                Err(err) => errors.push(err.to_string()),
            }
        }
    }

    RepoResult {
        repo_path: repo_path.to_path_buf(),
        repo_name,
        report_folder_name,
        branch,
        status,
        pre_pull: Some(pre_commit),
        post_pull: Some(post_commit),
        diffs,
        pull_log,
        errors,
        timestamp: Utc::now(),
    }
}

fn build_pairs(
    repo_path: &Path,
    cfg: &ProcessorConfig,
    pre: &CommitInfo,
    post: &CommitInfo,
    status: &RepoStatus,
) -> Vec<DiffPair> {
    match status {
        RepoStatus::Updated => {
            if cfg.pull_mode {
                build_history_pairs(&post.hash, cfg.history_depth, true)
            } else {
                let mut pairs = vec![DiffPair {
                    label: "N_vs_N-1".to_string(),
                    from: pre.hash.clone(),
                    to: post.hash.clone(),
                }];

                if cfg.history_depth >= 2 {
                    let extra = build_history_pairs(&post.hash, cfg.history_depth, false)
                        .into_iter()
                        .filter(|pair| {
                            pair.from != pre.hash
                                && pair.to != post.hash
                                && safe_diff_pair(repo_path, cfg.timeout_secs, pair)
                        })
                        .collect::<Vec<_>>();
                    pairs.extend(extra);
                }

                pairs
            }
        }
        RepoStatus::UpToDate => {
            if cfg.history_depth < 2 {
                Vec::new()
            } else {
                build_history_pairs(&pre.hash, cfg.history_depth, false)
            }
        }
        _ => Vec::new(),
    }
}

fn fallback_commit(spec: &str) -> CommitInfo {
    CommitInfo {
        hash: spec.to_string(),
        short_hash: spec.chars().take(7).collect(),
        message: "unavailable".to_string(),
        full_message: "unavailable".to_string(),
        author: "unknown".to_string(),
        timestamp: Utc::now().to_rfc3339(),
    }
}
