use std::collections::BTreeMap;
use std::process::Command;

use diffcatcher::extraction::{ExtractionOptions, extract_from_patch};
use diffcatcher::git::diff::NameStatusEntry;
use diffcatcher::scanner::{ScanOptions, discover_repositories};
use diffcatcher::types::{ElementKind, FileStatus};
use tempfile::tempdir;

#[test]
fn scanner_respects_nested_flag() {
    let tmp = tempdir().expect("temp dir");
    let root = tmp.path();

    let repo1 = root.join("repo1");
    std::fs::create_dir_all(&repo1).expect("create repo1");
    init_git_repo(&repo1);

    let nested_repo = repo1.join("nested").join("repo2");
    std::fs::create_dir_all(&nested_repo).expect("create nested repo");
    init_git_repo(&nested_repo);

    let no_nested = discover_repositories(
        root,
        &ScanOptions {
            nested: false,
            follow_symlinks: false,
            skip_hidden: false,
            include_bare: false,
        },
    )
    .expect("discover without nested");

    assert_eq!(no_nested.len(), 1);
    assert_eq!(no_nested[0], repo1);

    let with_nested = discover_repositories(
        root,
        &ScanOptions {
            nested: true,
            follow_symlinks: false,
            skip_hidden: false,
            include_bare: false,
        },
    )
    .expect("discover with nested");

    assert_eq!(with_nested.len(), 2);
    assert!(with_nested.contains(&repo1));
    assert!(with_nested.contains(&nested_repo));
}

#[test]
fn scanner_respects_skip_hidden() {
    let tmp = tempdir().expect("temp dir");
    let root = tmp.path();

    let visible = root.join("visible");
    let hidden_parent = root.join(".hidden");
    let hidden = hidden_parent.join("hidden_repo");

    std::fs::create_dir_all(&visible).expect("create visible");
    std::fs::create_dir_all(&hidden).expect("create hidden");
    init_git_repo(&visible);
    init_git_repo(&hidden);

    let discovered = discover_repositories(
        root,
        &ScanOptions {
            nested: true,
            follow_symlinks: false,
            skip_hidden: true,
            include_bare: false,
        },
    )
    .expect("discover with skip hidden");

    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0], visible);
}

#[cfg(unix)]
#[test]
fn scanner_can_follow_symlinks_when_enabled() {
    use std::os::unix::fs::symlink;

    let tmp = tempdir().expect("temp dir");
    let root = tmp.path();

    let real_repo = root.join("real");
    let symlink_repo = root.join("real-link");
    std::fs::create_dir_all(&real_repo).expect("create real");
    init_git_repo(&real_repo);
    symlink(&real_repo, &symlink_repo).expect("create symlink");

    let without_follow = discover_repositories(
        root,
        &ScanOptions {
            nested: true,
            follow_symlinks: false,
            skip_hidden: false,
            include_bare: false,
        },
    )
    .expect("discover without following symlink");
    assert_eq!(without_follow.len(), 1);
    assert!(without_follow.contains(&real_repo));

    let with_follow = discover_repositories(
        root,
        &ScanOptions {
            nested: true,
            follow_symlinks: true,
            skip_hidden: false,
            include_bare: false,
        },
    )
    .expect("discover with following symlink");
    assert!(with_follow.contains(&real_repo));
    assert!(with_follow.contains(&symlink_repo));
}

#[test]
fn extraction_detects_function_element() {
    let patch = r#"diff --git a/src/lib.rs b/src/lib.rs
index 123..456 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,4 +1,4 @@ fn validate_token(token: &str) -> bool {
-pub fn validate_token(token: &str) -> bool { false }
+pub fn validate_token(token: &str) -> bool { true }
 }
"#;

    let mut ns = BTreeMap::new();
    ns.insert(
        "src/lib.rs".to_string(),
        NameStatusEntry {
            status: FileStatus::Modified,
            old_path: Some("src/lib.rs".to_string()),
            new_path: "src/lib.rs".to_string(),
        },
    );

    let (files, summary) = extract_from_patch(
        patch,
        &ns,
        "aaaa1111",
        "bbbb2222",
        &ExtractionOptions {
            no_summary_extraction: false,
            no_snippets: false,
            snippet_context: 5,
            max_snippet_lines: 200,
            max_elements: 100,
            include_vendor: false,
            plugin_extractors: Vec::new(),
        },
    );

    assert_eq!(files.len(), 1);
    let summary = summary.expect("summary");
    assert!(summary.total_elements >= 1);
    assert!(
        summary
            .elements
            .iter()
            .any(|el| el.kind == ElementKind::Function && el.name == "validate_token")
    );
}

fn init_git_repo(path: &std::path::Path) {
    let status = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("init")
        .arg("-q")
        .status()
        .expect("run git init");
    assert!(status.success());
}
