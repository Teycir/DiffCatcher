use git_patrol::security::patterns::builtin_patterns;
use git_patrol::security::tagger::tag_file_changes;
use git_patrol::types::{
    CaptureScope, ChangeType, ChangedElement, CodeSnippet, FileChangeDetail, FileStatus, Language,
};

#[test]
fn tagger_applies_authentication_and_secrets_tags() {
    let mut file_changes = vec![FileChangeDetail {
        path: "src/auth.rs".to_string(),
        old_path: Some("src/auth.rs".to_string()),
        status: FileStatus::Modified,
        language: Language::Rust,
        insertions: 4,
        deletions: 1,
        elements: vec![ChangedElement {
            kind: git_patrol::types::ElementKind::Function,
            name: "login".to_string(),
            change_type: ChangeType::Modified,
            file_path: "src/auth.rs".to_string(),
            line_range: Some((1, 10)),
            lines_added: 4,
            lines_removed: 1,
            enclosing_context: None,
            signature: Some("pub fn login(user: &str, password: &str)".to_string()),
            snippet: CodeSnippet {
                before: None,
                after: None,
                diff_lines: "+ let jwt = issue_token(password);".to_string(),
                capture_scope: CaptureScope::DiffOnly,
            },
            security_tags: Vec::new(),
            in_test: false,
            snippet_files: None,
        }],
        raw_hunks: Vec::new(),
        is_binary: false,
    }];

    let defs = builtin_patterns();
    let review = tag_file_changes(&mut file_changes, &defs, false).expect("tagging should work");

    assert!(review.total_security_tagged_elements >= 1);
    let tags = &file_changes[0].elements[0].security_tags;
    assert!(tags.iter().any(|t| t == "authentication"));
    assert!(tags.iter().any(|t| t == "secrets"));
}

#[test]
fn tagger_suppresses_network_tag_for_test_url_false_positive() {
    let mut file_changes = vec![FileChangeDetail {
        path: "src/mock.rs".to_string(),
        old_path: Some("src/mock.rs".to_string()),
        status: FileStatus::Modified,
        language: Language::Rust,
        insertions: 2,
        deletions: 0,
        elements: vec![ChangedElement {
            kind: git_patrol::types::ElementKind::Constant,
            name: "TEST_URL".to_string(),
            change_type: ChangeType::Added,
            file_path: "src/mock.rs".to_string(),
            line_range: Some((1, 2)),
            lines_added: 2,
            lines_removed: 0,
            enclosing_context: None,
            signature: Some("const TEST_URL: &str = \"https://example.com\";".to_string()),
            snippet: CodeSnippet {
                before: None,
                after: None,
                diff_lines:
                    "+ const TEST_URL: &str = \"https://example.com\";\n+ let mock_request = true;"
                        .to_string(),
                capture_scope: CaptureScope::DiffOnly,
            },
            security_tags: Vec::new(),
            in_test: false,
            snippet_files: None,
        }],
        raw_hunks: Vec::new(),
        is_binary: false,
    }];

    let defs = builtin_patterns();
    let _ = tag_file_changes(&mut file_changes, &defs, false).expect("tagging should work");

    let tags = &file_changes[0].elements[0].security_tags;
    assert!(!tags.iter().any(|t| t == "network"));
}
