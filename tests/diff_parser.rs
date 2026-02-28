use diffcatcher::extraction::parser::parse_unified_diff;

#[test]
fn parser_sets_old_path_to_none_for_added_files() {
    let patch = r#"diff --git a/src/new.rs b/src/new.rs
new file mode 100644
index 0000000..1111111
--- /dev/null
+++ b/src/new.rs
@@ -0,0 +1,2 @@
+pub fn newly_added() {}
+const VERSION: &str = "1";
"#;

    let parsed = parse_unified_diff(patch);
    assert_eq!(parsed.files.len(), 1);
    let file = &parsed.files[0];
    assert_eq!(file.old_path, None);
    assert_eq!(file.new_path, "src/new.rs");
    assert_eq!(file.insertions, 2);
    assert_eq!(file.deletions, 0);
}

#[test]
fn parser_preserves_deleted_file_path_from_diff_header() {
    let patch = r#"diff --git a/src/old.rs b/src/old.rs
deleted file mode 100644
index 1111111..0000000
--- a/src/old.rs
+++ /dev/null
@@ -1,2 +0,0 @@
-pub fn deprecated() {}
-const FLAG: bool = true;
"#;

    let parsed = parse_unified_diff(patch);
    assert_eq!(parsed.files.len(), 1);
    let file = &parsed.files[0];
    assert_eq!(file.old_path.as_deref(), Some("src/old.rs"));
    assert_eq!(file.new_path, "src/old.rs");
    assert_eq!(file.insertions, 0);
    assert_eq!(file.deletions, 2);
}

#[test]
fn parser_tracks_rename_paths_and_hunk_context() {
    let patch = r#"diff --git a/src/old_name.rs b/src/new_name.rs
similarity index 80%
rename from src/old_name.rs
rename to src/new_name.rs
@@ -1 +1 @@ fn greet()
-pub fn greet() {}
+pub fn greet(name: &str) {}
"#;

    let parsed = parse_unified_diff(patch);
    assert_eq!(parsed.files.len(), 1);
    let file = &parsed.files[0];
    assert_eq!(file.old_path.as_deref(), Some("src/old_name.rs"));
    assert_eq!(file.new_path, "src/new_name.rs");
    assert_eq!(file.insertions, 1);
    assert_eq!(file.deletions, 1);
    assert_eq!(file.hunks.len(), 1);
    assert_eq!(
        file.hunks[0].context_function.as_deref(),
        Some("fn greet()")
    );
}

#[test]
fn parser_handles_binary_added_file_dev_null_old_side() {
    let patch = r#"diff --git a/assets/logo.bin b/assets/logo.bin
new file mode 100644
index 0000000..1111111
Binary files /dev/null and b/assets/logo.bin differ
"#;

    let parsed = parse_unified_diff(patch);
    assert_eq!(parsed.files.len(), 1);
    let file = &parsed.files[0];
    assert!(file.is_binary);
    assert_eq!(file.old_path, None);
    assert_eq!(file.new_path, "assets/logo.bin");
    assert_eq!(file.hunks.len(), 0);
    assert_eq!(file.insertions, 0);
    assert_eq!(file.deletions, 0);
}

#[test]
fn parser_handles_binary_deleted_file_dev_null_new_side() {
    let patch = r#"diff --git a/assets/old.bin b/assets/old.bin
deleted file mode 100644
index 1111111..0000000
Binary files a/assets/old.bin and /dev/null differ
"#;

    let parsed = parse_unified_diff(patch);
    assert_eq!(parsed.files.len(), 1);
    let file = &parsed.files[0];
    assert!(file.is_binary);
    assert_eq!(file.old_path.as_deref(), Some("assets/old.bin"));
    assert_eq!(file.new_path, "assets/old.bin");
    assert_eq!(file.hunks.len(), 0);
    assert_eq!(file.insertions, 0);
    assert_eq!(file.deletions, 0);
}
