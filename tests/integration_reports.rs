use std::path::Path;
use std::process::Command;

use tempfile::tempdir;

#[test]
fn integration_run_writes_expected_report_structure() {
    let tmp = tempdir().expect("temp dir");
    let root = tmp.path().join("root");
    let repo = root.join("service");
    std::fs::create_dir_all(&repo).expect("create repo");
    init_repo(&repo);

    std::fs::write(repo.join("lib.rs"), "pub fn login() -> bool { false }\n").expect("write c1");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "c1"]);

    std::fs::write(
        repo.join("lib.rs"),
        "pub fn login(password: &str) -> bool { !password.is_empty() }\n",
    )
    .expect("write c2");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "c2"]);

    std::fs::write(
        repo.join("lib.rs"),
        "pub fn login(password: &str) -> bool { password.len() > 2 }\n",
    )
    .expect("write c3");
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
        .arg("--summary-format")
        .arg("json,txt,md")
        .output()
        .expect("run diffcatcher");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    for required in [
        "summary.json",
        "summary.txt",
        "summary.md",
        "security_overview.json",
        "security_overview.txt",
        "security_overview.md",
    ] {
        assert!(
            report.join(required).exists(),
            "missing top-level file: {}",
            required
        );
    }

    let repo_dir = report.join("service");
    for required in ["status.json", "status.txt", "status.md", "pull_log.txt"] {
        assert!(
            repo_dir.join(required).exists(),
            "missing repo file: {}",
            required
        );
    }

    let diffs_dir = repo_dir.join("diffs");
    for required in [
        "diff_N-1_vs_N-2.patch",
        "changes_N-1_vs_N-2.txt",
        "summary_N-1_vs_N-2.json",
        "summary_N-1_vs_N-2.txt",
        "summary_N-1_vs_N-2.md",
    ] {
        assert!(
            diffs_dir.join(required).exists(),
            "missing diff file: {}",
            required
        );
    }

    let snippets_dir = diffs_dir.join("snippets");
    assert!(snippets_dir.exists(), "snippets directory missing");
    let snippet_count = std::fs::read_dir(&snippets_dir)
        .expect("read snippets")
        .count();
    assert!(snippet_count > 0, "expected snippet files");

    let summary_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(report.join("summary.json")).expect("read"))
            .expect("valid json");
    assert_eq!(summary_json["total_repos_found"], 1);
    assert!(summary_json["repos"].is_array());
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
