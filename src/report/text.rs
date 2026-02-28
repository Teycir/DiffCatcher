use std::fmt::Write;

use crate::types::{DiffResult, GlobalSecurityOverview, GlobalSummary, RepoResult, RepoStatus};

pub fn render_repo_status(repo: &RepoResult) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Repository: {}", repo.repo_path.display());
    let _ = writeln!(out, "Branch:     {}", repo.branch);
    let _ = writeln!(out, "Status:     {}", status_label(&repo.status));
    let _ = writeln!(out);

    if let Some(pre) = &repo.pre_pull {
        let _ = writeln!(out, "Pre:   {} {}", pre.short_hash, pre.message);
    }
    if let Some(post) = &repo.post_pull {
        let _ = writeln!(out, "Post:  {} {}", post.short_hash, post.message);
    }

    if !repo.errors.is_empty() {
        let _ = writeln!(out, "\nErrors:");
        for err in &repo.errors {
            let _ = writeln!(out, "  - {}", err);
        }
    }

    out
}

pub fn render_diff_summary(diff: &DiffResult) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Diff Summary: {}", diff.label);
    let _ = writeln!(
        out,
        "From: {} {}",
        diff.from_commit.short_hash, diff.from_commit.message
    );
    let _ = writeln!(
        out,
        "To:   {} {}",
        diff.to_commit.short_hash, diff.to_commit.message
    );
    let _ = writeln!(
        out,
        "{} files changed, +{} insertions, -{} deletions",
        diff.files_changed, diff.insertions, diff.deletions
    );

    if let Some(es) = &diff.element_summary {
        let _ = writeln!(out, "{} code elements changed", es.total_elements);

        for file in &diff.file_changes {
            let _ = writeln!(
                out,
                "  {:?} {} (+{} -{})",
                file.status, file.path, file.insertions, file.deletions
            );
            for element in &file.elements {
                let tags = if element.security_tags.is_empty() {
                    String::new()
                } else {
                    format!(" 🔐{}", element.security_tags.join(","))
                };
                let _ = writeln!(
                    out,
                    "    - {:?} {:?} {}{}",
                    element.change_type, element.kind, element.name, tags
                );
            }
        }
    }

    if let Some(sr) = &diff.security_review {
        let _ = writeln!(
            out,
            "\nSecurity tagged elements: {}",
            sr.total_security_tagged_elements
        );
        if !sr.high_attention_items.is_empty() {
            let _ = writeln!(out, "High attention:");
            for item in &sr.high_attention_items {
                let _ = writeln!(
                    out,
                    "  - {}: {} ({})",
                    item.reason, item.element_name, item.file_path
                );
            }
        }
    }

    out
}

pub fn render_global_summary(summary: &GlobalSummary) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Git Patrol Report");
    let _ = writeln!(out, "=================");
    let _ = writeln!(out, "Root:     {}", summary.scan_root.display());
    let _ = writeln!(out, "Date:     {}", summary.timestamp);
    let _ = writeln!(out, "Repos:    {} found", summary.total_repos_found);
    let _ = writeln!(
        out,
        "Elements: {} changed across all repos",
        summary.total_elements_changed_across_all_repos
    );
    let _ = writeln!(
        out,
        "Security: {} security-tagged elements",
        summary.total_security_tagged_elements
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "UPDATED ({}):", summary.updated);

    for repo in &summary.repos {
        if !matches!(repo.status, RepoStatus::Updated) {
            continue;
        }
        let _ = writeln!(out, "  ✓ {} {}", repo.name, repo.branch);
    }

    let _ = writeln!(out, "\nUP TO DATE ({}):", summary.up_to_date);
    for repo in &summary.repos {
        if !matches!(repo.status, RepoStatus::UpToDate) {
            continue;
        }
        let _ = writeln!(out, "  — {} {}", repo.name, repo.branch);
    }

    if summary.dirty_skipped > 0 {
        let _ = writeln!(out, "\nSKIPPED — DIRTY ({})", summary.dirty_skipped);
    }
    if summary.fetch_failed > 0 || summary.pull_failed > 0 {
        let _ = writeln!(
            out,
            "\nFAILED ({})",
            summary.fetch_failed + summary.pull_failed
        );
    }

    out
}

pub fn render_security_overview(overview: &GlobalSecurityOverview) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "GIT PATROL - SECURITY OVERVIEW");
    let _ = writeln!(out, "==============================");
    let _ = writeln!(out, "Scanned: {} repos", overview.total_repos_scanned);
    let _ = writeln!(
        out,
        "Repos with security-relevant changes: {}",
        overview.repos_with_security_flags
    );
    let _ = writeln!(
        out,
        "Total security-tagged elements: {}",
        overview.total_security_tagged_elements
    );

    if !overview.high_attention_items.is_empty() {
        let _ = writeln!(out, "\nHIGH ATTENTION:");
        for (idx, item) in overview.high_attention_items.iter().enumerate() {
            let _ = writeln!(
                out,
                "  {}. [{}] {} ({})",
                idx + 1,
                item.repo,
                item.reason,
                item.file_path
            );
        }
    }

    out
}

fn status_label(status: &RepoStatus) -> &'static str {
    match status {
        RepoStatus::Updated => "UPDATED",
        RepoStatus::UpToDate => "UP_TO_DATE",
        RepoStatus::DirtySkipped => "DIRTY_SKIPPED",
        RepoStatus::FetchFailed { .. } => "FETCH_FAILED",
        RepoStatus::PullFailed { .. } => "PULL_FAILED",
        RepoStatus::Skipped { .. } => "SKIPPED",
    }
}
