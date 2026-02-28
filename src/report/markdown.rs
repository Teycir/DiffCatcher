use std::fmt::Write;

use crate::types::{DiffResult, GlobalSecurityOverview, GlobalSummary, RepoResult};

pub fn render_repo_status(repo: &RepoResult) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Repo Status: {}", repo.repo_name);
    let _ = writeln!(out, "- Path: `{}`", repo.repo_path.display());
    let _ = writeln!(out, "- Branch: `{}`", repo.branch);
    let _ = writeln!(out, "- Status: `{:?}`", repo.status);
    if let Some(pre) = &repo.pre_pull {
        let _ = writeln!(out, "- Pre: `{}` {}", pre.short_hash, pre.message);
    }
    if let Some(post) = &repo.post_pull {
        let _ = writeln!(out, "- Post: `{}` {}", post.short_hash, post.message);
    }
    out
}

pub fn render_diff_summary(diff: &DiffResult) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Diff {}", diff.label);
    let _ = writeln!(
        out,
        "- From: `{}` {}",
        diff.from_commit.short_hash, diff.from_commit.message
    );
    let _ = writeln!(
        out,
        "- To: `{}` {}",
        diff.to_commit.short_hash, diff.to_commit.message
    );
    let _ = writeln!(out, "- Files changed: {}", diff.files_changed);
    let _ = writeln!(out, "- Insertions: {}", diff.insertions);
    let _ = writeln!(out, "- Deletions: {}", diff.deletions);

    if let Some(es) = &diff.element_summary {
        let _ = writeln!(out, "\n## Elements ({})", es.total_elements);
        for element in &es.elements {
            let tags = if element.security_tags.is_empty() {
                String::new()
            } else {
                format!(" 🔐 {}", element.security_tags.join(", "))
            };
            let _ = writeln!(
                out,
                "- `{:?}` `{:?}` **{}** in `{}`{}",
                element.change_type, element.kind, element.name, element.file_path, tags
            );
            if let Some(refs) = &element.snippet_files {
                if let Some(path) = &refs.before {
                    let _ = writeln!(out, "  - before: `{}`", path);
                }
                if let Some(path) = &refs.after {
                    let _ = writeln!(out, "  - after: `{}`", path);
                }
                if let Some(path) = &refs.diff {
                    let _ = writeln!(out, "  - diff: `{}`", path);
                }
            }
        }
    }

    if let Some(sr) = &diff.security_review {
        let _ = writeln!(
            out,
            "\n## Security Review ({} flagged)",
            sr.total_security_tagged_elements
        );
        for item in &sr.high_attention_items {
            let _ = writeln!(
                out,
                "- **{}**: {} (`{}`)",
                item.reason, item.element_name, item.file_path
            );
        }
    }

    out
}

pub fn render_global_summary(summary: &GlobalSummary) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Git Patrol Report");
    let _ = writeln!(out, "- Root: `{}`", summary.scan_root.display());
    let _ = writeln!(out, "- Report dir: `{}`", summary.report_dir.display());
    let _ = writeln!(out, "- Total repos: {}", summary.total_repos_found);
    let _ = writeln!(out, "- Updated: {}", summary.updated);
    let _ = writeln!(out, "- Up to date: {}", summary.up_to_date);
    let _ = writeln!(out, "- Dirty skipped: {}", summary.dirty_skipped);
    let _ = writeln!(
        out,
        "- Failed (fetch+pull): {}",
        summary.fetch_failed + summary.pull_failed
    );

    let _ = writeln!(out, "\n## Repositories");
    for repo in &summary.repos {
        let _ = writeln!(
            out,
            "- **{}** `{}` - `{:?}`",
            repo.name, repo.branch, repo.status
        );
    }
    out
}

pub fn render_security_overview(overview: &GlobalSecurityOverview) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Security Overview");
    let _ = writeln!(
        out,
        "- Total repos scanned: {}",
        overview.total_repos_scanned
    );
    let _ = writeln!(
        out,
        "- Repos with security flags: {}",
        overview.repos_with_security_flags
    );
    let _ = writeln!(
        out,
        "- Total security-tagged elements: {}",
        overview.total_security_tagged_elements
    );

    let _ = writeln!(out, "\n## High Attention Items");
    for item in &overview.high_attention_items {
        let _ = writeln!(
            out,
            "- [{}] {} - `{}`",
            item.repo, item.reason, item.file_path
        );
    }

    out
}
