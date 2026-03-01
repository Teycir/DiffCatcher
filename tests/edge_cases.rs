use std::path::Path;
use std::process::Command;

use diffcatcher::cli::PullStrategy;
use diffcatcher::extraction::ExtractionOptions;
use diffcatcher::processor::{ProcessorConfig, process_repository};
use diffcatcher::scanner::{ScanOptions, discover_repositories};
use diffcatcher::security::custom::load_custom_patterns;
use diffcatcher::security::patterns::builtin_patterns;
use diffcatcher::types::RepoStatus;
use tempfile::tempdir;

#[test]
fn detached_head_repo_is_skipped_when_not_included() {
    let tmp = tempdir().expect("temp dir");
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(&repo).expect("create repo");
    init_repo_with_commit(&repo);
    run_git(&repo, &["checkout", "--detach", "HEAD"]);

    let cfg = processor_config(tmp.path(), false);
    let result = process_repository(&repo, &cfg);

    assert!(matches!(result.status, RepoStatus::Skipped { .. }));
}

#[test]
fn single_commit_repo_handles_history_depth_without_crash() {
    let tmp = tempdir().expect("temp dir");
    let repo = tmp.path().join("single");
    std::fs::create_dir_all(&repo).expect("create repo");
    init_repo_with_commit(&repo);

    let cfg = processor_config(tmp.path(), true);
    let result = process_repository(&repo, &cfg);

    assert!(matches!(result.status, RepoStatus::UpToDate));
    assert!(result.diffs.is_empty());
}

#[test]
fn scanner_includes_bare_repo_only_when_enabled() {
    let tmp = tempdir().expect("temp dir");
    let root = tmp.path();
    let bare = root.join("bare.git");

    let status = Command::new("git")
        .arg("init")
        .arg("--bare")
        .arg(&bare)
        .status()
        .expect("init bare");
    assert!(status.success());

    let default_scan = discover_repositories(
        root,
        &ScanOptions {
            nested: true,
            follow_symlinks: false,
            skip_hidden: false,
            include_bare: false,
        },
    )
    .expect("default scan");
    assert!(default_scan.is_empty());

    let include_bare_scan = discover_repositories(
        root,
        &ScanOptions {
            nested: true,
            follow_symlinks: false,
            skip_hidden: false,
            include_bare: true,
        },
    )
    .expect("include bare scan");
    assert_eq!(include_bare_scan.len(), 1);
    assert_eq!(include_bare_scan[0], bare);
}

#[test]
fn invalid_custom_security_file_returns_error() {
    let tmp = tempdir().expect("temp dir");
    let invalid = tmp.path().join("invalid-security.json");
    std::fs::write(
        &invalid,
        r#"{
          "version": 1,
          "mode": "wrong",
          "tags": []
        }"#,
    )
    .expect("write invalid file");

    let err = load_custom_patterns(&invalid).expect_err("expected invalid mode error");
    let msg = err.to_string();
    assert!(msg.contains("invalid security tag mode"));
}

#[test]
fn zero_commit_repo_reports_failure_without_panicking() {
    let tmp = tempdir().expect("temp dir");
    let repo = tmp.path().join("zero");
    std::fs::create_dir_all(&repo).expect("create repo");
    run_git(&repo, &["init"]);

    let cfg = processor_config(tmp.path(), true);
    let result = process_repository(&repo, &cfg);

    assert!(matches!(result.status, RepoStatus::FetchFailed { .. }));
    assert!(!result.errors.is_empty());
}

#[test]
fn merge_commit_repo_is_processed_without_crash() {
    let tmp = tempdir().expect("temp dir");
    let repo = tmp.path().join("merge");
    std::fs::create_dir_all(&repo).expect("create repo");
    init_repo_with_commit(&repo);

    run_git(&repo, &["checkout", "-b", "feature"]);
    std::fs::write(repo.join("feature.txt"), "feature branch\n").expect("write feature");
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "feature change"]);

    run_git(&repo, &["checkout", "master"]);
    std::fs::write(repo.join("master.txt"), "master branch\n").expect("write master");
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "master change"]);

    run_git(&repo, &["merge", "--no-ff", "--no-edit", "feature"]);

    let mut cfg = processor_config(tmp.path(), true);
    cfg.history_depth = 3;
    let result = process_repository(&repo, &cfg);

    assert!(matches!(result.status, RepoStatus::UpToDate));
    assert!(!result.diffs.is_empty());
}

#[test]
fn scanner_handles_submodule_layout() {
    let tmp = tempdir().expect("temp dir");
    let root = tmp.path();

    let child = root.join("child");
    std::fs::create_dir_all(&child).expect("create child");
    init_repo_with_commit(&child);

    let parent = root.join("parent");
    std::fs::create_dir_all(&parent).expect("create parent");
    init_repo_with_commit(&parent);

    run_git(
        &parent,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            child.to_string_lossy().as_ref(),
            "modules/child",
        ],
    );
    run_git(&parent, &["commit", "-m", "add submodule"]);

    let discovered = discover_repositories(
        root,
        &ScanOptions {
            nested: true,
            follow_symlinks: false,
            skip_hidden: false,
            include_bare: false,
        },
    )
    .expect("scan root");

    assert!(discovered.contains(&parent));
}

#[test]
fn non_utf8_file_changes_are_processed_without_panicking() {
    let tmp = tempdir().expect("temp dir");
    let repo = tmp.path().join("non-utf8");
    std::fs::create_dir_all(&repo).expect("create repo");
    init_repo_with_commit(&repo);

    std::fs::write(repo.join("blob.bin"), [0xff_u8, 0xfe, 0x00]).expect("write binary v1");
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "add binary"]);

    std::fs::write(repo.join("blob.bin"), [0xff_u8, 0xfe, 0x01]).expect("write binary v2");
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "update binary"]);

    let cfg = processor_config(tmp.path(), true);
    let result = process_repository(&repo, &cfg);

    assert!(matches!(result.status, RepoStatus::UpToDate));
    assert!(!result.diffs.is_empty());
    assert!(result.diffs.iter().any(|diff| {
        diff.file_changes
            .iter()
            .any(|f| f.path.ends_with("blob.bin") && (f.is_binary || f.elements.is_empty()))
    }));
}

#[test]
fn history_depth_one_produces_empty_diff_set_for_up_to_date_repo() {
    let tmp = tempdir().expect("temp dir");
    let repo = tmp.path().join("single-history");
    std::fs::create_dir_all(&repo).expect("create repo");
    init_repo_with_commit(&repo);

    let mut cfg = processor_config(tmp.path(), true);
    cfg.history_depth = 1;

    let result = process_repository(&repo, &cfg);
    assert!(matches!(result.status, RepoStatus::UpToDate));
    assert!(result.diffs.is_empty());
}

fn processor_config(root: &Path, include_detached: bool) -> ProcessorConfig {
    ProcessorConfig {
        root_dir: root.to_path_buf(),
        report_dir: root.join("report"),
        timeout_secs: 30,
        pull_mode: false,
        force_pull: false,
        pull_strategy: PullStrategy::FfOnly,
        no_pull: true,
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
        include_detached,
        include_test_security: false,
        tag_definitions: builtin_patterns(),
        verbose: false,
    }
}

fn init_repo_with_commit(path: &Path) {
    run_git(path, &["init"]);
    std::fs::write(path.join("file.txt"), "hello\n").expect("write file");
    run_git(path, &["add", "."]);
    run_git(
        path,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-m",
            "init",
        ],
    );
}

fn run_git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .status()
        .expect("run git command");
    assert!(status.success(), "git command failed: {:?}", args);
}
