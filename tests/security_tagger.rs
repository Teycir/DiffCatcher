use diffcatcher::security::patterns::builtin_patterns;
use diffcatcher::security::tagger::tag_file_changes;
use diffcatcher::types::{
    CaptureScope, ChangeType, ChangedElement, CodeSnippet, FileChangeDetail, FileStatus, Language,
    SecurityTagDefinition, TagSeverity,
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
            kind: diffcatcher::types::ElementKind::Function,
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
            kind: diffcatcher::types::ElementKind::Constant,
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

#[test]
fn tagger_cache_is_keyed_by_definition_set() {
    let make_defs = |tag: &str, pattern: &str| {
        vec![SecurityTagDefinition {
            tag: tag.to_string(),
            patterns: vec![pattern.to_string()],
            negative_patterns: Vec::new(),
            description: "test".to_string(),
            severity: TagSeverity::Medium,
            min_matches: 1,
            pattern_kind: None,
            references: Vec::new(),
            false_positive_note: None,
        }]
    };

    let make_file = || FileChangeDetail {
        path: "src/cache.rs".to_string(),
        old_path: Some("src/cache.rs".to_string()),
        status: FileStatus::Modified,
        language: Language::Rust,
        insertions: 1,
        deletions: 0,
        elements: vec![ChangedElement {
            kind: diffcatcher::types::ElementKind::Function,
            name: "probe".to_string(),
            change_type: ChangeType::Modified,
            file_path: "src/cache.rs".to_string(),
            line_range: Some((1, 1)),
            lines_added: 1,
            lines_removed: 0,
            enclosing_context: None,
            signature: Some("fn probe()".to_string()),
            snippet: CodeSnippet {
                before: None,
                after: None,
                diff_lines: "+ dangerous_call();".to_string(),
                capture_scope: CaptureScope::DiffOnly,
            },
            security_tags: Vec::new(),
            in_test: false,
            snippet_files: None,
        }],
        raw_hunks: Vec::new(),
        is_binary: false,
    };

    let mut first = vec![make_file()];
    let first_defs = make_defs("alpha", "dangerous_call");
    let _ = tag_file_changes(&mut first, &first_defs, false).expect("first tag pass");
    assert!(
        first[0].elements[0]
            .security_tags
            .iter()
            .any(|t| t == "alpha")
    );

    let mut second = vec![make_file()];
    let second_defs = make_defs("beta", "safe_call");
    let _ = tag_file_changes(&mut second, &second_defs, false).expect("second tag pass");
    assert!(
        !second[0].elements[0]
            .security_tags
            .iter()
            .any(|t| t == "alpha")
    );
    assert!(
        !second[0].elements[0]
            .security_tags
            .iter()
            .any(|t| t == "beta")
    );
}
