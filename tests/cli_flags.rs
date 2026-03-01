use std::path::Path;
use std::process::Command;

use tempfile::tempdir;

#[test]
fn force_pull_requires_pull_flag() {
    let tmp = tempdir().expect("temp dir");
    let output = Command::new(bin())
        .arg(tmp.path())
        .arg("--force-pull")
        .output()
        .expect("run diffcatcher");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--force-pull requires --pull"));
}

#[test]
fn pull_and_no_pull_are_mutually_exclusive() {
    let tmp = tempdir().expect("temp dir");
    let output = Command::new(bin())
        .arg(tmp.path())
        .arg("--pull")
        .arg("--no-pull")
        .output()
        .expect("run diffcatcher");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--pull and --no-pull are mutually exclusive"));
}

#[test]
fn watch_requires_positive_interval() {
    let tmp = tempdir().expect("temp dir");
    let output = Command::new(bin())
        .arg(tmp.path())
        .arg("--watch")
        .arg("--watch-interval")
        .arg("0")
        .output()
        .expect("run diffcatcher");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--watch-interval must be >= 1 when --watch is enabled"));
}

#[test]
fn config_and_no_config_are_mutually_exclusive() {
    let tmp = tempdir().expect("temp dir");
    let cfg = tmp.path().join(".diffcatcher.toml");
    std::fs::write(&cfg, "no_pull = true\n").expect("write config");

    let output = Command::new(bin())
        .arg(tmp.path())
        .arg("--config")
        .arg(&cfg)
        .arg("--no-config")
        .output()
        .expect("run diffcatcher");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--config cannot be used together with --no-config"));
}

#[test]
fn no_snippets_and_no_security_tags_skip_snippet_dir_and_overview() {
    let tmp = tempdir().expect("temp dir");
    let root = tmp.path().join("root");
    let repo = root.join("repo");
    std::fs::create_dir_all(&repo).expect("create repo");
    init_repo(&repo);

    std::fs::write(repo.join("main.rs"), "fn a() -> i32 { 1 }\n").expect("write c1");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "c1"]);

    std::fs::write(
        repo.join("main.rs"),
        "fn a() -> i32 { 2 }\nfn validate_token() -> bool { true }\n",
    )
    .expect("write c2");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "c2"]);

    std::fs::write(repo.join("main.rs"), "fn a() -> i32 { 3 }\n").expect("write c3");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "c3"]);

    let report = tmp.path().join("report");
    let output = Command::new(bin())
        .arg(&root)
        .arg("-o")
        .arg(&report)
        .arg("--no-pull")
        .arg("--history-depth")
        .arg("2")
        .arg("--no-snippets")
        .arg("--no-security-tags")
        .arg("--summary-format")
        .arg("json,txt,md")
        .output()
        .expect("run diffcatcher");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(!report.join("security_overview.json").exists());
    assert!(!report.join("security_overview.txt").exists());
    assert!(!report.join("security_overview.md").exists());

    let repo_dir = report.join("repo");
    assert!(!repo_dir.join("diffs/snippets").exists());

    let summary_path = repo_dir.join("diffs/summary_N-1_vs_N-2.json");
    let summary_raw = std::fs::read_to_string(summary_path).expect("read diff summary");
    let summary_json: serde_json::Value = serde_json::from_str(&summary_raw).expect("valid json");

    assert!(summary_json["security_review"].is_null());
    let snippets_dir = summary_json
        .get("snippets_dir")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    assert!(snippets_dir.is_null());

    let has_no_before_after = summary_json["file_changes"]
        .as_array()
        .expect("file changes")
        .iter()
        .flat_map(|file| file["elements"].as_array().cloned().unwrap_or_default())
        .all(|element| {
            element["snippet"]["before"].is_null() && element["snippet"]["after"].is_null()
        });
    assert!(has_no_before_after);
}

#[test]
fn dry_run_does_not_change_repository_head() {
    let tmp = tempdir().expect("temp dir");
    let root = tmp.path().join("root");
    let repo = root.join("repo");
    std::fs::create_dir_all(&repo).expect("create repo");
    init_repo(&repo);

    std::fs::write(repo.join("main.rs"), "fn keep() {}\n").expect("write");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "init"]);
    let before = git_out(&repo, &["rev-parse", "HEAD"]);

    let report = tmp.path().join("report");
    let output = Command::new(bin())
        .arg(&root)
        .arg("-o")
        .arg(&report)
        .arg("--dry-run")
        .output()
        .expect("run diffcatcher");
    assert!(output.status.success());

    let after = git_out(&repo, &["rev-parse", "HEAD"]);
    assert_eq!(before, after);

    let porcelain = git_out(&repo, &["status", "--porcelain"]);
    assert!(porcelain.is_empty(), "repo should remain unchanged");
}

#[test]
fn partial_failures_exit_with_code_two() {
    let tmp = tempdir().expect("temp dir");
    let root = tmp.path().join("root");
    let good = root.join("good");
    let bad = root.join("bad");
    std::fs::create_dir_all(&good).expect("create good");
    std::fs::create_dir_all(&bad).expect("create bad");

    init_repo(&good);
    std::fs::write(good.join("ok.rs"), "fn ok() {}\n").expect("write");
    git(&good, &["add", "."]);
    git(&good, &["commit", "-m", "ok"]);

    // bad repo has no commits and should fail pre-state capture.
    git(&bad, &["init"]);

    let report = tmp.path().join("report");
    let output = Command::new(bin())
        .arg(&root)
        .arg("-o")
        .arg(&report)
        .arg("--no-pull")
        .output()
        .expect("run diffcatcher");

    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn verbose_mode_prints_discovered_repo_paths() {
    let tmp = tempdir().expect("temp dir");
    let root = tmp.path().join("root");
    let repo = root.join("repo");
    std::fs::create_dir_all(&repo).expect("create repo");
    init_repo(&repo);
    std::fs::write(repo.join("main.rs"), "fn main() {}\n").expect("write");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "init"]);

    let report = tmp.path().join("report");
    let output = Command::new(bin())
        .arg(&root)
        .arg("-o")
        .arg(&report)
        .arg("--no-pull")
        .arg("--history-depth")
        .arg("1")
        .arg("--verbose")
        .output()
        .expect("run diffcatcher");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(repo.to_string_lossy().as_ref()));
}

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_diffcatcher")
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
    assert!(status.success(), "git failed: {:?}", args);
}

fn git_out(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .expect("run git");
    assert!(output.status.success(), "git failed: {:?}", args);
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}
