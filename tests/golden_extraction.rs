use std::collections::BTreeMap;

use git_patrol::extraction::{ExtractionOptions, extract_from_patch};
use git_patrol::git::diff::NameStatusEntry;
use git_patrol::types::{CaptureScope, ChangeType, ElementKind, FileStatus, Language};

#[test]
fn rust_modified_function_has_before_after_and_full_element_scope() {
    let patch = r#"diff --git a/src/lib.rs b/src/lib.rs
index 123..456 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,4 +1,4 @@ fn validate_token(token: &str) -> bool {
-pub fn validate_token(token: &str) -> bool { false }
+pub fn validate_token(token: &str) -> bool { true }
 }
"#;

    let (files, summary) = extract(
        patch,
        "src/lib.rs",
        FileStatus::Modified,
        Some("src/lib.rs"),
    );
    assert_eq!(files[0].language, Language::Rust);
    let summary = summary.expect("summary");
    let element = summary
        .elements
        .iter()
        .find(|e| e.kind == ElementKind::Function && e.name == "validate_token")
        .expect("validate_token element");

    assert_eq!(element.change_type, ChangeType::Modified);
    assert!(element.snippet.before.is_some());
    assert!(element.snippet.after.is_some());
    assert!(matches!(
        element.snippet.capture_scope,
        CaptureScope::FullElement
    ));
}

#[test]
fn python_added_function_has_null_before_and_after_code() {
    let patch = r#"diff --git a/app/auth.py b/app/auth.py
new file mode 100644
index 0000000..1111111
--- /dev/null
+++ b/app/auth.py
@@ -0,0 +1,3 @@
+def check_permissions(user):
+    return user.is_admin
+"#;

    let (files, summary) = extract(patch, "app/auth.py", FileStatus::Added, None);
    assert_eq!(files[0].language, Language::Python);
    let summary = summary.expect("summary");
    let element = summary
        .elements
        .iter()
        .find(|e| e.kind == ElementKind::Function && e.name == "check_permissions")
        .expect("check_permissions element");

    assert_eq!(element.change_type, ChangeType::Added);
    assert!(element.snippet.before.is_none());
    assert!(element.snippet.after.is_some());
}

#[test]
fn javascript_removed_function_has_null_after() {
    let patch = r#"diff --git a/src/auth.js b/src/auth.js
index 3333333..4444444 100644
--- a/src/auth.js
+++ b/src/auth.js
@@ -1,3 +0,0 @@
-function legacyLogin(user) {
-  return user && user.admin;
-}
"#;

    let (files, summary) = extract(
        patch,
        "src/auth.js",
        FileStatus::Deleted,
        Some("src/auth.js"),
    );
    assert_eq!(files[0].language, Language::JavaScript);
    let summary = summary.expect("summary");
    let element = summary
        .elements
        .iter()
        .find(|e| e.kind == ElementKind::Function && e.name == "legacyLogin")
        .expect("legacyLogin element");

    assert_eq!(element.change_type, ChangeType::Removed);
    assert!(element.snippet.before.is_some());
    assert!(element.snippet.after.is_none());
}

#[test]
fn go_modified_function_is_detected() {
    let patch = r#"diff --git a/internal/auth.go b/internal/auth.go
index 5555555..6666666 100644
--- a/internal/auth.go
+++ b/internal/auth.go
@@ -1,4 +1,4 @@ func ValidateToken(token string) bool {
-func ValidateToken(token string) bool { return false }
+func ValidateToken(token string) bool { return true }
 }
"#;

    let (files, summary) = extract(
        patch,
        "internal/auth.go",
        FileStatus::Modified,
        Some("internal/auth.go"),
    );
    assert_eq!(files[0].language, Language::Go);
    let summary = summary.expect("summary");
    assert!(
        summary
            .elements
            .iter()
            .any(|e| e.kind == ElementKind::Function && e.name == "ValidateToken")
    );
}

fn extract(
    patch: &str,
    new_path: &str,
    status: FileStatus,
    old_path: Option<&str>,
) -> (
    Vec<git_patrol::types::FileChangeDetail>,
    Option<git_patrol::types::ElementSummary>,
) {
    let mut ns = BTreeMap::new();
    ns.insert(
        new_path.to_string(),
        NameStatusEntry {
            status,
            old_path: old_path.map(ToString::to_string),
            new_path: new_path.to_string(),
        },
    );

    extract_from_patch(
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
    )
}
