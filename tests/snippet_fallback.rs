use diffcatcher::extraction::elements::DetectedElement;
use diffcatcher::extraction::snippets::{SnippetOptions, build_snippet};
use diffcatcher::types::{ChangeType, ElementKind, RawHunk};

fn options() -> SnippetOptions {
    SnippetOptions {
        context_lines: 5,
        max_snippet_lines: 200,
        no_snippets: false,
    }
}

#[test]
fn modified_element_with_only_added_lines_still_gets_before_and_after() {
    let element = DetectedElement {
        kind: ElementKind::Other,
        name: "body_changes_hunk_10".to_string(),
        change_type: ChangeType::Modified,
        line_range: Some((10, 10)),
        lines_added: 1,
        lines_removed: 0,
        enclosing_context: None,
        signature: None,
    };
    let hunks = vec![RawHunk {
        header: "@@ -10,0 +10,1 @@".to_string(),
        old_start: 10,
        old_count: 0,
        new_start: 10,
        new_count: 1,
        context_function: Some("body_changes_hunk_10".to_string()),
        lines: "+new_value=true".to_string(),
    }];

    let snippet = build_snippet(&element, &hunks, "old", "new", &options());
    assert!(snippet.before.is_some());
    assert!(snippet.after.is_some());
}

#[test]
fn modified_element_with_only_removed_lines_still_gets_before_and_after() {
    let element = DetectedElement {
        kind: ElementKind::Other,
        name: "body_changes_hunk_11".to_string(),
        change_type: ChangeType::Modified,
        line_range: Some((11, 11)),
        lines_added: 0,
        lines_removed: 1,
        enclosing_context: None,
        signature: None,
    };
    let hunks = vec![RawHunk {
        header: "@@ -11,1 +11,0 @@".to_string(),
        old_start: 11,
        old_count: 1,
        new_start: 11,
        new_count: 0,
        context_function: Some("body_changes_hunk_11".to_string()),
        lines: "-legacy_value=true".to_string(),
    }];

    let snippet = build_snippet(&element, &hunks, "old", "new", &options());
    assert!(snippet.before.is_some());
    assert!(snippet.after.is_some());
}

#[test]
fn modified_element_with_no_diff_lines_gets_placeholder_before_and_after() {
    let element = DetectedElement {
        kind: ElementKind::Other,
        name: "body_changes_in_file".to_string(),
        change_type: ChangeType::Modified,
        line_range: None,
        lines_added: 0,
        lines_removed: 0,
        enclosing_context: None,
        signature: None,
    };
    let hunks = vec![RawHunk {
        header: "@@ -1,0 +1,0 @@".to_string(),
        old_start: 1,
        old_count: 0,
        new_start: 1,
        new_count: 0,
        context_function: Some("body_changes_in_file".to_string()),
        lines: String::new(),
    }];

    let snippet = build_snippet(&element, &hunks, "old", "new", &options());
    assert!(snippet.before.is_some());
    assert!(snippet.after.is_some());
}
