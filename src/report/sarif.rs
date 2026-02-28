use std::collections::BTreeMap;

use serde::Serialize;

use crate::types::{ChangedElement, DiffResult, RepoResult, SecurityTagDefinition, TagSeverity};

const SARIF_SCHEMA: &str =
    "https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/schemas/sarif-schema-2.1.0.json";
const SARIF_VERSION: &str = "2.1.0";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifLog {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub version: String,
    pub runs: Vec<SarifRun>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifRun {
    pub tool: SarifTool,
    pub results: Vec<SarifResult>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<SarifArtifact>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifTool {
    pub driver: SarifDriver,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifDriver {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub information_uri: Option<String>,
    pub rules: Vec<SarifRule>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifRule {
    pub id: String,
    pub name: String,
    pub short_description: SarifMessage,
    pub default_configuration: SarifRuleConfiguration,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<SarifMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<SarifRuleProperties>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifRuleConfiguration {
    pub level: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifRuleProperties {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifMessage {
    pub text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifResult {
    pub rule_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_index: Option<usize>,
    pub level: String,
    pub message: SarifMessage,
    pub locations: Vec<SarifLocation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<SarifResultProperties>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifResultProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_kind: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub all_tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_label: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifLocation {
    pub physical_location: SarifPhysicalLocation,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifPhysicalLocation {
    pub artifact_location: SarifArtifactLocation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<SarifRegion>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifArtifactLocation {
    pub uri: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifRegion {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<SarifMessage>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifArtifact {
    pub location: SarifArtifactLocation,
}

fn severity_to_level(severity: &TagSeverity) -> &'static str {
    match severity {
        TagSeverity::High => "error",
        TagSeverity::Medium => "warning",
        TagSeverity::Low => "note",
        TagSeverity::Info => "none",
    }
}

fn build_rules(tag_defs: &[SecurityTagDefinition]) -> (Vec<SarifRule>, BTreeMap<String, usize>) {
    let mut rules = Vec::new();
    let mut index_map = BTreeMap::new();

    for (idx, def) in tag_defs.iter().enumerate() {
        let rule_id = format!("DC/{}", def.tag);
        index_map.insert(def.tag.clone(), idx);

        rules.push(SarifRule {
            id: rule_id,
            name: def.tag.clone(),
            short_description: SarifMessage {
                text: def.description.clone(),
            },
            default_configuration: SarifRuleConfiguration {
                level: severity_to_level(&def.severity).to_string(),
            },
            help: def
                .false_positive_note
                .as_ref()
                .map(|note| SarifMessage { text: note.clone() }),
            properties: Some(SarifRuleProperties {
                tags: vec!["security".to_string()],
            }),
        });
    }

    // Add the synthetic security-removal rule.
    let removal_idx = rules.len();
    index_map.insert("security-removal".to_string(), removal_idx);
    rules.push(SarifRule {
        id: "DC/security-removal".to_string(),
        name: "security-removal".to_string(),
        short_description: SarifMessage {
            text: "Security control was removed".to_string(),
        },
        default_configuration: SarifRuleConfiguration {
            level: "error".to_string(),
        },
        help: None,
        properties: Some(SarifRuleProperties {
            tags: vec!["security".to_string()],
        }),
    });

    (rules, index_map)
}

fn element_to_results(
    element: &ChangedElement,
    rule_index_map: &BTreeMap<String, usize>,
    tag_severity: &BTreeMap<String, TagSeverity>,
    repo_name: Option<&str>,
    diff_label: Option<&str>,
) -> Vec<SarifResult> {
    let mut results = Vec::new();

    for tag in &element.security_tags {
        let rule_id = format!("DC/{}", tag);
        let rule_index = rule_index_map.get(tag).copied();

        let severity = tag_severity.get(tag).unwrap_or(&TagSeverity::Medium);
        let level = severity_to_level(severity).to_string();

        let snippet_text = element
            .snippet
            .after
            .as_ref()
            .or(element.snippet.before.as_ref())
            .map(|s| s.code.lines().take(10).collect::<Vec<_>>().join("\n"));

        let region = element.line_range.map(|(start, end)| SarifRegion {
            start_line: Some(start),
            end_line: Some(end),
            snippet: snippet_text.map(|text| SarifMessage { text }),
        });

        let message_text = format!(
            "{} '{}' ({:?}) — {:?} in {}",
            tag, element.name, element.kind, element.change_type, element.file_path
        );

        results.push(SarifResult {
            rule_id,
            rule_index,
            level,
            message: SarifMessage { text: message_text },
            locations: vec![SarifLocation {
                physical_location: SarifPhysicalLocation {
                    artifact_location: SarifArtifactLocation {
                        uri: element.file_path.clone(),
                    },
                    region,
                },
            }],
            properties: Some(SarifResultProperties {
                change_type: Some(format!("{:?}", element.change_type)),
                element_kind: Some(format!("{:?}", element.kind)),
                all_tags: element.security_tags.clone(),
                repo: repo_name.map(String::from),
                diff_label: diff_label.map(String::from),
            }),
        });
    }

    results
}

fn collect_artifacts(diffs: &[DiffResult]) -> Vec<SarifArtifact> {
    let mut seen = BTreeMap::new();
    for diff in diffs {
        for file in &diff.file_changes {
            seen.entry(file.path.clone()).or_insert(SarifArtifact {
                location: SarifArtifactLocation {
                    uri: file.path.clone(),
                },
            });
        }
    }
    seen.into_values().collect()
}

pub fn build_sarif_from_results(
    repos: &[RepoResult],
    tag_defs: &[SecurityTagDefinition],
) -> SarifLog {
    let (rules, rule_index_map) = build_rules(tag_defs);
    let tag_severity: BTreeMap<String, TagSeverity> = tag_defs
        .iter()
        .map(|d| (d.tag.clone(), d.severity.clone()))
        .chain(std::iter::once((
            "security-removal".to_string(),
            TagSeverity::High,
        )))
        .collect();

    let mut results = Vec::new();
    let mut artifacts = Vec::new();

    for repo in repos {
        artifacts.extend(collect_artifacts(&repo.diffs));

        for diff in &repo.diffs {
            for file in &diff.file_changes {
                for element in &file.elements {
                    if element.security_tags.is_empty() {
                        continue;
                    }
                    results.extend(element_to_results(
                        element,
                        &rule_index_map,
                        &tag_severity,
                        Some(&repo.repo_name),
                        Some(&diff.label),
                    ));
                }
            }
        }
    }

    SarifLog {
        schema: SARIF_SCHEMA.to_string(),
        version: SARIF_VERSION.to_string(),
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: "DiffCatcher".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    information_uri: Some("https://github.com/Teycir/DiffCatcher".to_string()),
                    rules,
                },
            },
            results,
            artifacts,
        }],
    }
}

pub fn build_sarif_from_single_repo(
    repo: &RepoResult,
    tag_defs: &[SecurityTagDefinition],
) -> SarifLog {
    build_sarif_from_results(std::slice::from_ref(repo), tag_defs)
}
