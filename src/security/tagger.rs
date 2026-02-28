use std::collections::BTreeMap;

use fancy_regex::Regex as FancyRegex;
use rayon::prelude::*;
use regex::Regex;

use crate::error::Result;
use crate::types::{
    ChangeType, FileChangeDetail, HighAttentionItem, PatternKind, RiskLevel, RiskScore,
    SecurityReview, SecurityTagDefinition, TagSeverity,
};

#[derive(Debug)]
enum CompiledPattern {
    Standard(Regex),
    Fancy(FancyRegex),
}

impl CompiledPattern {
    fn is_match(&self, text: &str) -> bool {
        match self {
            CompiledPattern::Standard(re) => re.is_match(text),
            CompiledPattern::Fancy(re) => re.is_match(text).unwrap_or(false),
        }
    }
}

#[derive(Debug)]
struct CompiledTag {
    def: SecurityTagDefinition,
    patterns: Vec<CompiledPattern>,
    negative_patterns: Vec<CompiledPattern>,
}

pub fn tag_file_changes(
    file_changes: &mut [FileChangeDetail],
    defs: &[SecurityTagDefinition],
    include_test_security: bool,
) -> Result<SecurityReview> {
    let compiled = compile_tags(defs)?;
    let partials = file_changes
        .par_iter_mut()
        .map(|file| process_file_tags(file, &compiled, defs, include_test_security))
        .collect::<Vec<_>>();

    let mut review = SecurityReview::default();
    for partial in partials {
        merge_review(&mut review, partial);
    }

    apply_composition_escalation(&mut review);
    review.risk_score = Some(compute_risk_score(&review));

    Ok(review)
}

fn compile_pattern(pattern: &str, kind: Option<PatternKind>) -> Result<CompiledPattern> {
    let pat = format!("(?i){}", pattern);
    match kind {
        Some(PatternKind::FancyRegex) => {
            let re = FancyRegex::new(&pat)?;
            Ok(CompiledPattern::Fancy(re))
        }
        _ => {
            let re = Regex::new(&pat)?;
            Ok(CompiledPattern::Standard(re))
        }
    }
}

fn compile_tags(defs: &[SecurityTagDefinition]) -> Result<Vec<CompiledTag>> {
    let mut compiled = Vec::with_capacity(defs.len());

    for def in defs {
        let patterns = def
            .patterns
            .iter()
            .map(|pattern| compile_pattern(pattern, def.pattern_kind))
            .collect::<Result<Vec<_>>>()?;

        let negative_patterns = def
            .negative_patterns
            .iter()
            .map(|pattern| compile_pattern(pattern, def.pattern_kind))
            .collect::<Result<Vec<_>>>()?;

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

fn process_file_tags(
    file: &mut FileChangeDetail,
    compiled: &[CompiledTag],
    defs: &[SecurityTagDefinition],
    include_test_security: bool,
) -> SecurityReview {
    let mut review = SecurityReview::default();

    for element in &mut file.elements {
        let corpus = format!(
            "{}\n{}\n{}\n{}",
            element.file_path,
            element.name,
            element.signature.clone().unwrap_or_default(),
            element.snippet.diff_lines
        );

        for tag in compiled {
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

            if count >= tag.def.min_matches && !element.security_tags.contains(&tag.def.tag) {
                element.security_tags.push(tag.def.tag.clone());
            }
        }

        if !element.security_tags.is_empty()
            && element.change_type == ChangeType::Removed
            && !element
                .security_tags
                .iter()
                .any(|tag| tag == "security-removal")
        {
            element.security_tags.push("security-removal".to_string());
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

    review
}

fn merge_review(target: &mut SecurityReview, source: SecurityReview) {
    target.total_security_tagged_elements += source.total_security_tagged_elements;
    for (tag, count) in source.by_tag {
        *target.by_tag.entry(tag).or_insert(0) += count;
    }
    for (severity, count) in source.by_severity {
        *target.by_severity.entry(severity).or_insert(0) += count;
    }
    target
        .high_attention_items
        .extend(source.high_attention_items);
    target.flagged_elements.extend(source.flagged_elements);
}

// ── Risk scoring (inspired by SCPF risk_scorer.rs) ──────────────────────────

const WEIGHT_HIGH: f64 = 15.0;
const WEIGHT_MEDIUM: f64 = 8.0;
const WEIGHT_LOW: f64 = 3.0;
const WEIGHT_INFO: f64 = 1.0;

fn compute_risk_score(review: &SecurityReview) -> RiskScore {
    let severity_score = review
        .by_severity
        .iter()
        .map(|(sev, count)| {
            let w = match sev {
                TagSeverity::High => WEIGHT_HIGH,
                TagSeverity::Medium => WEIGHT_MEDIUM,
                TagSeverity::Low => WEIGHT_LOW,
                TagSeverity::Info => WEIGHT_INFO,
            };
            w * (*count as f64)
        })
        .sum::<f64>();

    let concentration_factor = {
        let max_tag_count = review.by_tag.values().max().copied().unwrap_or(0);
        if max_tag_count > 10 {
            1.3
        } else if max_tag_count > 5 {
            1.2
        } else if max_tag_count > 3 {
            1.1
        } else {
            1.0
        }
    };

    let composition_bonus = compute_composition_bonus(review);

    let total = ((severity_score * concentration_factor) + composition_bonus).clamp(0.0, 100.0);
    let level = score_to_level(total);

    RiskScore {
        total,
        level: Some(level),
        severity_score,
        concentration_factor,
        composition_bonus,
    }
}

fn score_to_level(score: f64) -> RiskLevel {
    match score as u32 {
        80..=u32::MAX => RiskLevel::Critical,
        60..=79 => RiskLevel::High,
        40..=59 => RiskLevel::Medium,
        20..=39 => RiskLevel::Low,
        _ => RiskLevel::Minimal,
    }
}

fn compute_composition_bonus(review: &SecurityReview) -> f64 {
    let tags = &review.by_tag;
    let mut bonus = 0.0;

    // crypto + no authentication guard = higher risk
    if tags.contains_key("crypto") && !tags.contains_key("authentication") {
        bonus += 5.0;
    }

    // secrets + hardcoded-secret = very high risk
    if tags.contains_key("secrets") && tags.contains_key("hardcoded-secret") {
        bonus += 10.0;
    }

    // command-injection + unsafe-code = compounding risk
    if tags.contains_key("command-injection") && tags.contains_key("unsafe-code") {
        bonus += 8.0;
    }

    // network + no input-validation = unvalidated external input
    if tags.contains_key("network") && !tags.contains_key("input-validation") {
        bonus += 4.0;
    }

    // security-removal present = always escalate
    if tags.contains_key("security-removal") {
        bonus += 10.0;
    }

    bonus
}

// ── Composition-based high-attention escalation ─────────────────────────────

const DANGEROUS_COMBOS: &[(&[&str], &str)] = &[
    (
        &["crypto", "weak-hashing"],
        "Crypto code using weak hash algorithm",
    ),
    (
        &["secrets", "hardcoded-secret"],
        "Hardcoded credentials in secrets-handling code",
    ),
    (
        &["authentication", "operator-misuse"],
        "Auth code with potential operator misuse",
    ),
    (
        &["network", "command-injection"],
        "Network-facing code with command injection risk",
    ),
    (
        &["deserialization", "network"],
        "Untrusted deserialization from network input",
    ),
];

fn apply_composition_escalation(review: &mut SecurityReview) {
    for element in &review.flagged_elements {
        for (combo, reason) in DANGEROUS_COMBOS {
            let all_present = combo.iter().all(|tag| element.security_tags.contains(&tag.to_string()));
            if !all_present {
                continue;
            }

            let already_flagged = review
                .high_attention_items
                .iter()
                .any(|hi| hi.element_name == element.name && hi.file_path == element.file_path);
            if already_flagged {
                continue;
            }

            let preview = element
                .snippet
                .after
                .as_ref()
                .or(element.snippet.before.as_ref())
                .map(|s| s.code.lines().take(5).collect::<Vec<_>>().join("\n"))
                .unwrap_or_default();

            review.high_attention_items.push(HighAttentionItem {
                reason: format!("Composition risk: {}", reason),
                element_name: element.name.clone(),
                element_kind: element.kind,
                change_type: element.change_type,
                file_path: element.file_path.clone(),
                tags: element.security_tags.clone(),
                code_preview: preview,
                snippet_ref: format!("{}:{}", element.file_path, element.name),
            });
        }
    }
}
