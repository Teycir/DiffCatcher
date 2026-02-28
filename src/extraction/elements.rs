use std::collections::BTreeMap;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::types::{ChangeType, ElementKind, Language, RawHunk};

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
pub struct ElementPattern {
    pub kind: ElementKind,
    pub regex: Regex,
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

static RUST_PATTERNS: Lazy<Vec<ElementPattern>> = Lazy::new(|| {
    vec![
        ElementPattern { kind: ElementKind::Function, regex: Regex::new(r"^\s*(?:pub(?:\(crate\))?\s+)?(?:async\s+)?(?:unsafe\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Struct, regex: Regex::new(r"^\s*(?:pub(?:\(crate\))?\s+)?struct\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Enum, regex: Regex::new(r"^\s*(?:pub(?:\(crate\))?\s+)?enum\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Trait, regex: Regex::new(r"^\s*(?:pub(?:\(crate\))?\s+)?(?:unsafe\s+)?trait\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Impl, regex: Regex::new(r"^\s*(?:unsafe\s+)?impl(?:<[^>]*>)?\s+(?:([A-Za-z_][A-Za-z0-9_:]*)\s+for\s+)?([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::TypeAlias, regex: Regex::new(r"^\s*(?:pub(?:\(crate\))?\s+)?type\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Macro, regex: Regex::new(r"^\s*macro_rules!\s*([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Constant, regex: Regex::new(r"^\s*(?:pub(?:\(crate\))?\s+)?const\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Static, regex: Regex::new(r"^\s*(?:pub(?:\(crate\))?\s+)?static\s+(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Module, regex: Regex::new(r"^\s*(?:pub(?:\(crate\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Import, regex: Regex::new(r"^\s*use\s+(.+)").unwrap() },
        ElementPattern { kind: ElementKind::Test, regex: Regex::new(r"#\[(?:test|cfg\(test\))\]").unwrap() },
    ]
});

static PYTHON_PATTERNS: Lazy<Vec<ElementPattern>> = Lazy::new(|| {
    vec![
        ElementPattern { kind: ElementKind::Function, regex: Regex::new(r"^\s*(?:async\s+)?def\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Class, regex: Regex::new(r"^\s*class\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Import, regex: Regex::new(r"^\s*(?:from\s+\S+\s+)?import\s+(.+)").unwrap() },
        ElementPattern { kind: ElementKind::Constant, regex: Regex::new(r"^([A-Z][A-Z0-9_]+)\s*=").unwrap() },
        ElementPattern { kind: ElementKind::Test, regex: Regex::new(r"^\s*def\s+(test_[A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Config, regex: Regex::new(r"^\s*@(\w+)").unwrap() },
    ]
});

static JAVASCRIPT_PATTERNS: Lazy<Vec<ElementPattern>> = Lazy::new(|| {
    vec![
        ElementPattern { kind: ElementKind::Function, regex: Regex::new(r"^\s*(?:export\s+)?(?:async\s+)?function\s+([A-Za-z_$][A-Za-z0-9_$]*)").unwrap() },
        ElementPattern { kind: ElementKind::Function, regex: Regex::new(r"^\s*(?:export\s+)?(?:const|let|var)\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*=\s*(?:async\s+)?\(").unwrap() },
        ElementPattern { kind: ElementKind::Function, regex: Regex::new(r"^\s*(?:export\s+)?(?:const|let|var)\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*=\s*(?:async\s+)?(?:\([^)]*\)|[A-Za-z_$][A-Za-z0-9_$]*)\s*=>").unwrap() },
        ElementPattern { kind: ElementKind::Class, regex: Regex::new(r"^\s*(?:export\s+)?class\s+([A-Za-z_$][A-Za-z0-9_$]*)").unwrap() },
        ElementPattern { kind: ElementKind::Import, regex: Regex::new(r"^\s*import\s+(.+)").unwrap() },
        ElementPattern { kind: ElementKind::Import, regex: Regex::new(r"^\s*(?:const|let|var)\s+.+=\s*require\(").unwrap() },
        ElementPattern { kind: ElementKind::Module, regex: Regex::new(r"^\s*module\.exports\s*=").unwrap() },
        ElementPattern { kind: ElementKind::Constant, regex: Regex::new(r"^\s*(?:export\s+)?const\s+([A-Z][A-Z0-9_]+)\s*=").unwrap() },
        ElementPattern { kind: ElementKind::Interface, regex: Regex::new(r"^\s*(?:export\s+)?interface\s+([A-Za-z_$][A-Za-z0-9_$]*)").unwrap() },
        ElementPattern { kind: ElementKind::TypeAlias, regex: Regex::new(r"^\s*(?:export\s+)?type\s+([A-Za-z_$][A-Za-z0-9_$]*)").unwrap() },
        ElementPattern { kind: ElementKind::Enum, regex: Regex::new(r"^\s*(?:export\s+)?enum\s+([A-Za-z_$][A-Za-z0-9_$]*)").unwrap() },
        ElementPattern { kind: ElementKind::Test, regex: Regex::new(r"^\s*(?:describe|it|test)\s*\(").unwrap() },
    ]
});

static GO_PATTERNS: Lazy<Vec<ElementPattern>> = Lazy::new(|| {
    vec![
        ElementPattern { kind: ElementKind::Function, regex: Regex::new(r"^\s*func\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Method, regex: Regex::new(r"^\s*func\s+\([^)]+\)\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Struct, regex: Regex::new(r"^\s*type\s+([A-Za-z_][A-Za-z0-9_]*)\s+struct\b").unwrap() },
        ElementPattern { kind: ElementKind::Interface, regex: Regex::new(r"^\s*type\s+([A-Za-z_][A-Za-z0-9_]*)\s+interface\b").unwrap() },
        ElementPattern { kind: ElementKind::TypeAlias, regex: Regex::new(r"^\s*type\s+([A-Za-z_][A-Za-z0-9_]*)\s+[^si]").unwrap() },
        ElementPattern { kind: ElementKind::Import, regex: Regex::new(r"^\s*import\s+(.+)").unwrap() },
        ElementPattern { kind: ElementKind::Constant, regex: Regex::new(r"^\s*const\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Static, regex: Regex::new(r"^\s*var\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Module, regex: Regex::new(r"^\s*package\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Test, regex: Regex::new(r"^\s*func\s+(Test[A-Za-z0-9_]*)").unwrap() },
    ]
});

static JAVA_KOTLIN_PATTERNS: Lazy<Vec<ElementPattern>> = Lazy::new(|| {
    vec![
        ElementPattern { kind: ElementKind::Class, regex: Regex::new(r"^\s*(?:public|private|protected)?\s*(?:abstract|final|sealed)?\s*(?:data\s+)?class\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Interface, regex: Regex::new(r"^\s*(?:public|private|protected)?\s*interface\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Enum, regex: Regex::new(r"^\s*(?:public|private|protected)?\s*enum\s+(?:class\s+)?([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Function, regex: Regex::new(r"^\s*(?:public|private|protected)?\s*(?:static\s+)?(?:final\s+)?(?:suspend\s+)?(?:fun|void|int|long|String|boolean|double|float|[A-Z][A-Za-z0-9_<>,\s]*)\s+([a-z_][A-Za-z0-9_]*)\s*\(").unwrap() },
        ElementPattern { kind: ElementKind::Function, regex: Regex::new(r"^\s*(?:(?:public|private|protected|internal)\s+)?(?:suspend\s+)?fun\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Import, regex: Regex::new(r"^\s*import\s+(.+)").unwrap() },
        ElementPattern { kind: ElementKind::Module, regex: Regex::new(r"^\s*package\s+(.+)").unwrap() },
        ElementPattern { kind: ElementKind::Constant, regex: Regex::new(r"^\s*(?:public|private|protected)?\s*(?:static\s+)?(?:final\s+)?(?:val|const)\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Config, regex: Regex::new(r"^\s*@([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Test, regex: Regex::new(r"@Test|@ParameterizedTest|@RepeatedTest").unwrap() },
    ]
});

static RUBY_PATTERNS: Lazy<Vec<ElementPattern>> = Lazy::new(|| {
    vec![
        ElementPattern { kind: ElementKind::Function, regex: Regex::new(r"^\s*def\s+(?:self\.)?([A-Za-z_][A-Za-z0-9_!?]*)").unwrap() },
        ElementPattern { kind: ElementKind::Class, regex: Regex::new(r"^\s*class\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Module, regex: Regex::new(r"^\s*module\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Import, regex: Regex::new(r"^\s*require(?:_relative)?\s+(.+)").unwrap() },
        ElementPattern { kind: ElementKind::Import, regex: Regex::new(r"^\s*include\s+([A-Za-z_][A-Za-z0-9_:]*)").unwrap() },
        ElementPattern { kind: ElementKind::Config, regex: Regex::new(r"^\s*attr_(?:accessor|reader|writer)\s+(.+)").unwrap() },
        ElementPattern { kind: ElementKind::Constant, regex: Regex::new(r"^\s*([A-Z][A-Z0-9_]+)\s*=").unwrap() },
        ElementPattern { kind: ElementKind::Test, regex: Regex::new(r#"^\s*(?:def\s+test_|it\s+['"]|describe\s+['"])"#).unwrap() },
    ]
});

static C_CPP_PATTERNS: Lazy<Vec<ElementPattern>> = Lazy::new(|| {
    vec![
        ElementPattern { kind: ElementKind::Function, regex: Regex::new(r"^\s*(?:static\s+)?(?:inline\s+)?(?:virtual\s+)?(?:const\s+)?(?:unsigned\s+)?(?:void|int|long|char|float|double|bool|auto|[A-Z][A-Za-z0-9_]*(?:::[A-Za-z0-9_]*)*[*&\s]*)\s+([A-Za-z_][A-Za-z0-9_:]*)\s*\(").unwrap() },
        ElementPattern { kind: ElementKind::Struct, regex: Regex::new(r"^\s*(?:typedef\s+)?struct\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Class, regex: Regex::new(r"^\s*class\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Enum, regex: Regex::new(r"^\s*enum\s+(?:class\s+)?([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Import, regex: Regex::new(r"^\s*#include\s+(.+)").unwrap() },
        ElementPattern { kind: ElementKind::Macro, regex: Regex::new(r"^\s*#define\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::TypeAlias, regex: Regex::new(r"^\s*(?:typedef|using)\s+.+\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Module, regex: Regex::new(r"^\s*namespace\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Constant, regex: Regex::new(r"^\s*(?:static\s+)?(?:const(?:expr)?\s+)([A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z0-9_]*)*)").unwrap() },
    ]
});

static SHELL_PATTERNS: Lazy<Vec<ElementPattern>> = Lazy::new(|| {
    vec![
        ElementPattern { kind: ElementKind::Function, regex: Regex::new(r"^\s*(?:function\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*\(\)").unwrap() },
        ElementPattern { kind: ElementKind::Function, regex: Regex::new(r"^\s*function\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap() },
        ElementPattern { kind: ElementKind::Constant, regex: Regex::new(r"^\s*(?:export\s+)?([A-Z][A-Z0-9_]+)=").unwrap() },
        ElementPattern { kind: ElementKind::Import, regex: Regex::new(r"^\s*(?:source|\\.)\s+(.+)").unwrap() },
    ]
});

fn select_patterns(lang: &Language) -> &[ElementPattern] {
    match lang {
        Language::Rust => &RUST_PATTERNS,
        Language::Python => &PYTHON_PATTERNS,
        Language::JavaScript | Language::TypeScript => &JAVASCRIPT_PATTERNS,
        Language::Go => &GO_PATTERNS,
        Language::Java | Language::Kotlin => &JAVA_KOTLIN_PATTERNS,
        Language::Ruby => &RUBY_PATTERNS,
        Language::C | Language::Cpp => &C_CPP_PATTERNS,
        Language::Shell => &SHELL_PATTERNS,
        _ => &ELEMENT_PATTERNS,
    }
}

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
    lang: &Language,
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

            let patterns = select_patterns(lang);
            for pattern in patterns.iter() {
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
