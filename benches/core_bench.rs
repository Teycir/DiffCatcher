use std::collections::BTreeMap;
use std::hint::black_box;
use std::time::{Duration, Instant};

use diffcatcher::extraction::parser::parse_unified_diff;
use diffcatcher::extraction::{ExtractionOptions, extract_from_patch};
use diffcatcher::git::diff::NameStatusEntry;
use diffcatcher::types::FileStatus;

const SAMPLE_PATCH: &str = r#"diff --git a/src/auth.rs b/src/auth.rs
index 1111111..2222222 100644
--- a/src/auth.rs
+++ b/src/auth.rs
@@ -1,9 +1,13 @@
 pub fn login(user: &str, password: &str) -> bool {
-    verify_password(user, password)
+    if !validate_input(user) {
+        return false;
+    }
+    verify_password(user, password) && issue_token(user).is_ok()
 }
 
 fn verify_password(user: &str, password: &str) -> bool {
     hash(password) == lookup(user)
 }
diff --git a/src/new.rs b/src/new.rs
new file mode 100644
index 0000000..3333333
--- /dev/null
+++ b/src/new.rs
@@ -0,0 +1,5 @@
+pub fn newly_added() -> &'static str {
+    "ok"
+}
+
+pub const VERSION: u32 = 1;
"#;

fn main() {
    let mut name_status = BTreeMap::new();
    name_status.insert(
        "src/auth.rs".to_string(),
        NameStatusEntry {
            status: FileStatus::Modified,
            old_path: Some("src/auth.rs".to_string()),
            new_path: "src/auth.rs".to_string(),
        },
    );
    name_status.insert(
        "src/new.rs".to_string(),
        NameStatusEntry {
            status: FileStatus::Added,
            old_path: None,
            new_path: "src/new.rs".to_string(),
        },
    );

    let options = ExtractionOptions {
        no_summary_extraction: false,
        no_snippets: false,
        snippet_context: 5,
        max_snippet_lines: 200,
        max_elements: 500,
        include_vendor: false,
        plugin_extractors: Vec::new(),
    };

    let parse_runs = 10_000;
    let parse_elapsed = bench(parse_runs, || {
        let _ = black_box(parse_unified_diff(black_box(SAMPLE_PATCH)));
    });

    let extract_runs = 5_000;
    let extract_elapsed = bench(extract_runs, || {
        let _ = black_box(extract_from_patch(
            black_box(SAMPLE_PATCH),
            black_box(&name_status),
            black_box("1111111"),
            black_box("2222222"),
            black_box(&options),
        ));
    });

    println!(
        "parse_unified_diff: runs={} total_ms={} avg_us={:.2}",
        parse_runs,
        parse_elapsed.as_millis(),
        parse_elapsed.as_secs_f64() * 1_000_000.0 / parse_runs as f64
    );
    println!(
        "extract_from_patch: runs={} total_ms={} avg_us={:.2}",
        extract_runs,
        extract_elapsed.as_millis(),
        extract_elapsed.as_secs_f64() * 1_000_000.0 / extract_runs as f64
    );
}

fn bench<F>(runs: usize, mut f: F) -> Duration
where
    F: FnMut(),
{
    let start = Instant::now();
    for _ in 0..runs {
        f();
    }
    start.elapsed()
}
