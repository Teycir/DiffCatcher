use diffcatcher::security::patterns::builtin_patterns;
use diffcatcher::security::tagger::tag_file_changes;
use diffcatcher::types::{
    CaptureScope, ChangeType, ChangedElement, CodeSnippet, ElementKind, FileChangeDetail,
    FileStatus, Language, SnippetContent,
};

#[test]
fn security_review_matches_expected_tags_and_high_attention() {
    let mut file_changes = vec![
        FileChangeDetail {
            path: "src/auth/jwt.rs".to_string(),
            old_path: Some("src/auth/jwt.rs".to_string()),
            status: FileStatus::Modified,
            language: Language::Rust,
            insertions: 8,
            deletions: 0,
            elements: vec![ChangedElement {
                kind: ElementKind::Function,
                name: "validate_token".to_string(),
                change_type: ChangeType::Added,
                file_path: "src/auth/jwt.rs".to_string(),
                line_range: Some((10, 20)),
                lines_added: 8,
                lines_removed: 0,
                enclosing_context: Some("impl Auth".to_string()),
                signature: Some(
                    "pub fn validate_token(&self, token: &str) -> Result<Claims, Error>"
                        .to_string(),
                ),
                snippet: CodeSnippet {
                    before: None,
                    after: Some(SnippetContent {
                        code: "let key = DecodingKey::from_secret(self.secret.as_bytes());"
                            .to_string(),
                        start_line: 10,
                        end_line: 20,
                        commit: "new".to_string(),
                    }),
                    diff_lines: "+ let key = DecodingKey::from_secret(self.secret.as_bytes());"
                        .to_string(),
                    capture_scope: CaptureScope::FullElement,
                },
                security_tags: Vec::new(),
                in_test: false,
                snippet_files: None,
            }],
            raw_hunks: Vec::new(),
            is_binary: false,
        },
        FileChangeDetail {
            path: "src/middleware/auth.rs".to_string(),
            old_path: Some("src/middleware/auth.rs".to_string()),
            status: FileStatus::Modified,
            language: Language::Rust,
            insertions: 0,
            deletions: 3,
            elements: vec![ChangedElement {
                kind: ElementKind::Function,
                name: "check_permissions".to_string(),
                change_type: ChangeType::Removed,
                file_path: "src/middleware/auth.rs".to_string(),
                line_range: Some((30, 40)),
                lines_added: 0,
                lines_removed: 3,
                enclosing_context: Some("impl AuthMiddleware".to_string()),
                signature: Some(
                    "pub fn check_permissions(&self, user: &User, resource: &Resource) -> bool"
                        .to_string(),
                ),
                snippet: CodeSnippet {
                    before: Some(SnippetContent {
                        code: "if user.is_admin() { return true; }".to_string(),
                        start_line: 30,
                        end_line: 40,
                        commit: "old".to_string(),
                    }),
                    after: None,
                    diff_lines: "- if user.is_admin() { return true; }".to_string(),
                    capture_scope: CaptureScope::FullElement,
                },
                security_tags: Vec::new(),
                in_test: false,
                snippet_files: None,
            }],
            raw_hunks: Vec::new(),
            is_binary: false,
        },
    ];

    let defs = builtin_patterns();
    let review = tag_file_changes(&mut file_changes, &defs, false).expect("tagging");

    assert!(review.total_security_tagged_elements >= 2);
    assert!(review.by_tag.contains_key("authentication"));
    assert!(review.by_tag.contains_key("security-removal"));

    assert!(
        review
            .high_attention_items
            .iter()
            .any(|item| item.reason == "New crypto/auth code added")
    );
    assert!(
        review
            .high_attention_items
            .iter()
            .any(|item| item.reason == "Security control REMOVED")
    );
}
