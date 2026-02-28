use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;

use crate::cli::SummaryFormat;
use crate::error::Result;
use crate::report::json::to_pretty_json;
use crate::report::markdown;
use crate::report::snippet_writer::write_snippets;
use crate::report::text;
use crate::types::{DiffResult, GlobalSecurityOverview, GlobalSummary, RepoResult};

pub fn prepare_report_dir(output: Option<&Path>, overwrite: bool) -> Result<PathBuf> {
    let desired = output
        .map(ToOwned::to_owned)
        .unwrap_or_else(default_report_dir);

    if overwrite {
        if desired.exists() {
            fs::remove_dir_all(&desired)?;
        }
        fs::create_dir_all(&desired)?;
        return Ok(desired);
    }

    if !desired.exists() {
        fs::create_dir_all(&desired)?;
        return Ok(desired);
    }

    let mut idx = 1;
    loop {
        let candidate = PathBuf::from(format!("{}-{}", desired.display(), idx));
        if !candidate.exists() {
            fs::create_dir_all(&candidate)?;
            return Ok(candidate);
        }
        idx += 1;
    }
}

pub fn repo_folder_name(root: &Path, repo_path: &Path) -> String {
    let rel = repo_path
        .strip_prefix(root)
        .ok()
        .and_then(|p| p.to_str())
        .unwrap_or_else(|| {
            repo_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("repo")
        });

    if rel.is_empty() || rel == "." {
        return sanitize_segment(
            repo_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("repo"),
        );
    }

    rel.split('/')
        .filter(|s| !s.is_empty())
        .map(sanitize_segment)
        .collect::<Vec<_>>()
        .join("--")
}

pub fn write_repo_report(
    report_dir: &Path,
    repo: &mut RepoResult,
    summary_formats: &[SummaryFormat],
) -> Result<()> {
    let repo_dir = report_dir.join(&repo.report_folder_name);
    fs::create_dir_all(&repo_dir)?;
    fs::write(repo_dir.join("pull_log.txt"), repo.pull_log.as_bytes())?;

    let diffs_dir = repo_dir.join("diffs");
    fs::create_dir_all(&diffs_dir)?;

    for diff in &mut repo.diffs {
        write_diff_summaries(&diffs_dir, diff, summary_formats)?;
    }

    fs::write(repo_dir.join("status.json"), to_pretty_json(repo)?)?;
    fs::write(repo_dir.join("status.txt"), text::render_repo_status(repo))?;
    fs::write(
        repo_dir.join("status.md"),
        markdown::render_repo_status(repo),
    )?;

    Ok(())
}

pub fn write_top_level_reports(
    report_dir: &Path,
    summary: &GlobalSummary,
    security_overview: Option<&GlobalSecurityOverview>,
) -> Result<()> {
    fs::write(report_dir.join("summary.json"), to_pretty_json(summary)?)?;
    fs::write(
        report_dir.join("summary.txt"),
        text::render_global_summary(summary),
    )?;
    fs::write(
        report_dir.join("summary.md"),
        markdown::render_global_summary(summary),
    )?;

    if let Some(security_overview) = security_overview {
        fs::write(
            report_dir.join("security_overview.json"),
            to_pretty_json(security_overview)?,
        )?;
        fs::write(
            report_dir.join("security_overview.txt"),
            text::render_security_overview(security_overview),
        )?;
        fs::write(
            report_dir.join("security_overview.md"),
            markdown::render_security_overview(security_overview),
        )?;
    }

    Ok(())
}

fn write_diff_summaries(
    diffs_dir: &Path,
    diff: &mut DiffResult,
    summary_formats: &[SummaryFormat],
) -> Result<()> {
    if let Some(summary) = &mut diff.element_summary {
        let should_write_snippets = diff.file_changes.iter().any(|file_change| {
            file_change
                .elements
                .iter()
                .any(|element| element.snippet.before.is_some() || element.snippet.after.is_some())
        });

        if should_write_snippets {
            let snippets_dir = diffs_dir.join("snippets");
            let mut seq = 1_usize;
            for file_change in &mut diff.file_changes {
                seq = write_snippets(
                    &snippets_dir,
                    &file_change.path,
                    &mut file_change.elements,
                    seq,
                )?;
            }
            diff.snippets_dir = Some("diffs/snippets".to_string());
        } else {
            diff.snippets_dir = None;
            for file_change in &mut diff.file_changes {
                for element in &mut file_change.elements {
                    element.snippet_files = None;
                }
            }
        }

        // Rebuild summary element list from file changes so snippet paths are present.
        summary.elements = diff
            .file_changes
            .iter()
            .flat_map(|f| f.elements.clone())
            .collect::<Vec<_>>();
    }

    if let Some(security_review) = &mut diff.security_review {
        security_review.flagged_elements = diff
            .file_changes
            .iter()
            .flat_map(|file| file.elements.iter())
            .filter(|element| !element.security_tags.is_empty())
            .cloned()
            .collect();
    }

    let base_name = format!("summary_{}", diff.label);
    if summary_formats.contains(&SummaryFormat::Json) {
        diff.summary_json_filename = Some(format!("diffs/{}.json", base_name));
    }
    if summary_formats.contains(&SummaryFormat::Txt) {
        diff.summary_txt_filename = Some(format!("diffs/{}.txt", base_name));
    }
    if summary_formats.contains(&SummaryFormat::Md) {
        diff.summary_md_filename = Some(format!("diffs/{}.md", base_name));
    }

    for fmt in summary_formats {
        match fmt {
            SummaryFormat::Json => {
                let file = format!("{}.json", base_name);
                fs::write(diffs_dir.join(&file), to_pretty_json(diff)?)?;
            }
            SummaryFormat::Txt => {
                let file = format!("{}.txt", base_name);
                fs::write(diffs_dir.join(&file), text::render_diff_summary(diff))?;
            }
            SummaryFormat::Md => {
                let file = format!("{}.md", base_name);
                fs::write(diffs_dir.join(&file), markdown::render_diff_summary(diff))?;
            }
            SummaryFormat::Sarif => {
                // SARIF is written at the top level, not per-diff.
            }
        }
    }

    Ok(())
}

fn default_report_dir() -> PathBuf {
    PathBuf::from("reports").join(Utc::now().format("%Y%m%d-%H%M%S").to_string())
}

fn sanitize_segment(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('-');
        }
    }

    if out.is_empty() {
        "repo".to_string()
    } else {
        out
    }
}
