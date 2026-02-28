use once_cell::sync::Lazy;
use regex::Regex;

use crate::types::RawHunk;

#[derive(Debug, Clone)]
pub struct ParsedFileDiff {
    pub old_path: Option<String>,
    pub new_path: String,
    pub hunks: Vec<RawHunk>,
    pub insertions: u32,
    pub deletions: u32,
    pub is_binary: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedDiff {
    pub files: Vec<ParsedFileDiff>,
}

static HUNK_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^@@\s+-(\d+)(?:,(\d+))?\s+\+(\d+)(?:,(\d+))?\s+@@\s*(.*)$").expect("valid regex")
});

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

            if let Some(new_file_path) = line.strip_prefix("+++ b/") {
                file.new_path = new_file_path.to_string();
                continue;
            }
            if let Some(old_file_path) = line.strip_prefix("--- a/") {
                file.old_path = Some(old_file_path.to_string());
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
