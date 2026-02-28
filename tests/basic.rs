use std::collections::BTreeMap;
use std::process::Command;

use git_patrol::extraction::{ExtractionOptions, extract_from_patch};
use git_patrol::git::diff::NameStatusEntry;
use git_patrol::scanner::{ScanOptions, discover_repositories};
use git_patrol::types::{ElementKind, FileStatus};
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
        },
    );

    assert_eq!(files.len(), 1);
    let summary = summary.expect("summary");
    assert!(summary.total_elements >= 1);
    assert!(summary
        .elements
        .iter()
        .any(|el| el.kind == ElementKind::Function && el.name == "validate_token"));
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
