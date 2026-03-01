use std::path::Path;
use std::process::Command;

use tempfile::tempdir;

#[test]
fn config_file_controls_output_formats_and_security_overview() {
    let tmp = tempdir().expect("temp dir");
    let root = tmp.path().join("root");
    let repo = root.join("repo");
    std::fs::create_dir_all(&repo).expect("create repo");
    init_repo(&repo);

    std::fs::write(repo.join("main.rs"), "fn a() -> i32 { 1 }\n").expect("write c1");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "c1"]);
    std::fs::write(repo.join("main.rs"), "fn a() -> i32 { 2 }\n").expect("write c2");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "c2"]);
    std::fs::write(repo.join("main.rs"), "fn a() -> i32 { 3 }\n").expect("write c3");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "c3"]);

    let cfg = r#"
output = "cfg-report"
no_pull = true
history_depth = 2
summary_formats = ["json", "txt"]
no_security_tags = true
"#;
    std::fs::write(root.join(".diffcatcher.toml"), cfg).expect("write config");

    let output = Command::new(bin())
        .arg(&root)
        .output()
        .expect("run diffcatcher");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let report = root.join("cfg-report");
    assert!(report.join("summary.json").exists());
    assert!(report.join("summary.txt").exists());
    assert!(!report.join("security_overview.json").exists());
    assert!(report.join("repo/diffs/summary_N-1_vs_N-2.json").exists());
    assert!(report.join("repo/diffs/summary_N-1_vs_N-2.txt").exists());
    assert!(!report.join("repo/diffs/summary_N-1_vs_N-2.md").exists());
}

#[test]
fn cli_flags_override_config_defaults() {
    let tmp = tempdir().expect("temp dir");
    let root = tmp.path().join("root");
    let repo = root.join("repo");
    std::fs::create_dir_all(&repo).expect("create repo");
    init_repo(&repo);

    std::fs::write(repo.join("main.rs"), "fn a() -> i32 { 1 }\n").expect("write c1");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "c1"]);
    std::fs::write(repo.join("main.rs"), "fn a() -> i32 { 2 }\n").expect("write c2");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "c2"]);
    std::fs::write(repo.join("main.rs"), "fn a() -> i32 { 3 }\n").expect("write c3");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "c3"]);

    let cfg = r#"
output = "cfg-report"
no_pull = true
history_depth = 2
summary_formats = ["json"]
"#;
    std::fs::write(root.join(".diffcatcher.toml"), cfg).expect("write config");

    let output = Command::new(bin())
        .arg(&root)
        .arg("--summary-format")
        .arg("json,txt")
        .output()
        .expect("run diffcatcher");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let diff_summary = root.join("cfg-report/repo/diffs/summary_N-1_vs_N-2.json");
    assert!(diff_summary.exists(), "expected {}", diff_summary.display());
    let diff_summary_txt = root.join("cfg-report/repo/diffs/summary_N-1_vs_N-2.txt");
    assert!(
        diff_summary_txt.exists(),
        "expected {}",
        diff_summary_txt.display()
    );
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
