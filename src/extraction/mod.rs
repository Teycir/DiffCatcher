use std::collections::BTreeMap;

use classifier::classify_language;
use elements::detect_elements;
use parser::parse_unified_diff;
use rayon::prelude::*;
use snippets::{SnippetOptions, build_snippet};

use crate::git::diff::NameStatusEntry;
use crate::types::{
    ChangeType, ChangedElement, ElementKind, ElementSummary, FileChangeDetail, FileStatus,
    KindCounts,
};

pub mod boundary;
pub mod classifier;
pub mod elements;
pub mod languages;
pub mod parser;
pub mod snippets;

#[derive(Debug, Clone)]
pub struct ExtractionOptions {
    pub no_summary_extraction: bool,
    pub no_snippets: bool,
    pub snippet_context: u32,
    pub max_snippet_lines: u32,
    pub max_elements: usize,
}

pub fn extract_from_patch(
    patch_text: &str,
    name_status: &BTreeMap<String, NameStatusEntry>,
    from_commit: &str,
    to_commit: &str,
    options: &ExtractionOptions,
) -> (Vec<FileChangeDetail>, Option<ElementSummary>) {
    let parsed = parse_unified_diff(patch_text);

    let mut files = parsed
        .files
        .into_par_iter()
        .map(|file| {
            let status_entry = name_status.get(&file.new_path);
            let status = status_entry
                .map(|s| s.status)
                .unwrap_or(FileStatus::Modified);

            let old_path = status_entry
                .and_then(|s| s.old_path.clone())
                .or(file.old_path.clone());

            let language = classify_language(&file.new_path);
            let file_path = file.new_path.clone();
            let is_binary = file.is_binary;
            let hunks = file.hunks;

            let elements = if options.no_summary_extraction || is_binary {
                Vec::new()
            } else {
                let detected = detect_elements(&file_path, &hunks, options.max_elements, &language);
                let snippet_options = SnippetOptions {
                    context_lines: options.snippet_context,
                    max_snippet_lines: options.max_snippet_lines,
                    no_snippets: options.no_snippets,
                };

                detected
                    .into_iter()
                    .map(|detected| {
                        let snippet = build_snippet(
                            &detected,
                            &hunks,
                            from_commit,
                            to_commit,
                            &snippet_options,
                        );

                        let in_test = is_test_path(&file_path);

                        ChangedElement {
                            kind: detected.kind,
                            name: detected.name,
                            change_type: detected.change_type,
                            file_path: file_path.clone(),
                            line_range: detected.line_range,
                            lines_added: detected.lines_added,
                            lines_removed: detected.lines_removed,
                            enclosing_context: detected.enclosing_context,
                            signature: detected.signature,
                            snippet,
                            security_tags: Vec::new(),
                            in_test,
                            snippet_files: None,
                        }
                    })
                    .collect::<Vec<_>>()
            };

            FileChangeDetail {
                path: file_path,
                old_path,
                status,
                language,
                insertions: file.insertions,
                deletions: file.deletions,
                elements,
                raw_hunks: hunks,
                is_binary,
            }
        })
        .collect::<Vec<_>>();

    files.sort_by(|a, b| a.path.cmp(&b.path));
    let all_elements = files
        .iter()
        .flat_map(|file| file.elements.iter().cloned())
        .collect::<Vec<_>>();

    let summary = if options.no_summary_extraction {
        None
    } else {
        Some(build_element_summary(all_elements))
    };

    (files, summary)
}

fn build_element_summary(elements: Vec<ChangedElement>) -> ElementSummary {
    let mut by_change_type = BTreeMap::new();
    let mut by_kind = BTreeMap::new();

    for element in &elements {
        *by_change_type.entry(element.change_type).or_insert(0) += 1;
        let kind_counts = by_kind
            .entry(element.kind)
            .or_insert_with(KindCounts::default);
        match element.change_type {
            ChangeType::Added => kind_counts.added += 1,
            ChangeType::Modified => kind_counts.modified += 1,
            ChangeType::Removed => kind_counts.removed += 1,
        }
    }

    // Rank top elements by impact: lines changed, security tags, and element importance
    let mut scored: Vec<(usize, u32)> = elements
        .iter()
        .enumerate()
        .map(|(idx, e)| {
            let lines_score = e.lines_added + e.lines_removed;
            let security_bonus = if e.security_tags.is_empty() { 0 } else { 50 };
            let kind_bonus = match e.kind {
                ElementKind::Function | ElementKind::Method => 10,
                ElementKind::Struct
                | ElementKind::Class
                | ElementKind::Trait
                | ElementKind::Interface => 8,
                ElementKind::Enum | ElementKind::Impl => 6,
                ElementKind::Constant | ElementKind::Static | ElementKind::TypeAlias => 4,
                ElementKind::Macro => 5,
                ElementKind::Test => 3,
                ElementKind::Module => 2,
                ElementKind::Import | ElementKind::Config => 1,
                ElementKind::Other => 0,
            };
            (idx, lines_score + security_bonus + kind_bonus)
        })
        .collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1));

    let top_elements = scored
        .iter()
        .take(10)
        .map(|(idx, _)| {
            let e = &elements[*idx];
            format!("{:?} {} ({})", e.change_type, e.name, e.file_path)
        })
        .collect::<Vec<_>>();

    let top_elements = if top_elements.is_empty() {
        vec!["No elements extracted".to_string()]
    } else {
        top_elements
    };

    ElementSummary {
        total_elements: elements.len() as u32,
        by_change_type,
        by_kind,
        elements,
        top_elements,
    }
}

fn is_test_path(path: &str) -> bool {
    let lowered = path.to_lowercase();
    lowered.contains("/test")
        || lowered.contains("/tests")
        || lowered.contains("/spec")
        || lowered.contains("__tests__")
        || lowered.ends_with("_test.rs")
        || lowered.ends_with("_test.py")
        || lowered.ends_with("_spec.rb")
}
