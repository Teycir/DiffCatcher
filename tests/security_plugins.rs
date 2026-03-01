use diffcatcher::security::load_tag_definitions;
use tempfile::tempdir;

#[test]
fn security_plugin_file_extends_builtin_tags() {
    let tmp = tempdir().expect("temp dir");
    let plugin_file = tmp.path().join("security-plugin.json");
    std::fs::write(
        &plugin_file,
        r#"{
  "version": 1,
  "mode": "extend",
  "tags": [
    {
      "tag": "custom-risk",
      "description": "Custom security tag from plugin",
      "severity": "High",
      "patterns": ["allow_admin"]
    }
  ]
}"#,
    )
    .expect("write plugin file");

    let defs = load_tag_definitions(None, std::slice::from_ref(&plugin_file))
        .expect("load tag definitions");
    assert!(defs.iter().any(|d| d.tag == "custom-risk"));
}
