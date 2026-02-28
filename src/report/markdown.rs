use std::fmt::Write;

use crate::types::{DiffResult, GlobalSecurityOverview, GlobalSummary, RepoResult};

pub fn render_repo_status(repo: &RepoResult) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# {}", repo.repo_name);
    let _ = writeln!(out);
    let _ = writeln!(out, "| Field | Value |");
    let _ = writeln!(out, "|-------|-------|");
    let _ = writeln!(out, "| Path | `{}` |", repo.repo_path.display());
    let _ = writeln!(out, "| Branch | `{}` |", repo.branch);
    let _ = writeln!(out, "| Status | `{:?}` |", repo.status);
    if let Some(pre) = &repo.pre_pull {
        let _ = writeln!(out, "| Pre-commit | `{}` {} |", pre.short_hash, pre.message);
    }
    if let Some(post) = &repo.post_pull {
        let _ = writeln!(
            out,
            "| Post-commit | `{}` {} |",
            post.short_hash, post.message
        );
    }
    let _ = writeln!(out);

    if !repo.errors.is_empty() {
        let _ = writeln!(out, "## Errors");
        for err in &repo.errors {
            let _ = writeln!(out, "- ⚠️ {}", err);
        }
        let _ = writeln!(out);
    }

    out
}

pub fn render_diff_summary(diff: &DiffResult) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Diff {}", diff.label);
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "**From**: `{}` {}  ",
        diff.from_commit.short_hash, diff.from_commit.message
    );
    let _ = writeln!(
        out,
        "**To**: `{}` {}  ",
        diff.to_commit.short_hash, diff.to_commit.message
    );
    let _ = writeln!(out);

    // Stats summary
    let _ = writeln!(
        out,
        "> **{}** files changed, **+{}** insertions, **-{}** deletions",
        diff.files_changed, diff.insertions, diff.deletions
    );
    let _ = writeln!(out);

    // File changes table
    if !diff.file_changes.is_empty() {
        let _ = writeln!(out, "## File Changes");
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "| Status | File | Language | +Lines | -Lines | Change |"
        );
        let _ = writeln!(
            out,
            "|--------|------|----------|--------|--------|--------|"
        );
        for file in &diff.file_changes {
            let status_icon = match file.status {
                crate::types::FileStatus::Added => "🟢 Added",
                crate::types::FileStatus::Modified => "🟡 Modified",
                crate::types::FileStatus::Deleted => "🔴 Deleted",
                crate::types::FileStatus::Renamed => "🔵 Renamed",
                crate::types::FileStatus::Copied => "📋 Copied",
                crate::types::FileStatus::Unknown => "❓ Unknown",
            };
            let lang = format!("{:?}", file.language);
            let bar = diffstat_bar(file.insertions, file.deletions, 20);
            let _ = writeln!(
                out,
                "| {} | `{}` | {} | +{} | -{} | {} |",
                status_icon, file.path, lang, file.insertions, file.deletions, bar
            );
        }
        let _ = writeln!(out);
    }

    // Elements section
    if let Some(es) = &diff.element_summary {
        let _ = writeln!(out, "## Elements Changed ({})", es.total_elements);
        let _ = writeln!(out);

        // By-kind summary table
        if !es.by_kind.is_empty() {
            let _ = writeln!(out, "| Kind | Added | Modified | Removed |");
            let _ = writeln!(out, "|------|-------|----------|---------|");
            for (kind, counts) in &es.by_kind {
                let _ = writeln!(
                    out,
                    "| {:?} | {} | {} | {} |",
                    kind, counts.added, counts.modified, counts.removed
                );
            }
            let _ = writeln!(out);
        }

        // Element details - collapsible if many
        let use_details = es.elements.len() > 15;
        if use_details {
            let _ = writeln!(
                out,
                "<details>\n<summary>Show all {} elements</summary>\n",
                es.elements.len()
            );
        }

        for element in &es.elements {
            let change_icon = match element.change_type {
                crate::types::ChangeType::Added => "➕",
                crate::types::ChangeType::Modified => "✏️",
                crate::types::ChangeType::Removed => "➖",
            };
            let tags = if element.security_tags.is_empty() {
                String::new()
            } else {
                format!(" 🔐 `{}`", element.security_tags.join("`, `"))
            };
            let _ = writeln!(
                out,
                "- {} **`{:?}`** `{}` in `{}`{}",
                change_icon, element.kind, element.name, element.file_path, tags
            );
            if let Some(sig) = &element.signature {
                let _ = writeln!(out, "  ```");
                let _ = writeln!(out, "  {}", sig);
                let _ = writeln!(out, "  ```");
            }
            if let Some(refs) = &element.snippet_files {
                if let Some(path) = &refs.before {
                    let _ = writeln!(out, "  - 📄 before: `{}`", path);
                }
                if let Some(path) = &refs.after {
                    let _ = writeln!(out, "  - 📄 after: `{}`", path);
                }
                if let Some(path) = &refs.diff {
                    let _ = writeln!(out, "  - 📄 diff: `{}`", path);
                }
            }
        }

        if use_details {
            let _ = writeln!(out, "\n</details>");
        }
        let _ = writeln!(out);
    }

    // Security review section
    if let Some(sr) = &diff.security_review
        && sr.total_security_tagged_elements > 0
    {
            let _ = writeln!(
                out,
                "## 🔒 Security Review ({} flagged)",
                sr.total_security_tagged_elements
            );
            let _ = writeln!(out);

            // By tag table
            if !sr.by_tag.is_empty() {
                let _ = writeln!(out, "| Tag | Count |");
                let _ = writeln!(out, "|-----|-------|");
                for (tag, count) in &sr.by_tag {
                    let _ = writeln!(out, "| `{}` | {} |", tag, count);
                }
                let _ = writeln!(out);
            }

            if !sr.high_attention_items.is_empty() {
                let _ = writeln!(out, "### ⚠️ High Attention Items");
                let _ = writeln!(out);
                for item in &sr.high_attention_items {
                    let _ = writeln!(
                        out,
                        "- **{}**: `{}` in `{}`",
                        item.reason, item.element_name, item.file_path
                    );
                    if !item.code_preview.is_empty() {
                        let _ = writeln!(out, "  ```");
                        let _ = writeln!(out, "  {}", item.code_preview);
                        let _ = writeln!(out, "  ```");
                    }
                }
            }
    }

    out
}

pub fn render_global_summary(summary: &GlobalSummary) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# DiffCatcher Report");
    let _ = writeln!(out);
    let _ = writeln!(out, "| Field | Value |");
    let _ = writeln!(out, "|-------|-------|");
    let _ = writeln!(out, "| Scan root | `{}` |", summary.scan_root.display());
    let _ = writeln!(out, "| Report dir | `{}` |", summary.report_dir.display());
    let _ = writeln!(out, "| Timestamp | {} |", summary.timestamp);
    let _ = writeln!(out, "| Total repos | {} |", summary.total_repos_found);
    let _ = writeln!(out);

    // Status breakdown
    let _ = writeln!(out, "## Status Summary");
    let _ = writeln!(out);
    let _ = writeln!(out, "| Status | Count |");
    let _ = writeln!(out, "|--------|-------|");
    let _ = writeln!(out, "| ✅ Updated | {} |", summary.updated);
    let _ = writeln!(out, "| ➖ Up to date | {} |", summary.up_to_date);
    let _ = writeln!(out, "| ⏭️ Dirty skipped | {} |", summary.dirty_skipped);
    let _ = writeln!(
        out,
        "| ❌ Failed | {} |",
        summary.fetch_failed + summary.pull_failed
    );
    let _ = writeln!(out, "| ⏩ Skipped | {} |", summary.skipped);
    let _ = writeln!(out);

    let _ = writeln!(
        out,
        "> **{}** elements changed across all repos, **{}** security-tagged",
        summary.total_elements_changed_across_all_repos, summary.total_security_tagged_elements
    );
    let _ = writeln!(out);

    // Repositories table
    let _ = writeln!(out, "## Repositories");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "| Repo | Branch | Status | Files | +/- | Elements | Security |"
    );
    let _ = writeln!(
        out,
        "|------|--------|--------|-------|-----|----------|----------|"
    );
    for repo in &summary.repos {
        let status_icon = match &repo.status {
            crate::types::RepoStatus::Updated => "✅",
            crate::types::RepoStatus::UpToDate => "➖",
            crate::types::RepoStatus::DirtySkipped => "⏭️",
            crate::types::RepoStatus::FetchFailed { .. } => "❌",
            crate::types::RepoStatus::PullFailed { .. } => "❌",
            crate::types::RepoStatus::Skipped { .. } => "⏩",
        };
        let (files, insertions, deletions, elements, security) =
            if let Some(latest) = &repo.latest_diff {
                (
                    latest.files_changed,
                    latest.insertions,
                    latest.deletions,
                    latest.elements_added + latest.elements_modified + latest.elements_removed,
                    latest.security_tagged,
                )
            } else {
                (0, 0, 0, 0, 0)
            };
        let _ = writeln!(
            out,
            "| **{}** | `{}` | {} | {} | +{}/-{} | {} | {} |",
            repo.name, repo.branch, status_icon, files, insertions, deletions, elements, security
        );
    }
    let _ = writeln!(out);

    out
}

pub fn render_security_overview(overview: &GlobalSecurityOverview) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# 🔒 Security Overview");
    let _ = writeln!(out);
    let _ = writeln!(out, "| Metric | Value |");
    let _ = writeln!(out, "|--------|-------|");
    let _ = writeln!(
        out,
        "| Repos scanned | {} |",
        overview.total_repos_scanned
    );
    let _ = writeln!(
        out,
        "| Repos with security flags | {} |",
        overview.repos_with_security_flags
    );
    let _ = writeln!(
        out,
        "| Total security-tagged elements | {} |",
        overview.total_security_tagged_elements
    );
    let _ = writeln!(out);

    // Severity breakdown
    if !overview.by_severity.is_empty() {
        let _ = writeln!(out, "## By Severity");
        let _ = writeln!(out);
        let _ = writeln!(out, "| Severity | Count |");
        let _ = writeln!(out, "|----------|-------|");
        for (severity, count) in &overview.by_severity {
            let icon = match severity {
                crate::types::TagSeverity::High => "🔴",
                crate::types::TagSeverity::Medium => "🟡",
                crate::types::TagSeverity::Low => "🟢",
                crate::types::TagSeverity::Info => "🔵",
            };
            let _ = writeln!(out, "| {} {:?} | {} |", icon, severity, count);
        }
        let _ = writeln!(out);
    }

    // By tag breakdown
    if !overview.by_tag_global.is_empty() {
        let _ = writeln!(out, "## By Tag");
        let _ = writeln!(out);
        let _ = writeln!(out, "| Tag | Count |");
        let _ = writeln!(out, "|-----|-------|");
        for (tag, count) in &overview.by_tag_global {
            let _ = writeln!(out, "| `{}` | {} |", tag, count);
        }
        let _ = writeln!(out);
    }

    // High attention items
    if !overview.high_attention_items.is_empty() {
        let _ = writeln!(out, "## ⚠️ High Attention Items");
        let _ = writeln!(out);
        for (idx, item) in overview.high_attention_items.iter().enumerate() {
            let _ = writeln!(out, "### {}. [{}] {}", idx + 1, item.repo, item.reason);
            let _ = writeln!(out, "- **Element**: `{}`", item.element_name);
            let _ = writeln!(out, "- **File**: `{}`", item.file_path);
            let _ = writeln!(out, "- **Tags**: `{}`", item.tags.join("`, `"));
            let _ = writeln!(
                out,
                "- **Commits**: `{}` → `{}`",
                item.commit_from, item.commit_to
            );
            if let Some(code) = &item.after_code_preview
                && !code.is_empty()
            {
                let _ = writeln!(out, "```");
                let _ = writeln!(out, "{}", code);
                let _ = writeln!(out, "```");
            }
            let _ = writeln!(out);
        }
    }

    // Repos with security flags
    if !overview.repos.is_empty() {
        let _ = writeln!(out, "## Flagged Repositories");
        let _ = writeln!(out);
        let _ = writeln!(out, "| Repo | Security Elements | Tags |");
        let _ = writeln!(out, "|------|-------------------|------|");
        for repo in &overview.repos {
            let _ = writeln!(
                out,
                "| **{}** | {} | `{}` |",
                repo.name,
                repo.security_elements,
                repo.tags.join("`, `")
            );
        }
    }

    out
}

fn diffstat_bar(insertions: u32, deletions: u32, width: u32) -> String {
    let total = insertions + deletions;
    if total == 0 {
        return String::new();
    }
    let plus_width = ((insertions as f64 / total as f64) * width as f64).round() as u32;
    let minus_width = width.saturating_sub(plus_width);
    let plus_chars = "█".repeat(plus_width as usize);
    let minus_chars = "░".repeat(minus_width as usize);
    format!("`{}{}`", plus_chars, minus_chars)
}
