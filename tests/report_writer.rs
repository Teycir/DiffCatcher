use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::Utc;
use git_patrol::cli::SummaryFormat;
use git_patrol::report::writer::{write_repo_report, write_top_level_reports};
use git_patrol::security::overview::build_global_security_overview;
use git_patrol::types::{
    CaptureScope, ChangeType, ChangedElement, CodeSnippet, CommitInfo, DiffResult, ElementKind,
    ElementSummary, FileChangeDetail, FileStatus, GlobalSummary, KindCounts, Language, RepoResult,
    RepoStatus, SecurityReview, SnippetContent,
};
use tempfile::tempdir;

#[test]
fn report_writer_outputs_expected_structure() {
    let tmp = tempdir().expect("temp dir");
    let report_dir = tmp.path().join("report");
    std::fs::create_dir_all(&report_dir).expect("create report dir");

    let element = ChangedElement {
        kind: ElementKind::Function,
        name: "validate_token".to_string(),
        change_type: ChangeType::Modified,
        file_path: "src/auth.rs".to_string(),
        line_range: Some((10, 20)),
        lines_added: 3,
        lines_removed: 2,
        enclosing_context: Some("impl AuthService".to_string()),
        signature: Some("pub fn validate_token(token: &str) -> bool".to_string()),
        snippet: CodeSnippet {
            before: Some(SnippetContent {
                code: "pub fn validate_token(token: &str) -> bool { false }".to_string(),
                start_line: 10,
                end_line: 10,
                commit: "aaaa1111".to_string(),
            }),
            after: Some(SnippetContent {
                code: "pub fn validate_token(token: &str) -> bool { true }".to_string(),
                start_line: 10,
                end_line: 10,
                commit: "bbbb2222".to_string(),
            }),
            diff_lines: "-pub fn validate_token...\n+pub fn validate_token...".to_string(),
            capture_scope: CaptureScope::HunkWithContext { context_lines: 5 },
        },
        security_tags: vec!["authentication".to_string()],
        in_test: false,
        snippet_files: None,
    };

    let mut by_change_type = BTreeMap::new();
    by_change_type.insert(ChangeType::Modified, 1);

    let mut by_kind = BTreeMap::new();
    by_kind.insert(
        ElementKind::Function,
        KindCounts {
            added: 0,
            modified: 1,
            removed: 0,
        },
    );

    let diff = DiffResult {
        label: "N_vs_N-1".to_string(),
        from_commit: commit("aaaa1111", "aaaa111"),
        to_commit: commit("bbbb2222", "bbbb222"),
        files_changed: 1,
        insertions: 3,
        deletions: 2,
        file_changes: vec![FileChangeDetail {
            path: "src/auth.rs".to_string(),
            old_path: Some("src/auth.rs".to_string()),
            status: FileStatus::Modified,
            language: Language::Rust,
            insertions: 3,
            deletions: 2,
            elements: vec![element.clone()],
            raw_hunks: Vec::new(),
            is_binary: false,
        }],
        element_summary: Some(ElementSummary {
            total_elements: 1,
            by_change_type,
            by_kind,
            elements: vec![element.clone()],
            top_elements: vec!["Modified validate_token (src/auth.rs)".to_string()],
        }),
        security_review: Some(SecurityReview {
            total_security_tagged_elements: 1,
            by_tag: BTreeMap::from([("authentication".to_string(), 1)]),
            by_severity: BTreeMap::new(),
            high_attention_items: Vec::new(),
            flagged_elements: vec![element],
        }),
        patch_filename: "diffs/diff_N_vs_N-1.patch".to_string(),
        changes_filename: "diffs/changes_N_vs_N-1.txt".to_string(),
        summary_json_filename: None,
        summary_txt_filename: None,
        summary_md_filename: None,
        snippets_dir: None,
    };

    let mut repo = RepoResult {
        repo_path: PathBuf::from("/tmp/example/repo"),
        repo_name: "repo".to_string(),
        report_folder_name: "repo".to_string(),
        branch: "main".to_string(),
        status: RepoStatus::Updated,
        pre_pull: Some(commit("aaaa1111", "aaaa111")),
        post_pull: Some(commit("bbbb2222", "bbbb222")),
        diffs: vec![diff],
        pull_log: "fetch ok".to_string(),
        errors: Vec::new(),
        timestamp: Utc::now(),
    };

    write_repo_report(
        &report_dir,
        &mut repo,
        &[SummaryFormat::Json, SummaryFormat::Txt, SummaryFormat::Md],
    )
    .expect("write repo report");

    assert!(report_dir.join("repo/status.json").exists());
    assert!(report_dir.join("repo/status.txt").exists());
    assert!(report_dir.join("repo/status.md").exists());
    assert!(report_dir.join("repo/pull_log.txt").exists());
    assert!(report_dir.join("repo/diffs/summary_N_vs_N-1.json").exists());
    assert!(report_dir.join("repo/diffs/summary_N_vs_N-1.txt").exists());
    assert!(report_dir.join("repo/diffs/summary_N_vs_N-1.md").exists());

    let snippets_dir = report_dir.join("repo/diffs/snippets");
    assert!(snippets_dir.exists());
    assert!(
        std::fs::read_dir(&snippets_dir)
            .expect("read snippets dir")
            .next()
            .is_some()
    );

    let repos = vec![repo.clone()];
    let summary =
        GlobalSummary::from_results(PathBuf::from("/tmp/root"), report_dir.clone(), &repos);
    let overview = build_global_security_overview(&repos);
    write_top_level_reports(&report_dir, &summary, &overview).expect("write top-level reports");

    assert!(report_dir.join("summary.json").exists());
    assert!(report_dir.join("summary.txt").exists());
    assert!(report_dir.join("summary.md").exists());
    assert!(report_dir.join("security_overview.json").exists());
    assert!(report_dir.join("security_overview.txt").exists());
    assert!(report_dir.join("security_overview.md").exists());
}

fn commit(hash: &str, short_hash: &str) -> CommitInfo {
    CommitInfo {
        hash: hash.to_string(),
        short_hash: short_hash.to_string(),
        message: "message".to_string(),
        full_message: "message".to_string(),
        author: "Author <author@example.com>".to_string(),
        timestamp: Utc::now().to_rfc3339(),
    }
}
