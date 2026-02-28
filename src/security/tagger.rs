use std::collections::BTreeMap;

use regex::Regex;

use crate::error::Result;
use crate::types::{
    ChangeType, FileChangeDetail, HighAttentionItem, SecurityReview, SecurityTagDefinition,
    TagSeverity,
};

#[derive(Debug)]
struct CompiledTag {
    def: SecurityTagDefinition,
    patterns: Vec<Regex>,
    negative_patterns: Vec<Regex>,
}

pub fn tag_file_changes(
    file_changes: &mut [FileChangeDetail],
    defs: &[SecurityTagDefinition],
    include_test_security: bool,
) -> Result<SecurityReview> {
    let compiled = compile_tags(defs)?;

    let mut review = SecurityReview::default();

    for file in file_changes {
        for element in &mut file.elements {
            let corpus = format!(
                "{}\n{}\n{}\n{}",
                element.file_path,
                element.name,
                element.signature.clone().unwrap_or_default(),
                element.snippet.diff_lines
            );

            for tag in &compiled {
                let negative_hit = tag.negative_patterns.iter().any(|re| re.is_match(&corpus));
                if negative_hit {
                    continue;
                }

                let mut count = 0_u32;
                for re in &tag.patterns {
                    if re.is_match(&corpus) {
                        count += 1;
                    }
                }

                if count >= tag.def.min_matches {
                    if !element.security_tags.contains(&tag.def.tag) {
                        element.security_tags.push(tag.def.tag.clone());
                    }
                }
            }

            if !element.security_tags.is_empty() && element.change_type == ChangeType::Removed {
                if !element
                    .security_tags
                    .iter()
                    .any(|tag| tag == "security-removal")
                {
                    element.security_tags.push("security-removal".to_string());
                }
            }

            if element.security_tags.is_empty() {
                continue;
            }

            review.total_security_tagged_elements += 1;

            for tag_name in &element.security_tags {
                *review.by_tag.entry(tag_name.clone()).or_insert(0) += 1;
                if let Some(def) = defs.iter().find(|d| &d.tag == tag_name) {
                    *review.by_severity.entry(def.severity.clone()).or_insert(0) += 1;
                } else if tag_name == "security-removal" {
                    *review.by_severity.entry(TagSeverity::High).or_insert(0) += 1;
                }
            }

            let preview = element
                .snippet
                .after
                .as_ref()
                .or(element.snippet.before.as_ref())
                .map(|s| s.code.lines().take(5).collect::<Vec<_>>().join("\n"))
                .unwrap_or_default();

            if !element.in_test || include_test_security {
                if element.change_type == ChangeType::Removed
                    && element
                        .security_tags
                        .iter()
                        .any(|tag| tag == "security-removal")
                {
                    review.high_attention_items.push(HighAttentionItem {
                        reason: "Security control REMOVED".to_string(),
                        element_name: element.name.clone(),
                        element_kind: element.kind,
                        change_type: element.change_type,
                        file_path: element.file_path.clone(),
                        tags: element.security_tags.clone(),
                        code_preview: preview.clone(),
                        snippet_ref: format!("{}:{}", element.file_path, element.name),
                    });
                } else if element.change_type == ChangeType::Added
                    && (element.security_tags.iter().any(|t| t == "crypto")
                        || element.security_tags.iter().any(|t| t == "authentication"))
                {
                    review.high_attention_items.push(HighAttentionItem {
                        reason: "New crypto/auth code added".to_string(),
                        element_name: element.name.clone(),
                        element_kind: element.kind,
                        change_type: element.change_type,
                        file_path: element.file_path.clone(),
                        tags: element.security_tags.clone(),
                        code_preview: preview.clone(),
                        snippet_ref: format!("{}:{}", element.file_path, element.name),
                    });
                }
            }

            review.flagged_elements.push(element.clone());
        }
    }

    Ok(review)
}

fn compile_tags(defs: &[SecurityTagDefinition]) -> Result<Vec<CompiledTag>> {
    let mut compiled = Vec::with_capacity(defs.len());

    for def in defs {
        let patterns = def
            .patterns
            .iter()
            .map(|pattern| Regex::new(&format!("(?i){}", pattern)))
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let negative_patterns = def
            .negative_patterns
            .iter()
            .map(|pattern| Regex::new(&format!("(?i){}", pattern)))
            .collect::<std::result::Result<Vec<_>, _>>()?;

        compiled.push(CompiledTag {
            def: def.clone(),
            patterns,
            negative_patterns,
        });
    }

    Ok(compiled)
}

pub fn merge_tag_severity(defs: &[SecurityTagDefinition]) -> BTreeMap<String, TagSeverity> {
    defs.iter()
        .map(|d| (d.tag.clone(), d.severity.clone()))
        .collect()
}
