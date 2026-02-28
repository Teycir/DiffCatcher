use std::collections::{HashMap, VecDeque};
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use chrono::Utc;
use glob::Pattern;

use crate::cli::PullStrategy;
use crate::extraction::{ExtractionOptions, extract_from_patch};
use crate::git::commands::run_git;
use crate::git::diff::{
    DiffPair, NameStatusEntry, build_history_pairs, generate_diff_artifacts, safe_diff_pair,
};
use crate::git::file_retrieval::show_file;
use crate::git::state::{capture_commit, capture_repo_state};
use crate::report::writer::repo_folder_name;
use crate::security::tagger::tag_file_changes;
use crate::types::{
    CommitInfo, DiffResult, FileChangeDetail, Language, RepoResult, RepoStatus,
    SecurityTagDefinition,
};

const MAX_PATCH_BYTES: usize = 50 * 1024 * 1024;
const SHOW_FILE_CACHE_CAPACITY: usize = 2048;

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

    let pre_state = match capture_repo_state(repo_path, cfg.timeout_secs, cfg.pull_mode) {
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
    if let Some(pattern) = branch_pattern
        && !pattern.matches(&branch)
    {
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
                if pre_state.dirty
                    && cfg.force_pull
                    && let Ok(stash_out) = run_git(
                        repo_path,
                        cfg.timeout_secs,
                        &["stash", "push", "-m", "git-patrol auto-stash"],
                    )
                {
                    pull_log.push_str(&stash_out.stdout);
                    if !stash_out.stderr.is_empty() {
                        pull_log.push('\n');
                        pull_log.push_str(&stash_out.stderr);
                    }
                    stashed = stash_out.ok();
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
                            if let Ok(new_state) = capture_repo_state(repo_path, cfg.timeout_secs, false) {
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

                if stashed
                    && let Ok(pop_out) = run_git(repo_path, cfg.timeout_secs, &["stash", "pop"])
                {
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
        let mut retrieval_cache = ShowFileCache::new(SHOW_FILE_CACHE_CAPACITY);

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
                    let patch_bytes = fs::read(&patch_path).unwrap_or_default();

                    let (file_changes, element_summary, security_review) = if patch_bytes.len()
                        > MAX_PATCH_BYTES
                    {
                        errors.push(format!(
                            "diff {} exceeds {} bytes; extraction skipped",
                            pair.label, MAX_PATCH_BYTES
                        ));
                        (file_level_fallback(&artifacts.name_status), None, None)
                    } else {
                        let patch_text = String::from_utf8_lossy(&patch_bytes).to_string();
                        match catch_unwind(AssertUnwindSafe(|| {
                            extract_from_patch(
                                &patch_text,
                                &artifacts.name_status,
                                &from_commit.hash,
                                &to_commit.hash,
                                &cfg.extraction,
                            )
                        })) {
                            Ok((mut file_changes, element_summary)) => {
                                apply_git_show_diffonly_fallback(
                                    repo_path,
                                    cfg.timeout_secs,
                                    &from_commit.hash,
                                    &to_commit.hash,
                                    &mut file_changes,
                                    &mut retrieval_cache,
                                    &mut errors,
                                );

                                let security_review = if cfg.no_security_tags {
                                    None
                                } else {
                                    match tag_file_changes(
                                        &mut file_changes,
                                        &cfg.tag_definitions,
                                        cfg.include_test_security,
                                    ) {
                                        Ok(review) => Some(review),
                                        Err(err) => {
                                            errors.push(format!(
                                                "security tagging failed for {}: {}",
                                                pair.label, err
                                            ));
                                            None
                                        }
                                    }
                                };
                                (file_changes, element_summary, security_review)
                            }
                            Err(_) => {
                                errors.push(format!(
                                        "element extraction panicked for {}; falling back to file-level report",
                                        pair.label
                                    ));
                                (file_level_fallback(&artifacts.name_status), None, None)
                            }
                        }
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

fn apply_git_show_diffonly_fallback(
    repo_path: &Path,
    timeout_secs: u64,
    from_commit: &str,
    to_commit: &str,
    file_changes: &mut [FileChangeDetail],
    cache: &mut ShowFileCache,
    errors: &mut Vec<String>,
) {
    for file in file_changes {
        if file.elements.is_empty() {
            continue;
        }

        let old_path = file.old_path.as_deref().unwrap_or(&file.path).to_string();
        let new_path = file.path.clone();

        let needs_old = !matches!(file.status, crate::types::FileStatus::Added);
        let needs_new = !matches!(file.status, crate::types::FileStatus::Deleted);

        let old_ok = if needs_old {
            fetch_file_content(
                repo_path,
                timeout_secs,
                from_commit,
                &old_path,
                cache,
                errors,
            )
            .is_some()
        } else {
            true
        };
        let new_ok = if needs_new {
            fetch_file_content(repo_path, timeout_secs, to_commit, &new_path, cache, errors)
                .is_some()
        } else {
            true
        };

        if old_ok && new_ok {
            continue;
        }

        for element in &mut file.elements {
            element.snippet.before = None;
            element.snippet.after = None;
            element.snippet.capture_scope = crate::types::CaptureScope::DiffOnly;
        }
    }
}

fn fetch_file_content(
    repo_path: &Path,
    timeout_secs: u64,
    commit: &str,
    path: &str,
    cache: &mut ShowFileCache,
    errors: &mut Vec<String>,
) -> Option<String> {
    let key = (commit.to_string(), path.to_string());
    if let Some(cached) = cache.get(&key) {
        return cached;
    }

    let content = match show_file(repo_path, commit, path, timeout_secs) {
        Ok(content) => content,
        Err(err) => {
            errors.push(format!(
                "git show failed for {}:{}; using DiffOnly fallback ({})",
                commit, path, err
            ));
            None
        }
    };
    cache.insert(key, content.clone());
    content
}

#[derive(Debug, Clone)]
struct ShowFileCache {
    capacity: usize,
    map: HashMap<(String, String), Option<String>>,
    order: VecDeque<(String, String)>,
}

impl ShowFileCache {
    fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            map: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    fn get(&mut self, key: &(String, String)) -> Option<Option<String>> {
        let value = self.map.get(key)?.clone();
        self.touch(key);
        Some(value)
    }

    fn insert(&mut self, key: (String, String), value: Option<String>) {
        let exists = self.map.contains_key(&key);
        self.map.insert(key.clone(), value);
        if exists {
            self.touch(&key);
            return;
        }

        self.order.push_back(key.clone());
        while self.map.len() > self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.map.remove(&oldest);
            } else {
                break;
            }
        }
    }

    fn touch(&mut self, key: &(String, String)) {
        if let Some(pos) = self.order.iter().position(|entry| entry == key) {
            self.order.remove(pos);
        }
        self.order.push_back(key.clone());
    }
}

fn file_level_fallback(
    name_status: &std::collections::BTreeMap<String, NameStatusEntry>,
) -> Vec<FileChangeDetail> {
    let mut files = name_status
        .iter()
        .map(|(path, status)| FileChangeDetail {
            path: path.clone(),
            old_path: status.old_path.clone(),
            status: status.status,
            language: Language::Unknown("fallback".to_string()),
            insertions: 0,
            deletions: 0,
            elements: Vec::new(),
            raw_hunks: Vec::new(),
            is_binary: false,
        })
        .collect::<Vec<_>>();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}
