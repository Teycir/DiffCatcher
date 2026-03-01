use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::Deserialize;

use crate::error::{PatrolError, Result};
use crate::extraction::elements::DetectedElement;
use crate::types::{ChangeType, ElementKind, Language, RawHunk};

#[derive(Debug, Clone)]
pub struct ExtractorPlugin {
    pub name: String,
    pub language: Option<Language>,
    pub kind: ElementKind,
    pub regex: Regex,
    pub capture_group: usize,
}

#[derive(Debug, Deserialize)]
struct ExtractorPluginFile {
    version: u32,
    extractors: Vec<ExtractorPluginSpec>,
}

#[derive(Debug, Deserialize)]
struct ExtractorPluginSpec {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    language: Option<String>,
    kind: String,
    regex: String,
    #[serde(default)]
    capture_group: Option<usize>,
}

#[derive(Debug, Default)]
struct Tracker {
    plus: bool,
    minus: bool,
    lines_added: u32,
    lines_removed: u32,
    signature: Option<String>,
    context: Option<String>,
    line_range: Option<(u32, u32)>,
}

pub fn load_extractor_plugins(paths: &[PathBuf]) -> Result<Vec<ExtractorPlugin>> {
    let mut plugins = Vec::new();

    for path in paths {
        let loaded = load_extractor_plugin_file(path)?;
        plugins.extend(loaded);
    }

    Ok(plugins)
}

fn load_extractor_plugin_file(path: &Path) -> Result<Vec<ExtractorPlugin>> {
    let raw = fs::read_to_string(path)?;
    let cfg: ExtractorPluginFile = serde_json::from_str(&raw).map_err(|err| {
        PatrolError::InvalidArgument(format!(
            "invalid extractor plugin file {}: {}",
            path.display(),
            err
        ))
    })?;

    if cfg.version != 1 {
        return Err(PatrolError::InvalidArgument(format!(
            "unsupported extractor plugin file version {} in {} (expected 1)",
            cfg.version,
            path.display()
        )));
    }

    let mut plugins = Vec::new();
    for spec in cfg.extractors {
        let kind = parse_element_kind(&spec.kind).ok_or_else(|| {
            PatrolError::InvalidArgument(format!(
                "unknown extractor kind '{}' in {}",
                spec.kind,
                path.display()
            ))
        })?;

        let language = match spec.language.as_deref() {
            None => None,
            Some(v) => Some(parse_language(v).ok_or_else(|| {
                PatrolError::InvalidArgument(format!(
                    "unknown extractor language '{}' in {}",
                    v,
                    path.display()
                ))
            })?),
        };

        let regex = Regex::new(&spec.regex).map_err(|err| {
            PatrolError::InvalidArgument(format!(
                "invalid extractor regex '{}' in {}: {}",
                spec.regex,
                path.display(),
                err
            ))
        })?;

        plugins.push(ExtractorPlugin {
            name: spec
                .name
                .unwrap_or_else(|| format!("plugin_{}", spec.kind.to_ascii_lowercase())),
            language,
            kind,
            regex,
            capture_group: spec.capture_group.unwrap_or(1),
        });
    }

    Ok(plugins)
}

pub fn detect_plugin_elements(
    _file_path: &str,
    hunks: &[RawHunk],
    language: &Language,
    plugins: &[ExtractorPlugin],
    max_elements: usize,
) -> Vec<DetectedElement> {
    if plugins.is_empty() || max_elements == 0 {
        return Vec::new();
    }

    let active = plugins
        .iter()
        .filter(|plugin| plugin.language.as_ref().is_none_or(|lang| lang == language))
        .collect::<Vec<_>>();

    if active.is_empty() {
        return Vec::new();
    }

    let mut tracked: BTreeMap<(ElementKind, String), Tracker> = BTreeMap::new();

    for hunk in hunks {
        for raw_line in hunk.lines.lines() {
            let (sign, content) = match raw_line.chars().next() {
                Some('+') => ('+', raw_line.get(1..).unwrap_or_default()),
                Some('-') => ('-', raw_line.get(1..).unwrap_or_default()),
                _ => continue,
            };

            let content = content.trim();
            if content.is_empty() {
                continue;
            }

            for plugin in &active {
                let Some(caps) = plugin.regex.captures(content) else {
                    continue;
                };

                let extracted_name = caps
                    .get(plugin.capture_group)
                    .map(|m| m.as_str().trim())
                    .filter(|v| !v.is_empty())
                    .unwrap_or(plugin.name.as_str())
                    .to_string();

                let key = (plugin.kind, extracted_name.clone());
                let tracker = tracked.entry(key).or_default();

                if sign == '+' {
                    tracker.plus = true;
                    tracker.lines_added += 1;
                } else {
                    tracker.minus = true;
                    tracker.lines_removed += 1;
                }

                if tracker.signature.is_none() {
                    tracker.signature = Some(content.to_string());
                    tracker.context = hunk.context_function.clone();
                    let line = if sign == '+' {
                        hunk.new_start
                    } else {
                        hunk.old_start
                    };
                    tracker.line_range = Some((line, line));
                }
            }
        }
    }

    let mut out = tracked
        .into_iter()
        .map(|((kind, name), tracker)| DetectedElement {
            kind,
            name,
            change_type: if tracker.plus && tracker.minus {
                ChangeType::Modified
            } else if tracker.plus {
                ChangeType::Added
            } else {
                ChangeType::Removed
            },
            line_range: tracker.line_range,
            lines_added: tracker.lines_added,
            lines_removed: tracker.lines_removed,
            enclosing_context: tracker.context,
            signature: tracker.signature,
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out.truncate(max_elements);

    out
}

fn parse_element_kind(value: &str) -> Option<ElementKind> {
    match value.trim().to_ascii_lowercase().as_str() {
        "function" => Some(ElementKind::Function),
        "method" => Some(ElementKind::Method),
        "struct" => Some(ElementKind::Struct),
        "class" => Some(ElementKind::Class),
        "enum" => Some(ElementKind::Enum),
        "trait" => Some(ElementKind::Trait),
        "interface" => Some(ElementKind::Interface),
        "impl" => Some(ElementKind::Impl),
        "module" => Some(ElementKind::Module),
        "import" => Some(ElementKind::Import),
        "constant" => Some(ElementKind::Constant),
        "static" => Some(ElementKind::Static),
        "typealias" | "type_alias" => Some(ElementKind::TypeAlias),
        "macro" => Some(ElementKind::Macro),
        "test" => Some(ElementKind::Test),
        "config" => Some(ElementKind::Config),
        "other" => Some(ElementKind::Other),
        _ => None,
    }
}

fn parse_language(value: &str) -> Option<Language> {
    match value.trim().to_ascii_lowercase().as_str() {
        "rust" | "rs" => Some(Language::Rust),
        "python" | "py" => Some(Language::Python),
        "javascript" | "js" => Some(Language::JavaScript),
        "typescript" | "ts" => Some(Language::TypeScript),
        "go" => Some(Language::Go),
        "c" => Some(Language::C),
        "cpp" | "cxx" | "cc" => Some(Language::Cpp),
        "java" => Some(Language::Java),
        "kotlin" | "kt" => Some(Language::Kotlin),
        "ruby" | "rb" => Some(Language::Ruby),
        "toml" => Some(Language::Toml),
        "yaml" | "yml" => Some(Language::Yaml),
        "json" => Some(Language::Json),
        "markdown" | "md" => Some(Language::Markdown),
        "shell" | "sh" | "bash" => Some(Language::Shell),
        "dockerfile" | "docker" => Some(Language::Dockerfile),
        _ => None,
    }
}
