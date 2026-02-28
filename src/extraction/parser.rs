//! Unified diff parser for extracting file changes and hunks.
//!
//! This module parses Git unified diff output (from `git diff`) into structured
//! data for element extraction and analysis. It handles:
//! - File headers (diff --git, +++, ---)
//! - Hunk headers (@@ -old_start,old_count +new_start,new_count @@)
//! - Binary file detection
//! - Rename operations
//! - Insertion/deletion counting

use once_cell::sync::Lazy;
use regex::Regex;

use crate::types::RawHunk;

/// Represents a single file's changes within a diff.
///
/// Contains all hunks for the file, along with metadata like paths,
/// insertion/deletion counts, and binary file status.
#[derive(Debug, Clone)]
pub struct ParsedFileDiff {
    /// Original file path (None for new files)
    pub old_path: Option<String>,
    /// New file path
    pub new_path: String,
    /// All hunks (change blocks) in this file
    pub hunks: Vec<RawHunk>,
    /// Total number of inserted lines
    pub insertions: u32,
    /// Total number of deleted lines
    pub deletions: u32,
    /// Whether this is a binary file (no text diff available)
    pub is_binary: bool,
}

/// Complete parsed diff containing all file changes.
#[derive(Debug, Clone, Default)]
pub struct ParsedDiff {
    /// All files modified in this diff
    pub files: Vec<ParsedFileDiff>,
}

/// Regex for parsing hunk headers: @@ -old_start,old_count +new_start,new_count @@ context
static HUNK_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^@@\s+-(\d+)(?:,(\d+))?\s+\+(\d+)(?:,(\d+))?\s+@@\s*(.*)$").expect("valid regex")
});

/// Parses a unified diff string into structured data.
///
/// # Arguments
/// * `input` - Raw unified diff output from `git diff`
///
/// # Returns
/// A `ParsedDiff` containing all file changes with hunks, line counts, and metadata.
///
/// # Example
/// ```ignore
/// let diff_output = "diff --git a/file.rs b/file.rs\n...";
/// let parsed = parse_unified_diff(diff_output);
/// for file in parsed.files {
///     println!(
///         "File: {} (+{} -{})",
///         file.new_path, file.insertions, file.deletions
///     );
/// }
/// ```
pub fn parse_unified_diff(input: &str) -> ParsedDiff {
    let hunk_re = &*HUNK_RE;

    let mut parsed = ParsedDiff::default();
    let mut current_file: Option<ParsedFileDiff> = None;
    let mut current_hunk: Option<RawHunk> = None;

    for line in input.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            if let Some(mut file) = current_file.take() {
                if let Some(hunk) = current_hunk.take() {
                    file.hunks.push(hunk);
                }
                parsed.files.push(file);
            }

            let mut parts = rest.split_whitespace();
            let a = parts.next().unwrap_or("a/");
            let b = parts.next().unwrap_or("b/");
            let old_path = a.strip_prefix("a/").unwrap_or(a).to_string();
            let new_path = b.strip_prefix("b/").unwrap_or(b).to_string();
            current_file = Some(ParsedFileDiff {
                old_path: Some(old_path),
                new_path,
                hunks: Vec::new(),
                insertions: 0,
                deletions: 0,
                is_binary: false,
            });
            continue;
        }

        if let Some(file) = &mut current_file {
            if line.starts_with("Binary files ") {
                file.is_binary = true;
                continue;
            }

            if let Some(raw_new_path) = line.strip_prefix("+++ ") {
                let new_path = raw_new_path
                    .split('\t')
                    .next()
                    .unwrap_or(raw_new_path)
                    .trim();
                if new_path != "/dev/null" {
                    file.new_path = new_path.strip_prefix("b/").unwrap_or(new_path).to_string();
                }
                continue;
            }
            if let Some(raw_old_path) = line.strip_prefix("--- ") {
                let old_path = raw_old_path
                    .split('\t')
                    .next()
                    .unwrap_or(raw_old_path)
                    .trim();
                if old_path == "/dev/null" {
                    file.old_path = None;
                } else {
                    file.old_path =
                        Some(old_path.strip_prefix("a/").unwrap_or(old_path).to_string());
                }
                continue;
            }
            if let Some(rename_from) = line.strip_prefix("rename from ") {
                file.old_path = Some(rename_from.to_string());
                continue;
            }
            if let Some(rename_to) = line.strip_prefix("rename to ") {
                file.new_path = rename_to.to_string();
                continue;
            }

            if let Some(caps) = hunk_re.captures(line) {
                if let Some(hunk) = current_hunk.take() {
                    file.hunks.push(hunk);
                }

                let old_start = caps
                    .get(1)
                    .and_then(|m| m.as_str().parse::<u32>().ok())
                    .unwrap_or(0);
                let old_count = caps
                    .get(2)
                    .and_then(|m| m.as_str().parse::<u32>().ok())
                    .unwrap_or(1);
                let new_start = caps
                    .get(3)
                    .and_then(|m| m.as_str().parse::<u32>().ok())
                    .unwrap_or(0);
                let new_count = caps
                    .get(4)
                    .and_then(|m| m.as_str().parse::<u32>().ok())
                    .unwrap_or(1);
                let context_function = caps
                    .get(5)
                    .map(|m| m.as_str().trim().to_string())
                    .filter(|s| !s.is_empty());

                current_hunk = Some(RawHunk {
                    header: line.to_string(),
                    old_start,
                    old_count,
                    new_start,
                    new_count,
                    context_function,
                    lines: String::new(),
                });
                continue;
            }

            if let Some(hunk) = &mut current_hunk {
                if line.starts_with('+') && !line.starts_with("+++") {
                    file.insertions += 1;
                } else if line.starts_with('-') && !line.starts_with("---") {
                    file.deletions += 1;
                }

                hunk.lines.push_str(line);
                hunk.lines.push('\n');
            }
        }
    }

    if let Some(mut file) = current_file {
        if let Some(hunk) = current_hunk {
            file.hunks.push(hunk);
        }
        parsed.files.push(file);
    }

    parsed
}
