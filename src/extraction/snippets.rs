use crate::extraction::boundary::{truncate_with_limit, try_capture_full_element};
use crate::extraction::elements::DetectedElement;
use crate::types::RawHunk;
use crate::types::{CaptureScope, ChangeType, CodeSnippet, SnippetContent};

#[derive(Debug, Clone)]
pub struct SnippetOptions {
    pub context_lines: u32,
    pub max_snippet_lines: u32,
    pub no_snippets: bool,
}

pub fn build_snippet(
    element: &DetectedElement,
    hunks: &[RawHunk],
    from_commit: &str,
    to_commit: &str,
    options: &SnippetOptions,
) -> CodeSnippet {
    let relevant = collect_relevant_hunks(element, hunks);
    let diff_lines = relevant
        .iter()
        .map(|h| h.lines.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    if options.no_snippets {
        return CodeSnippet {
            before: None,
            after: None,
            diff_lines,
            capture_scope: CaptureScope::DiffOnly,
        };
    }

    let (mut before_code, mut after_code) = split_before_after(&relevant);

    if matches!(element.change_type, ChangeType::Modified) {
        if before_code.is_none() && after_code.is_none() {
            let (fallback_before, fallback_after) = split_before_after_from_lines(&diff_lines);
            before_code = fallback_before;
            after_code = fallback_after;
        }
        if before_code.is_none() && after_code.is_none() {
            before_code = Some(String::new());
            after_code = Some(String::new());
        }
        if before_code.is_none() {
            before_code = after_code.clone();
        }
        if after_code.is_none() {
            after_code = before_code.clone();
        }
    }

    let mut capture_scope = CaptureScope::HunkWithContext {
        context_lines: options.context_lines,
    };

    let before = if matches!(element.change_type, ChangeType::Added) {
        None
    } else {
        before_code.map(|code| {
            let full_candidate = try_capture_full_element(&code);
            let candidate = if let Some(full) = full_candidate {
                capture_scope = CaptureScope::FullElement;
                full
            } else {
                code
            };
            let (code, truncated, actual) =
                truncate_with_limit(&candidate, options.max_snippet_lines);
            if truncated {
                capture_scope = CaptureScope::Truncated {
                    actual_lines: actual,
                    max_lines: options.max_snippet_lines,
                };
            }
            SnippetContent {
                code,
                start_line: element.line_range.map(|(s, _)| s).unwrap_or(0),
                end_line: element.line_range.map(|(_, e)| e).unwrap_or(0),
                commit: from_commit.to_string(),
            }
        })
    };

    let after = if matches!(element.change_type, ChangeType::Removed) {
        None
    } else {
        after_code.map(|code| {
            let full_candidate = try_capture_full_element(&code);
            let candidate = if let Some(full) = full_candidate {
                capture_scope = CaptureScope::FullElement;
                full
            } else {
                code
            };
            let (code, truncated, actual) =
                truncate_with_limit(&candidate, options.max_snippet_lines);
            if truncated {
                capture_scope = CaptureScope::Truncated {
                    actual_lines: actual,
                    max_lines: options.max_snippet_lines,
                };
            }
            SnippetContent {
                code,
                start_line: element.line_range.map(|(s, _)| s).unwrap_or(0),
                end_line: element.line_range.map(|(_, e)| e).unwrap_or(0),
                commit: to_commit.to_string(),
            }
        })
    };

    CodeSnippet {
        before,
        after,
        diff_lines,
        capture_scope,
    }
}

fn collect_relevant_hunks<'a>(element: &DetectedElement, hunks: &'a [RawHunk]) -> Vec<&'a RawHunk> {
    let by_context = hunks
        .iter()
        .filter(|h| {
            h.context_function
                .as_ref()
                .is_some_and(|ctx| ctx.contains(&element.name))
        })
        .collect::<Vec<_>>();

    if !by_context.is_empty() {
        return by_context;
    }

    if let Some((start, _)) = element.line_range {
        let by_range = hunks
            .iter()
            .filter(|h| start >= h.new_start && start <= h.new_start.saturating_add(h.new_count))
            .collect::<Vec<_>>();
        if !by_range.is_empty() {
            return by_range;
        }
    }

    hunks.iter().collect()
}

fn split_before_after(hunks: &[&RawHunk]) -> (Option<String>, Option<String>) {
    let mut before = Vec::new();
    let mut after = Vec::new();

    for hunk in hunks {
        for line in hunk.lines.lines() {
            if let Some(rest) = line.strip_prefix('-') {
                before.push(rest.to_string());
            } else if let Some(rest) = line.strip_prefix('+') {
                after.push(rest.to_string());
            } else if let Some(rest) = line.strip_prefix(' ') {
                before.push(rest.to_string());
                after.push(rest.to_string());
            } else {
                // Preserve special diff markers in both views.
                before.push(line.to_string());
                after.push(line.to_string());
            }
        }
    }

    let before = if before.is_empty() {
        None
    } else {
        Some(before.join("\n"))
    };
    let after = if after.is_empty() {
        None
    } else {
        Some(after.join("\n"))
    };

    (before, after)
}

fn split_before_after_from_lines(lines: &str) -> (Option<String>, Option<String>) {
    let mut before = Vec::new();
    let mut after = Vec::new();

    for line in lines.lines() {
        if let Some(rest) = line.strip_prefix('-') {
            before.push(rest.to_string());
        } else if let Some(rest) = line.strip_prefix('+') {
            after.push(rest.to_string());
        } else if let Some(rest) = line.strip_prefix(' ') {
            before.push(rest.to_string());
            after.push(rest.to_string());
        }
    }

    let before = if before.is_empty() {
        None
    } else {
        Some(before.join("\n"))
    };
    let after = if after.is_empty() {
        None
    } else {
        Some(after.join("\n"))
    };

    (before, after)
}
