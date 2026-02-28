use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;

use crate::types::{
    GlobalHighAttentionItem, GlobalSecurityOverview, RepoResult, RepoSecuritySummary, TagSeverity,
};

pub fn build_global_security_overview(repos: &[RepoResult]) -> GlobalSecurityOverview {
    let mut total_security_tagged_elements = 0_u32;
    let mut by_tag_global = BTreeMap::new();
    let mut by_severity = BTreeMap::new();
    let mut high_attention_items = Vec::new();
    let mut repo_summaries = Vec::new();

    for repo in repos {
        let mut repo_security_count = 0_u32;
        let mut repo_tags = BTreeSet::new();

        for diff in &repo.diffs {
            if let Some(review) = &diff.security_review {
                repo_security_count += review.total_security_tagged_elements;
                total_security_tagged_elements += review.total_security_tagged_elements;

                for (tag, count) in &review.by_tag {
                    *by_tag_global.entry(tag.clone()).or_insert(0) += *count;
                    repo_tags.insert(tag.clone());
                }

                for (severity, count) in &review.by_severity {
                    *by_severity.entry(severity.clone()).or_insert(0) += *count;
                }

                for hi in &review.high_attention_items {
                    high_attention_items.push(GlobalHighAttentionItem {
                        repo: repo.repo_name.clone(),
                        reason: hi.reason.clone(),
                        element_name: hi.element_name.clone(),
                        file_path: hi.file_path.clone(),
                        tags: hi.tags.clone(),
                        before_code_preview: None,
                        after_code_preview: Some(hi.code_preview.clone()),
                        commit_from: diff.from_commit.short_hash.clone(),
                        commit_to: diff.to_commit.short_hash.clone(),
                    });
                }
            }
        }

        if repo_security_count > 0 {
            let detail_path = repo
                .diffs
                .first()
                .and_then(|d| d.summary_json_filename.clone())
                .unwrap_or_else(|| "diffs/summary_N_vs_N-1.json".to_string());

            repo_summaries.push(RepoSecuritySummary {
                name: repo.repo_name.clone(),
                security_elements: repo_security_count,
                tags: repo_tags.into_iter().collect(),
                detail_path,
            });
        }
    }

    // Deduplicate repos_with_security_flags because each diff can increment it.
    let repos_with_security_flags = repos
        .iter()
        .filter(|repo| {
            repo.diffs.iter().any(|diff| {
                diff.security_review
                    .as_ref()
                    .is_some_and(|r| r.total_security_tagged_elements > 0)
            })
        })
        .count() as u32;

    if by_severity.is_empty() {
        by_severity.insert(TagSeverity::Info, 0);
    }

    GlobalSecurityOverview {
        timestamp: Utc::now(),
        total_repos_scanned: repos.len() as u32,
        repos_with_security_flags,
        total_security_tagged_elements,
        by_tag_global,
        by_severity,
        high_attention_items,
        repos: repo_summaries,
    }
}
