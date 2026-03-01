use std::collections::BTreeMap;

use diffcatcher::extraction::plugins::load_extractor_plugins;
use diffcatcher::extraction::{ExtractionOptions, extract_from_patch};
use diffcatcher::git::diff::NameStatusEntry;
use diffcatcher::types::{ElementKind, FileStatus};
use tempfile::tempdir;

#[test]
fn custom_extractor_plugin_adds_elements() {
    let tmp = tempdir().expect("temp dir");
    let plugin_file = tmp.path().join("extractor-plugin.json");
    std::fs::write(
        &plugin_file,
        r#"{
  "version": 1,
  "extractors": [
    {
      "name": "policy-rule",
      "kind": "Config",
      "regex": "^policy\\s+([A-Za-z_][A-Za-z0-9_]*)"
    }
  ]
}"#,
    )
    .expect("write plugin file");

    let plugins =
        load_extractor_plugins(std::slice::from_ref(&plugin_file)).expect("load extractor plugins");
    assert_eq!(plugins.len(), 1);

    let patch = r#"diff --git a/rules.policy b/rules.policy
index 1111111..2222222 100644
--- a/rules.policy
+++ b/rules.policy
@@ -1 +1 @@
-policy deny_admin
+policy allow_admin
"#;

    let mut name_status = BTreeMap::new();
    name_status.insert(
        "rules.policy".to_string(),
        NameStatusEntry {
            status: FileStatus::Modified,
            old_path: Some("rules.policy".to_string()),
            new_path: "rules.policy".to_string(),
        },
    );

    let (files, summary) = extract_from_patch(
        patch,
        &name_status,
        "aaaa1111",
        "bbbb2222",
        &ExtractionOptions {
            no_summary_extraction: false,
            no_snippets: true,
            snippet_context: 2,
            max_snippet_lines: 20,
            max_elements: 100,
            include_vendor: false,
            plugin_extractors: plugins,
        },
    );

    assert_eq!(files.len(), 1);
    let summary = summary.expect("element summary");
    assert!(
        summary
            .elements
            .iter()
            .any(|e| e.kind == ElementKind::Config && e.name == "allow_admin")
    );
}
