use std::collections::BTreeMap;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::types::{ChangeType, ElementKind, RawHunk};

#[derive(Debug, Clone)]
pub struct DetectedElement {
    pub kind: ElementKind,
    pub name: String,
    pub change_type: ChangeType,
    pub line_range: Option<(u32, u32)>,
    pub lines_added: u32,
    pub lines_removed: u32,
    pub enclosing_context: Option<String>,
    pub signature: Option<String>,
}

#[derive(Debug)]
struct ElementPattern {
    kind: ElementKind,
    regex: Regex,
}

static ELEMENT_PATTERNS: Lazy<Vec<ElementPattern>> = Lazy::new(|| {
    vec![
        ElementPattern {
            kind: ElementKind::Function,
            regex: Regex::new(r"(?i)^\s*(?:pub\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap(),
        },
        ElementPattern {
            kind: ElementKind::Function,
            regex: Regex::new(r"(?i)^\s*def\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap(),
        },
        ElementPattern {
            kind: ElementKind::Function,
            regex: Regex::new(r"(?i)^\s*function\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap(),
        },
        ElementPattern {
            kind: ElementKind::Function,
            regex: Regex::new(r"(?i)^\s*func\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap(),
        },
        ElementPattern {
            kind: ElementKind::Struct,
            regex: Regex::new(r"(?i)^\s*struct\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap(),
        },
        ElementPattern {
            kind: ElementKind::Class,
            regex: Regex::new(r"(?i)^\s*class\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap(),
        },
        ElementPattern {
            kind: ElementKind::Enum,
            regex: Regex::new(r"(?i)^\s*enum(?:\s+class)?\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap(),
        },
        ElementPattern {
            kind: ElementKind::Trait,
            regex: Regex::new(r"(?i)^\s*trait\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap(),
        },
        ElementPattern {
            kind: ElementKind::Interface,
            regex: Regex::new(r"(?i)^\s*interface\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap(),
        },
        ElementPattern {
            kind: ElementKind::Impl,
            regex: Regex::new(r"(?i)^\s*impl\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap(),
        },
        ElementPattern {
            kind: ElementKind::TypeAlias,
            regex: Regex::new(r"(?i)^\s*type\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap(),
        },
        ElementPattern {
            kind: ElementKind::Import,
            regex: Regex::new(r"(?i)^\s*(?:use|import|from\s+.+\s+import|#include)\s+(.+)").unwrap(),
        },
        ElementPattern {
            kind: ElementKind::Module,
            regex: Regex::new(r"(?i)^\s*(?:mod|module\.exports|package)\s+([A-Za-z_][A-Za-z0-9_\-\.]*)").unwrap(),
        },
        ElementPattern {
            kind: ElementKind::Constant,
            regex: Regex::new(r"(?i)^\s*(?:pub\s+)?const\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap(),
        },
        ElementPattern {
            kind: ElementKind::Static,
            regex: Regex::new(r"(?i)^\s*(?:pub\s+)?static\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap(),
        },
        ElementPattern {
            kind: ElementKind::Macro,
            regex: Regex::new(r"(?i)^\s*(?:macro_rules!\s*([A-Za-z_][A-Za-z0-9_]*)|#define\s+([A-Za-z_][A-Za-z0-9_]*))").unwrap(),
        },
        ElementPattern {
            kind: ElementKind::Test,
            regex: Regex::new(r"(?i)(#\[test\]|#\[cfg\(test\)\]|\bdescribe\(|\bit\(|\btest_[A-Za-z_0-9]+)").unwrap(),
        },
    ]
});

#[derive(Debug)]
struct Tracker {
    plus: bool,
    minus: bool,
    lines_added: u32,
    lines_removed: u32,
    signature: Option<String>,
    context: Option<String>,
    line_range: Option<(u32, u32)>,
    kind: ElementKind,
    name: String,
}

impl Default for Tracker {
    fn default() -> Self {
        Self {
            plus: false,
            minus: false,
            lines_added: 0,
            lines_removed: 0,
            signature: None,
            context: None,
            line_range: None,
            kind: ElementKind::Other,
            name: String::new(),
        }
    }
}

pub fn detect_elements(
    file_path: &str,
    hunks: &[RawHunk],
    max_elements: usize,
) -> Vec<DetectedElement> {
    let mut trackers: BTreeMap<(ElementKind, String), Tracker> = BTreeMap::new();

    for hunk in hunks {
        let mut saw_specific = false;
        for line in hunk.lines.lines() {
            let (change, content) = if let Some(rest) = line.strip_prefix('+') {
                (Some(ChangeType::Added), rest)
            } else if let Some(rest) = line.strip_prefix('-') {
                (Some(ChangeType::Removed), rest)
            } else {
                (None, line)
            };

            for pattern in ELEMENT_PATTERNS.iter() {
                if let Some(caps) = pattern.regex.captures(content) {
                    saw_specific = true;
                    let mut name = caps
                        .get(1)
                        .map(|m| m.as_str().trim().to_string())
                        .unwrap_or_default();
                    if name.is_empty() {
                        name = caps
                            .get(2)
                            .map(|m| m.as_str().trim().to_string())
                            .unwrap_or_else(|| "anonymous".to_string());
                    }
                    if name.len() > 120 {
                        name.truncate(120);
                    }
                    let key = (pattern.kind, name.clone());
                    let entry = trackers.entry(key).or_insert_with(|| Tracker {
                        kind: pattern.kind,
                        name: name.clone(),
                        context: hunk.context_function.clone(),
                        line_range: Some((
                            hunk.new_start,
                            hunk.new_start + hunk.new_count.saturating_sub(1),
                        )),
                        ..Tracker::default()
                    });

                    entry
                        .signature
                        .get_or_insert_with(|| content.trim().to_string());
                    if change == Some(ChangeType::Added) {
                        entry.plus = true;
                        entry.lines_added += 1;
                    } else if change == Some(ChangeType::Removed) {
                        entry.minus = true;
                        entry.lines_removed += 1;
                    }
                }
            }
        }

        if !saw_specific {
            let name = hunk
                .context_function
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| format!("body_changes_hunk_{}", hunk.new_start));
            let key = (ElementKind::Other, name.clone());
            let entry = trackers.entry(key).or_insert_with(|| Tracker {
                kind: ElementKind::Other,
                name: name.clone(),
                context: hunk.context_function.clone(),
                line_range: Some((
                    hunk.new_start,
                    hunk.new_start + hunk.new_count.saturating_sub(1),
                )),
                ..Tracker::default()
            });

            for line in hunk.lines.lines() {
                if line.starts_with('+') {
                    entry.plus = true;
                    entry.lines_added += 1;
                } else if line.starts_with('-') {
                    entry.minus = true;
                    entry.lines_removed += 1;
                }
            }
        }

        if trackers.len() >= max_elements {
            break;
        }
    }

    let mut elements = Vec::new();
    for (_, tr) in trackers.into_iter().take(max_elements) {
        let change_type = match (tr.plus, tr.minus) {
            (true, false) => ChangeType::Added,
            (false, true) => ChangeType::Removed,
            _ => ChangeType::Modified,
        };

        elements.push(DetectedElement {
            kind: tr.kind,
            name: tr.name,
            change_type,
            line_range: tr.line_range,
            lines_added: tr.lines_added,
            lines_removed: tr.lines_removed,
            enclosing_context: tr.context,
            signature: tr.signature,
        });
    }

    if elements.is_empty() {
        elements.push(DetectedElement {
            kind: ElementKind::Other,
            name: format!("body_changes_in_{}", file_path),
            change_type: ChangeType::Modified,
            line_range: None,
            lines_added: 0,
            lines_removed: 0,
            enclosing_context: None,
            signature: None,
        });
    }

    elements
}
