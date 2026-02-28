use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "error")]
pub enum RepoStatus {
    Updated,
    UpToDate,
    DirtySkipped,
    FetchFailed { error: String },
    PullFailed { error: String },
    Skipped { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub full_message: String,
    pub author: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawHunk {
    pub header: String,
    pub old_start: u32,
    pub old_count: u32,
    pub new_start: u32,
    pub new_count: u32,
    pub context_function: Option<String>,
    pub lines: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    C,
    Cpp,
    Java,
    Kotlin,
    Ruby,
    Toml,
    Yaml,
    Json,
    Markdown,
    Shell,
    Dockerfile,
    Unknown(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ElementKind {
    Function,
    Method,
    Struct,
    Class,
    Enum,
    Trait,
    Interface,
    Impl,
    Module,
    Import,
    Constant,
    Static,
    TypeAlias,
    Macro,
    Test,
    Config,
    Other,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChangeType {
    Added,
    Modified,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetContent {
    pub code: String,
    pub start_line: u32,
    pub end_line: u32,
    pub commit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "scope", content = "meta")]
pub enum CaptureScope {
    FullElement,
    HunkWithContext { context_lines: u32 },
    DiffOnly,
    Truncated { actual_lines: u32, max_lines: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSnippet {
    pub before: Option<SnippetContent>,
    pub after: Option<SnippetContent>,
    pub diff_lines: String,
    pub capture_scope: CaptureScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetFileRefs {
    pub before: Option<String>,
    pub after: Option<String>,
    pub diff: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangedElement {
    pub kind: ElementKind,
    pub name: String,
    pub change_type: ChangeType,
    pub file_path: String,
    pub line_range: Option<(u32, u32)>,
    pub lines_added: u32,
    pub lines_removed: u32,
    pub enclosing_context: Option<String>,
    pub signature: Option<String>,
    pub snippet: CodeSnippet,
    pub security_tags: Vec<String>,
    pub in_test: bool,
    pub snippet_files: Option<SnippetFileRefs>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KindCounts {
    pub added: u32,
    pub modified: u32,
    pub removed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ElementSummary {
    pub total_elements: u32,
    pub by_change_type: BTreeMap<ChangeType, u32>,
    pub by_kind: BTreeMap<ElementKind, KindCounts>,
    pub elements: Vec<ChangedElement>,
    pub top_elements: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeDetail {
    pub path: String,
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub language: Language,
    pub insertions: u32,
    pub deletions: u32,
    pub elements: Vec<ChangedElement>,
    pub raw_hunks: Vec<RawHunk>,
    pub is_binary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TagSeverity {
    High,
    Medium,
    Low,
    Info,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum PatternKind {
    Regex,
    FancyRegex,
    Literal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityTagDefinition {
    pub tag: String,
    pub patterns: Vec<String>,
    pub negative_patterns: Vec<String>,
    pub description: String,
    pub severity: TagSeverity,
    pub min_matches: u32,
    #[serde(default)]
    pub pattern_kind: Option<PatternKind>,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub false_positive_note: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfidenceLevel {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighAttentionItem {
    pub reason: String,
    pub element_name: String,
    pub element_kind: ElementKind,
    pub change_type: ChangeType,
    pub file_path: String,
    pub tags: Vec<String>,
    pub code_preview: String,
    pub snippet_ref: String,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub confidence_level: Option<ConfidenceLevel>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    Minimal,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RiskScore {
    pub total: f64,
    pub level: Option<RiskLevel>,
    pub severity_score: f64,
    pub concentration_factor: f64,
    pub composition_bonus: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SecurityReview {
    pub total_security_tagged_elements: u32,
    pub by_tag: BTreeMap<String, u32>,
    pub by_severity: BTreeMap<TagSeverity, u32>,
    pub high_attention_items: Vec<HighAttentionItem>,
    pub flagged_elements: Vec<ChangedElement>,
    pub risk_score: Option<RiskScore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffResult {
    pub label: String,
    pub from_commit: CommitInfo,
    pub to_commit: CommitInfo,
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
    pub file_changes: Vec<FileChangeDetail>,
    pub element_summary: Option<ElementSummary>,
    pub security_review: Option<SecurityReview>,
    pub patch_filename: String,
    pub changes_filename: String,
    pub summary_json_filename: Option<String>,
    pub summary_txt_filename: Option<String>,
    pub summary_md_filename: Option<String>,
    pub snippets_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoResult {
    pub repo_path: PathBuf,
    pub repo_name: String,
    pub report_folder_name: String,
    pub branch: String,
    pub status: RepoStatus,
    pub pre_pull: Option<CommitInfo>,
    pub post_pull: Option<CommitInfo>,
    pub diffs: Vec<DiffResult>,
    pub pull_log: String,
    pub errors: Vec<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSecuritySummary {
    pub name: String,
    pub security_elements: u32,
    pub tags: Vec<String>,
    pub detail_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalHighAttentionItem {
    pub repo: String,
    pub reason: String,
    pub element_name: String,
    pub file_path: String,
    pub tags: Vec<String>,
    pub before_code_preview: Option<String>,
    pub after_code_preview: Option<String>,
    pub commit_from: String,
    pub commit_to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSecurityOverview {
    pub timestamp: DateTime<Utc>,
    pub total_repos_scanned: u32,
    pub repos_with_security_flags: u32,
    pub total_security_tagged_elements: u32,
    pub by_tag_global: BTreeMap<String, u32>,
    pub by_severity: BTreeMap<TagSeverity, u32>,
    pub high_attention_items: Vec<GlobalHighAttentionItem>,
    pub repos: Vec<RepoSecuritySummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoTopElement {
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSummaryEntry {
    pub name: String,
    pub path: String,
    pub status: RepoStatus,
    pub branch: String,
    pub latest_diff: Option<RepoLatestDiffSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoLatestDiffSummary {
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
    pub elements_added: u32,
    pub elements_modified: u32,
    pub elements_removed: u32,
    pub security_tagged: u32,
    pub top_elements: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSummary {
    pub scan_root: PathBuf,
    pub report_dir: PathBuf,
    pub timestamp: DateTime<Utc>,
    pub total_repos_found: u32,
    pub updated: u32,
    pub up_to_date: u32,
    pub dirty_skipped: u32,
    pub fetch_failed: u32,
    pub pull_failed: u32,
    pub skipped: u32,
    pub total_elements_changed_across_all_repos: u32,
    pub total_security_tagged_elements: u32,
    pub repos: Vec<RepoSummaryEntry>,
}

impl GlobalSummary {
    pub fn from_results(scan_root: PathBuf, report_dir: PathBuf, repos: &[RepoResult]) -> Self {
        let mut summary = Self {
            scan_root,
            report_dir,
            timestamp: Utc::now(),
            total_repos_found: repos.len() as u32,
            updated: 0,
            up_to_date: 0,
            dirty_skipped: 0,
            fetch_failed: 0,
            pull_failed: 0,
            skipped: 0,
            total_elements_changed_across_all_repos: 0,
            total_security_tagged_elements: 0,
            repos: Vec::with_capacity(repos.len()),
        };

        for repo in repos {
            match &repo.status {
                RepoStatus::Updated => summary.updated += 1,
                RepoStatus::UpToDate => summary.up_to_date += 1,
                RepoStatus::DirtySkipped => summary.dirty_skipped += 1,
                RepoStatus::FetchFailed { .. } => summary.fetch_failed += 1,
                RepoStatus::PullFailed { .. } => summary.pull_failed += 1,
                RepoStatus::Skipped { .. } => summary.skipped += 1,
            }

            for diff in &repo.diffs {
                if let Some(es) = &diff.element_summary {
                    summary.total_elements_changed_across_all_repos += es.total_elements;
                }
                if let Some(sr) = &diff.security_review {
                    summary.total_security_tagged_elements += sr.total_security_tagged_elements;
                }
            }

            let latest = repo.diffs.first().map(|diff| {
                let mut added = 0;
                let mut modified = 0;
                let mut removed = 0;
                if let Some(es) = &diff.element_summary {
                    added = *es.by_change_type.get(&ChangeType::Added).unwrap_or(&0);
                    modified = *es.by_change_type.get(&ChangeType::Modified).unwrap_or(&0);
                    removed = *es.by_change_type.get(&ChangeType::Removed).unwrap_or(&0);
                }
                let sec = diff
                    .security_review
                    .as_ref()
                    .map(|sr| sr.total_security_tagged_elements)
                    .unwrap_or(0);

                RepoLatestDiffSummary {
                    files_changed: diff.files_changed,
                    insertions: diff.insertions,
                    deletions: diff.deletions,
                    elements_added: added,
                    elements_modified: modified,
                    elements_removed: removed,
                    security_tagged: sec,
                    top_elements: diff
                        .element_summary
                        .as_ref()
                        .map(|es| es.top_elements.clone())
                        .unwrap_or_default(),
                }
            });

            summary.repos.push(RepoSummaryEntry {
                name: repo.repo_name.clone(),
                path: repo.repo_path.display().to_string(),
                status: repo.status.clone(),
                branch: repo.branch.clone(),
                latest_diff: latest,
            });
        }

        summary
    }
}

pub fn unique_tags(elements: &[ChangedElement]) -> Vec<String> {
    let mut set = BTreeSet::new();
    for element in elements {
        for tag in &element.security_tags {
            set.insert(tag.clone());
        }
    }
    set.into_iter().collect()
}
