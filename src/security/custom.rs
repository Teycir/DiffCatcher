use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::error::{PatrolError, Result};
use crate::types::{SecurityTagDefinition, TagSeverity};

#[derive(Debug, Deserialize)]
struct CustomConfig {
    version: u32,
    mode: String,
    tags: Vec<CustomTag>,
}

#[derive(Debug, Deserialize)]
struct CustomTag {
    tag: String,
    description: String,
    severity: TagSeverity,
    patterns: Vec<String>,
    #[serde(default)]
    negative_patterns: Vec<String>,
    #[serde(default)]
    min_matches: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
pub enum MergeMode {
    Extend,
    Replace,
}

#[derive(Debug)]
pub struct CustomPatternSet {
    pub mode: MergeMode,
    pub tags: Vec<SecurityTagDefinition>,
}

pub fn load_custom_patterns(path: &Path) -> Result<CustomPatternSet> {
    let raw = fs::read_to_string(path)?;
    let cfg: CustomConfig = serde_json::from_str(&raw)?;

    if cfg.version != 1 {
        return Err(PatrolError::InvalidArgument(format!(
            "unsupported security tag file version {} (expected 1)",
            cfg.version
        )));
    }

    let mode = match cfg.mode.as_str() {
        "extend" => MergeMode::Extend,
        "replace" => MergeMode::Replace,
        other => {
            return Err(PatrolError::InvalidArgument(format!(
                "invalid security tag mode '{}', expected 'extend' or 'replace'",
                other
            )));
        }
    };

    let tags = cfg
        .tags
        .into_iter()
        .map(|tag| SecurityTagDefinition {
            tag: tag.tag,
            patterns: tag.patterns,
            negative_patterns: tag.negative_patterns,
            description: tag.description,
            severity: tag.severity,
            min_matches: tag.min_matches.unwrap_or(1),
        })
        .collect::<Vec<_>>();

    Ok(CustomPatternSet { mode, tags })
}
