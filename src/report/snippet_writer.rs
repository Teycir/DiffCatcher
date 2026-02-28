use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::types::{ChangeType, ChangedElement, SnippetFileRefs};

pub fn write_snippets(
    snippets_dir: &Path,
    file_extension_hint: &str,
    elements: &mut [ChangedElement],
    sequence_start: usize,
) -> Result<usize> {
    fs::create_dir_all(snippets_dir)?;
    let mut seq_counter = sequence_start;

    for element in elements.iter_mut() {
        let seq = format!("{:03}", seq_counter);
        seq_counter += 1;
        let safe_name = sanitize_name(&element.name);
        let ext = normalize_extension(file_extension_hint);

        let mut refs = SnippetFileRefs {
            before: None,
            after: None,
            diff: None,
        };

        if let Some(before) = &element.snippet.before {
            let suffix = if element.change_type == ChangeType::Removed {
                "REMOVED"
            } else {
                "BEFORE"
            };
            let file_name = format!("{}_{}_{}.{}", seq, safe_name, suffix, ext);
            let rel = PathBuf::from("snippets").join(&file_name);
            fs::write(snippets_dir.join(&file_name), before.code.as_bytes())?;
            refs.before = Some(rel.display().to_string());
        }

        if let Some(after) = &element.snippet.after {
            let suffix = if element.change_type == ChangeType::Added {
                "ADDED"
            } else {
                "AFTER"
            };
            let file_name = format!("{}_{}_{}.{}", seq, safe_name, suffix, ext);
            let rel = PathBuf::from("snippets").join(&file_name);
            fs::write(snippets_dir.join(&file_name), after.code.as_bytes())?;
            refs.after = Some(rel.display().to_string());
        }

        let diff_name = format!("{}_{}.diff", seq, safe_name);
        let rel = PathBuf::from("snippets").join(&diff_name);
        fs::write(
            snippets_dir.join(&diff_name),
            element.snippet.diff_lines.as_bytes(),
        )?;
        refs.diff = Some(rel.display().to_string());

        element.snippet_files = Some(refs);
    }

    Ok(seq_counter)
}

fn sanitize_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "element".to_string()
    } else {
        out
    }
}

fn normalize_extension(path: &str) -> String {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("txt");
    if ext.is_empty() {
        "txt".to_string()
    } else {
        ext.to_string()
    }
}
