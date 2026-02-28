use std::path::Path;
use std::process::Command;

use git_patrol::git::diff::{
    DiffPair, build_history_pairs, generate_diff_artifacts, safe_diff_pair,
};
use git_patrol::types::FileStatus;
use tempfile::tempdir;

#[test]
fn build_history_pairs_produces_expected_labels() {
    let pairs = build_history_pairs("abc123", 4, true);
    let labels = pairs.iter().map(|p| p.label.as_str()).collect::<Vec<_>>();
    assert_eq!(
        labels,
        vec!["N_vs_N-1", "N-1_vs_N-2", "N-2_vs_N-3", "N-3_vs_N-4"]
    );
}

#[test]
fn generate_diff_artifacts_for_known_commits() {
    let tmp = tempdir().expect("temp dir");
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(&repo).expect("create repo");
    init_repo(&repo);

    std::fs::write(repo.join("src.rs"), "pub fn a() -> i32 { 1 }\n").expect("write commit1");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "c1"]);
    let c1 = git_out(&repo, &["rev-parse", "HEAD"]);

    std::fs::write(
        repo.join("src.rs"),
        "pub fn a() -> i32 { 2 }\npub fn b() -> i32 { 3 }\n",
    )
    .expect("write commit2");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "c2"]);
    let c2 = git_out(&repo, &["rev-parse", "HEAD"]);

    let diff_dir = tmp.path().join("diffs");
    let pair = DiffPair {
        label: "N_vs_N-1".to_string(),
        from: c1.clone(),
        to: c2.clone(),
    };
    let artifacts = generate_diff_artifacts(&repo, &diff_dir, 30, &pair).expect("generate diff");

    assert_eq!(artifacts.files_changed, 1);
    assert!(artifacts.insertions >= 2);
    assert!(artifacts.deletions >= 1);
    assert!(diff_dir.join("diff_N_vs_N-1.patch").exists());
    assert!(diff_dir.join("changes_N_vs_N-1.txt").exists());

    let patch = std::fs::read_to_string(diff_dir.join("diff_N_vs_N-1.patch")).expect("read patch");
    assert!(patch.contains("pub fn a() -> i32 { 2 }"));
    assert!(patch.contains("pub fn b() -> i32 { 3 }"));

    let changes =
        std::fs::read_to_string(diff_dir.join("changes_N_vs_N-1.txt")).expect("read changes");
    assert!(changes.contains("# name-status"));
    assert!(
        artifacts
            .name_status
            .get("src.rs")
            .is_some_and(|entry| entry.status == FileStatus::Modified)
    );
}

#[test]
fn generate_diff_artifacts_tracks_rename_status() {
    let tmp = tempdir().expect("temp dir");
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(&repo).expect("create repo");
    init_repo(&repo);

    std::fs::write(repo.join("old.txt"), "line1\nline2\nline3\nline4\nline5\n").expect("write old");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "c1"]);
    let c1 = git_out(&repo, &["rev-parse", "HEAD"]);

    git(&repo, &["mv", "old.txt", "new.txt"]);
    std::fs::write(
        repo.join("new.txt"),
        "line1\nline2\nline3\nline4\nline5 changed\n",
    )
    .expect("write new");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "rename"]);
    let c2 = git_out(&repo, &["rev-parse", "HEAD"]);

    let pair = DiffPair {
        label: "N_vs_N-1".to_string(),
        from: c1.clone(),
        to: c2.clone(),
    };
    let artifacts =
        generate_diff_artifacts(&repo, &tmp.path().join("diffs"), 30, &pair).expect("artifacts");

    assert!(
        artifacts
            .name_status
            .get("new.txt")
            .is_some_and(|entry| entry.status == FileStatus::Renamed)
    );
}

#[test]
fn safe_diff_pair_rejects_missing_commit() {
    let tmp = tempdir().expect("temp dir");
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(&repo).expect("create repo");
    init_repo(&repo);

    std::fs::write(repo.join("a.txt"), "a\n").expect("write file");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "init"]);
    let head = git_out(&repo, &["rev-parse", "HEAD"]);

    let pair = DiffPair {
        label: "bad".to_string(),
        from: "does-not-exist".to_string(),
        to: head,
    };
    assert!(!safe_diff_pair(&repo, 30, &pair));
}

fn init_repo(path: &Path) {
    git(path, &["init"]);
    git(path, &["config", "user.name", "Test"]);
    git(path, &["config", "user.email", "test@example.com"]);
}

fn git(path: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .status()
        .expect("run git");
    assert!(status.success(), "git command failed: {:?}", args);
}

fn git_out(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .expect("run git");
    assert!(output.status.success(), "git command failed: {:?}", args);
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}
