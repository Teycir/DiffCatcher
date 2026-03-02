use std::path::{Path, PathBuf};
use std::process::Command;

use diffcatcher::cli::PullStrategy;
use diffcatcher::extraction::ExtractionOptions;
use diffcatcher::processor::{ProcessorConfig, process_repository};
use diffcatcher::security::patterns::builtin_patterns;
use diffcatcher::types::RepoStatus;
use tempfile::tempdir;

#[test]
fn fetch_mode_transitions_from_up_to_date_to_updated() {
    let tmp = tempdir().expect("temp dir");
    let root = tmp.path();

    let remote = root.join("remote.git");
    git_raw(&["init", "--bare", remote.to_string_lossy().as_ref()]);

    let seed = root.join("seed");
    std::fs::create_dir_all(&seed).expect("create seed");
    git(&seed, &["init"]);
    git(&seed, &["config", "user.name", "Test"]);
    git(&seed, &["config", "user.email", "test@example.com"]);
    std::fs::write(seed.join("lib.rs"), "pub fn v() -> i32 { 1 }\n").expect("write v1");
    git(&seed, &["add", "."]);
    git(&seed, &["commit", "-m", "v1"]);
    git(
        &seed,
        &["remote", "add", "origin", remote.to_string_lossy().as_ref()],
    );
    git(&seed, &["push", "-u", "origin", "HEAD"]);

    let local = root.join("repo");
    git_raw(&[
        "clone",
        remote.to_string_lossy().as_ref(),
        local.to_string_lossy().as_ref(),
    ]);

    let cfg = processor_config(root, root.join("report"), false, false);

    let first = process_repository(&local, &cfg, None);
    assert!(matches!(first.status, RepoStatus::UpToDate));
    let first_pre = first.pre_pull.as_ref().expect("pre pull hash").hash.clone();
    let first_post = first
        .post_pull
        .as_ref()
        .expect("post pull hash")
        .hash
        .clone();
    assert_eq!(first_pre, first_post);

    std::fs::write(seed.join("lib.rs"), "pub fn v() -> i32 { 2 }\n").expect("write v2");
    git(&seed, &["add", "."]);
    git(&seed, &["commit", "-m", "v2"]);
    git(&seed, &["push", "origin", "HEAD"]);

    let second = process_repository(&local, &cfg, None);
    assert!(matches!(second.status, RepoStatus::Updated));
    let second_pre = second
        .pre_pull
        .as_ref()
        .expect("pre pull hash")
        .hash
        .clone();
    let second_post = second
        .post_pull
        .as_ref()
        .expect("post pull hash")
        .hash
        .clone();
    assert_ne!(second_pre, second_post);
}

#[test]
fn pull_mode_skips_pull_when_up_to_date() {
    let tmp = tempdir().expect("temp dir");
    let root = tmp.path();

    let remote = root.join("remote.git");
    git_raw(&["init", "--bare", remote.to_string_lossy().as_ref()]);

    let seed = root.join("seed");
    std::fs::create_dir_all(&seed).expect("create seed");
    git(&seed, &["init"]);
    git(&seed, &["config", "user.name", "Test"]);
    git(&seed, &["config", "user.email", "test@example.com"]);
    std::fs::write(seed.join("lib.rs"), "pub fn v() -> i32 { 1 }\n").expect("write v1");
    git(&seed, &["add", "."]);
    git(&seed, &["commit", "-m", "v1"]);
    git(
        &seed,
        &["remote", "add", "origin", remote.to_string_lossy().as_ref()],
    );
    git(&seed, &["push", "-u", "origin", "HEAD"]);

    let local = root.join("repo");
    git_raw(&[
        "clone",
        remote.to_string_lossy().as_ref(),
        local.to_string_lossy().as_ref(),
    ]);

    let cfg = processor_config(root, root.join("report-pull"), false, true);
    let result = process_repository(&local, &cfg, None);

    assert!(matches!(result.status, RepoStatus::UpToDate));
    let pre = result
        .pre_pull
        .as_ref()
        .expect("pre pull hash should exist")
        .hash
        .clone();
    let post = result
        .post_pull
        .as_ref()
        .expect("post pull hash should exist")
        .hash
        .clone();
    assert_eq!(pre, post);
    assert!(
        result.pull_log.contains("pull skipped"),
        "expected pull skip log, got: {}",
        result.pull_log
    );
}

fn processor_config(
    root: &Path,
    report_dir: PathBuf,
    no_pull: bool,
    pull_mode: bool,
) -> ProcessorConfig {
    ProcessorConfig {
        root_dir: root.to_path_buf(),
        report_dir,
        timeout_secs: 30,
        pull_mode,
        force_pull: false,
        pull_strategy: PullStrategy::FfOnly,
        no_pull,
        dry_run: false,
        history_depth: 2,
        branch_filter: "*".to_string(),
        extraction: ExtractionOptions {
            no_summary_extraction: false,
            no_snippets: true,
            snippet_context: 2,
            max_snippet_lines: 20,
            max_elements: 50,
            include_vendor: false,
            plugin_extractors: Vec::new(),
        },
        no_security_tags: true,
        include_detached: true,
        include_test_security: false,
        tag_definitions: builtin_patterns(),
        verbose: false,
    }
}

fn git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .status()
        .expect("run git command");
    assert!(status.success(), "git command failed: {:?}", args);
}

fn git_raw(args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .status()
        .expect("run git command");
    assert!(status.success(), "git command failed: {:?}", args);
}
