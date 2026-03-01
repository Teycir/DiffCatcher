use std::path::{Path, PathBuf};

use crate::cli::{Cli, PullStrategy, SummaryFormat};
use crate::error::{PatrolError, Result};

const DEFAULT_WATCH_INTERVAL: u64 = 300;
const DEFAULT_TIMEOUT: u64 = 120;
const DEFAULT_HISTORY_DEPTH: u32 = 2;
const DEFAULT_PARALLEL: usize = 4;
const DEFAULT_BRANCH_FILTER: &str = "*";
const DEFAULT_SNIPPET_CONTEXT: u32 = 5;
const DEFAULT_MAX_SNIPPET_LINES: u32 = 200;
const DEFAULT_MAX_ELEMENTS: usize = 500;

#[derive(Debug, Clone, Default)]
struct PluginFileConfig {
    security_pattern_files: Vec<PathBuf>,
    extractor_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Default)]
struct FileConfig {
    output: Option<PathBuf>,
    watch: Option<bool>,
    watch_interval: Option<u64>,
    pull_strategy: Option<String>,
    timeout: Option<u64>,
    nested: Option<bool>,
    follow_symlinks: Option<bool>,
    skip_hidden: Option<bool>,
    pull: Option<bool>,
    force_pull: Option<bool>,
    no_pull: Option<bool>,
    history_depth: Option<u32>,
    parallel: Option<usize>,
    quiet: Option<bool>,
    verbose: Option<bool>,
    dry_run: Option<bool>,
    json_stdout: Option<bool>,
    branch_filter: Option<String>,
    no_summary_extraction: Option<bool>,
    no_snippets: Option<bool>,
    no_security_tags: Option<bool>,
    snippet_context: Option<u32>,
    max_snippet_lines: Option<u32>,
    max_elements: Option<usize>,
    summary_formats: Option<Vec<String>>,
    incremental: Option<bool>,
    security_tags_file: Option<PathBuf>,
    security_plugin_files: Vec<PathBuf>,
    extractor_plugin_files: Vec<PathBuf>,
    overwrite: Option<bool>,
    include_detached: Option<bool>,
    include_bare: Option<bool>,
    include_test_security: Option<bool>,
    include_vendor: Option<bool>,
    plugins: PluginFileConfig,
}

#[derive(Debug, Clone)]
enum ConfigValue {
    Bool(bool),
    Int(i64),
    Str(String),
    StrArray(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct RuntimeSettings {
    pub output: Option<PathBuf>,
    pub watch: bool,
    pub watch_interval: u64,
    pub pull_strategy: PullStrategy,
    pub timeout: u64,
    pub nested: bool,
    pub follow_symlinks: bool,
    pub skip_hidden: bool,
    pub pull: bool,
    pub force_pull: bool,
    pub no_pull: bool,
    pub history_depth: u32,
    pub parallel: usize,
    pub quiet: bool,
    pub verbose: bool,
    pub dry_run: bool,
    pub json_stdout: bool,
    pub branch_filter: String,
    pub no_summary_extraction: bool,
    pub no_snippets: bool,
    pub no_security_tags: bool,
    pub snippet_context: u32,
    pub max_snippet_lines: u32,
    pub max_elements: usize,
    pub summary_formats: Vec<SummaryFormat>,
    pub incremental: bool,
    pub security_tags_file: Option<PathBuf>,
    pub security_plugin_files: Vec<PathBuf>,
    pub extractor_plugin_files: Vec<PathBuf>,
    pub overwrite: bool,
    pub include_detached: bool,
    pub include_bare: bool,
    pub include_test_security: bool,
    pub include_vendor: bool,
}

pub fn resolve_runtime_settings(cli: &Cli, root_dir: &Path) -> Result<RuntimeSettings> {
    let loaded = load_file_config(cli, root_dir)?;
    let cfg = loaded.as_ref().map(|(_, cfg)| cfg);

    let output = if cli.output.is_some() {
        cli.output.clone()
    } else {
        cfg.and_then(|c| c.output.clone())
    };

    let pull_strategy = pick_with_default(
        cli.pull_strategy.clone(),
        PullStrategy::FfOnly,
        cfg.and_then(|c| c.pull_strategy.as_deref().map(parse_pull_strategy))
            .transpose()?,
    );

    let summary_formats = if cli.summary_formats != default_summary_formats() {
        cli.summary_formats.clone()
    } else if let Some(values) = cfg.and_then(|c| c.summary_formats.as_ref()) {
        parse_summary_formats(values)?
    } else {
        default_summary_formats()
    };

    let mut security_plugin_files = cfg
        .map(resolve_security_plugin_paths)
        .unwrap_or_else(Vec::new);
    security_plugin_files.extend(cli.security_plugin_files.clone());
    dedup_paths(&mut security_plugin_files);

    let mut extractor_plugin_files = cfg
        .map(resolve_extractor_plugin_paths)
        .unwrap_or_else(Vec::new);
    extractor_plugin_files.extend(cli.extractor_plugin_files.clone());
    dedup_paths(&mut extractor_plugin_files);

    let security_tags_file = if cli.security_tags_file.is_some() {
        cli.security_tags_file.clone()
    } else {
        cfg.and_then(|c| c.security_tags_file.clone())
    };

    let runtime = RuntimeSettings {
        output,
        watch: pick_with_default(cli.watch, false, cfg.and_then(|c| c.watch)),
        watch_interval: pick_with_default(
            cli.watch_interval,
            DEFAULT_WATCH_INTERVAL,
            cfg.and_then(|c| c.watch_interval),
        ),
        pull_strategy,
        timeout: pick_with_default(cli.timeout, DEFAULT_TIMEOUT, cfg.and_then(|c| c.timeout)),
        nested: pick_with_default(cli.nested, false, cfg.and_then(|c| c.nested)),
        follow_symlinks: pick_with_default(
            cli.follow_symlinks,
            false,
            cfg.and_then(|c| c.follow_symlinks),
        ),
        skip_hidden: pick_with_default(cli.skip_hidden, false, cfg.and_then(|c| c.skip_hidden)),
        pull: pick_with_default(cli.pull, false, cfg.and_then(|c| c.pull)),
        force_pull: pick_with_default(cli.force_pull, false, cfg.and_then(|c| c.force_pull)),
        no_pull: pick_with_default(cli.no_pull, false, cfg.and_then(|c| c.no_pull)),
        history_depth: pick_with_default(
            cli.history_depth,
            DEFAULT_HISTORY_DEPTH,
            cfg.and_then(|c| c.history_depth),
        ),
        parallel: pick_with_default(cli.parallel, DEFAULT_PARALLEL, cfg.and_then(|c| c.parallel)),
        quiet: pick_with_default(cli.quiet, false, cfg.and_then(|c| c.quiet)),
        verbose: pick_with_default(cli.verbose, false, cfg.and_then(|c| c.verbose)),
        dry_run: pick_with_default(cli.dry_run, false, cfg.and_then(|c| c.dry_run)),
        json_stdout: pick_with_default(cli.json_stdout, false, cfg.and_then(|c| c.json_stdout)),
        branch_filter: pick_with_default(
            cli.branch_filter.clone(),
            DEFAULT_BRANCH_FILTER.to_string(),
            cfg.and_then(|c| c.branch_filter.clone()),
        ),
        no_summary_extraction: pick_with_default(
            cli.no_summary_extraction,
            false,
            cfg.and_then(|c| c.no_summary_extraction),
        ),
        no_snippets: pick_with_default(cli.no_snippets, false, cfg.and_then(|c| c.no_snippets)),
        no_security_tags: pick_with_default(
            cli.no_security_tags,
            false,
            cfg.and_then(|c| c.no_security_tags),
        ),
        snippet_context: pick_with_default(
            cli.snippet_context,
            DEFAULT_SNIPPET_CONTEXT,
            cfg.and_then(|c| c.snippet_context),
        ),
        max_snippet_lines: pick_with_default(
            cli.max_snippet_lines,
            DEFAULT_MAX_SNIPPET_LINES,
            cfg.and_then(|c| c.max_snippet_lines),
        ),
        max_elements: pick_with_default(
            cli.max_elements,
            DEFAULT_MAX_ELEMENTS,
            cfg.and_then(|c| c.max_elements),
        ),
        summary_formats,
        incremental: pick_with_default(cli.incremental, false, cfg.and_then(|c| c.incremental)),
        security_tags_file,
        security_plugin_files,
        extractor_plugin_files,
        overwrite: pick_with_default(cli.overwrite, false, cfg.and_then(|c| c.overwrite)),
        include_detached: pick_with_default(
            cli.include_detached,
            false,
            cfg.and_then(|c| c.include_detached),
        ),
        include_bare: pick_with_default(cli.include_bare, false, cfg.and_then(|c| c.include_bare)),
        include_test_security: pick_with_default(
            cli.include_test_security,
            false,
            cfg.and_then(|c| c.include_test_security),
        ),
        include_vendor: pick_with_default(
            cli.include_vendor,
            false,
            cfg.and_then(|c| c.include_vendor),
        ),
    };

    validate_runtime_settings(&runtime)?;
    Ok(runtime)
}

fn load_file_config(cli: &Cli, root_dir: &Path) -> Result<Option<(PathBuf, FileConfig)>> {
    if cli.no_config {
        return Ok(None);
    }

    let config_path = if let Some(path) = &cli.config {
        path.clone()
    } else {
        let path = root_dir.join(".diffcatcher.toml");
        if path.exists() {
            path
        } else {
            return Ok(None);
        }
    };

    let raw = std::fs::read_to_string(&config_path)?;
    let mut cfg = parse_file_config(&raw, &config_path)?;

    let base = config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    relativize_paths(&mut cfg, &base);

    Ok(Some((config_path, cfg)))
}

fn parse_file_config(raw: &str, path: &Path) -> Result<FileConfig> {
    let mut cfg = FileConfig::default();
    let mut section = String::new();

    for (idx, original_line) in raw.lines().enumerate() {
        let line_no = idx + 1;
        let mut line = original_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(comment_idx) = line.find('#') {
            line = line[..comment_idx].trim();
            if line.is_empty() {
                continue;
            }
        }

        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_string();
            continue;
        }

        let (key, value_raw) = line.split_once('=').ok_or_else(|| {
            PatrolError::InvalidArgument(format!(
                "invalid config line {} in {}: expected key = value",
                line_no,
                path.display()
            ))
        })?;
        let key = key.trim();
        let value = parse_config_value(value_raw.trim(), path, line_no)?;
        assign_value(&mut cfg, &section, key, value, path, line_no)?;
    }

    Ok(cfg)
}

fn parse_config_value(raw: &str, path: &Path, line_no: usize) -> Result<ConfigValue> {
    if raw.starts_with('\"') && raw.ends_with('\"') && raw.len() >= 2 {
        return Ok(ConfigValue::Str(unquote(raw)));
    }
    if raw.eq_ignore_ascii_case("true") {
        return Ok(ConfigValue::Bool(true));
    }
    if raw.eq_ignore_ascii_case("false") {
        return Ok(ConfigValue::Bool(false));
    }
    if raw.starts_with('[') && raw.ends_with(']') {
        let inner = &raw[1..raw.len() - 1];
        let mut out = Vec::new();
        for part in inner.split(',') {
            let item = part.trim();
            if item.is_empty() {
                continue;
            }
            if item.starts_with('\"') && item.ends_with('\"') {
                out.push(unquote(item));
            } else {
                out.push(item.to_string());
            }
        }
        return Ok(ConfigValue::StrArray(out));
    }
    if let Ok(v) = raw.parse::<i64>() {
        return Ok(ConfigValue::Int(v));
    }

    Err(PatrolError::InvalidArgument(format!(
        "invalid config value '{}' at {}:{}",
        raw,
        path.display(),
        line_no
    )))
}

fn unquote(raw: &str) -> String {
    let trimmed = &raw[1..raw.len() - 1];
    trimmed
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
        .to_string()
}

fn expect_bool(value: ConfigValue, path: &Path, line_no: usize, key: &str) -> Result<bool> {
    match value {
        ConfigValue::Bool(v) => Ok(v),
        _ => Err(PatrolError::InvalidArgument(format!(
            "expected boolean for '{}' at {}:{}",
            key,
            path.display(),
            line_no
        ))),
    }
}

fn expect_u64(value: ConfigValue, path: &Path, line_no: usize, key: &str) -> Result<u64> {
    match value {
        ConfigValue::Int(v) if v >= 0 => Ok(v as u64),
        _ => Err(PatrolError::InvalidArgument(format!(
            "expected non-negative integer for '{}' at {}:{}",
            key,
            path.display(),
            line_no
        ))),
    }
}

fn expect_u32(value: ConfigValue, path: &Path, line_no: usize, key: &str) -> Result<u32> {
    let v = expect_u64(value, path, line_no, key)?;
    u32::try_from(v).map_err(|_| {
        PatrolError::InvalidArgument(format!(
            "value out of range for '{}' at {}:{}",
            key,
            path.display(),
            line_no
        ))
    })
}

fn expect_usize(value: ConfigValue, path: &Path, line_no: usize, key: &str) -> Result<usize> {
    let v = expect_u64(value, path, line_no, key)?;
    usize::try_from(v).map_err(|_| {
        PatrolError::InvalidArgument(format!(
            "value out of range for '{}' at {}:{}",
            key,
            path.display(),
            line_no
        ))
    })
}

fn expect_str(value: ConfigValue, path: &Path, line_no: usize, key: &str) -> Result<String> {
    match value {
        ConfigValue::Str(v) => Ok(v),
        _ => Err(PatrolError::InvalidArgument(format!(
            "expected string for '{}' at {}:{}",
            key,
            path.display(),
            line_no
        ))),
    }
}

fn expect_str_array(
    value: ConfigValue,
    path: &Path,
    line_no: usize,
    key: &str,
) -> Result<Vec<String>> {
    match value {
        ConfigValue::StrArray(v) => Ok(v),
        _ => Err(PatrolError::InvalidArgument(format!(
            "expected string array for '{}' at {}:{}",
            key,
            path.display(),
            line_no
        ))),
    }
}

fn assign_value(
    cfg: &mut FileConfig,
    section: &str,
    key: &str,
    value: ConfigValue,
    path: &Path,
    line_no: usize,
) -> Result<()> {
    match (section, key) {
        ("", "output") => cfg.output = Some(PathBuf::from(expect_str(value, path, line_no, key)?)),
        ("", "watch") => cfg.watch = Some(expect_bool(value, path, line_no, key)?),
        ("", "watch_interval") => cfg.watch_interval = Some(expect_u64(value, path, line_no, key)?),
        ("", "pull_strategy") => {
            cfg.pull_strategy = Some(expect_str(value, path, line_no, key)?);
        }
        ("", "timeout") => cfg.timeout = Some(expect_u64(value, path, line_no, key)?),
        ("", "nested") => cfg.nested = Some(expect_bool(value, path, line_no, key)?),
        ("", "follow_symlinks") => {
            cfg.follow_symlinks = Some(expect_bool(value, path, line_no, key)?)
        }
        ("", "skip_hidden") => cfg.skip_hidden = Some(expect_bool(value, path, line_no, key)?),
        ("", "pull") => cfg.pull = Some(expect_bool(value, path, line_no, key)?),
        ("", "force_pull") => cfg.force_pull = Some(expect_bool(value, path, line_no, key)?),
        ("", "no_pull") => cfg.no_pull = Some(expect_bool(value, path, line_no, key)?),
        ("", "history_depth") => cfg.history_depth = Some(expect_u32(value, path, line_no, key)?),
        ("", "parallel") => cfg.parallel = Some(expect_usize(value, path, line_no, key)?),
        ("", "quiet") => cfg.quiet = Some(expect_bool(value, path, line_no, key)?),
        ("", "verbose") => cfg.verbose = Some(expect_bool(value, path, line_no, key)?),
        ("", "dry_run") => cfg.dry_run = Some(expect_bool(value, path, line_no, key)?),
        ("", "json_stdout") => cfg.json_stdout = Some(expect_bool(value, path, line_no, key)?),
        ("", "branch_filter") => {
            cfg.branch_filter = Some(expect_str(value, path, line_no, key)?);
        }
        ("", "no_summary_extraction") => {
            cfg.no_summary_extraction = Some(expect_bool(value, path, line_no, key)?)
        }
        ("", "no_snippets") => cfg.no_snippets = Some(expect_bool(value, path, line_no, key)?),
        ("", "no_security_tags") => {
            cfg.no_security_tags = Some(expect_bool(value, path, line_no, key)?)
        }
        ("", "snippet_context") => {
            cfg.snippet_context = Some(expect_u32(value, path, line_no, key)?)
        }
        ("", "max_snippet_lines") => {
            cfg.max_snippet_lines = Some(expect_u32(value, path, line_no, key)?)
        }
        ("", "max_elements") => cfg.max_elements = Some(expect_usize(value, path, line_no, key)?),
        ("", "summary_formats") => {
            cfg.summary_formats = Some(expect_str_array(value, path, line_no, key)?)
        }
        ("", "incremental") => cfg.incremental = Some(expect_bool(value, path, line_no, key)?),
        ("", "security_tags_file") => {
            cfg.security_tags_file = Some(PathBuf::from(expect_str(value, path, line_no, key)?))
        }
        ("", "security_plugin_files") => {
            cfg.security_plugin_files = expect_str_array(value, path, line_no, key)?
                .into_iter()
                .map(PathBuf::from)
                .collect();
        }
        ("", "extractor_plugin_files") => {
            cfg.extractor_plugin_files = expect_str_array(value, path, line_no, key)?
                .into_iter()
                .map(PathBuf::from)
                .collect();
        }
        ("", "overwrite") => cfg.overwrite = Some(expect_bool(value, path, line_no, key)?),
        ("", "include_detached") => {
            cfg.include_detached = Some(expect_bool(value, path, line_no, key)?)
        }
        ("", "include_bare") => cfg.include_bare = Some(expect_bool(value, path, line_no, key)?),
        ("", "include_test_security") => {
            cfg.include_test_security = Some(expect_bool(value, path, line_no, key)?)
        }
        ("", "include_vendor") => {
            cfg.include_vendor = Some(expect_bool(value, path, line_no, key)?)
        }
        ("plugins", "security_pattern_files") => {
            cfg.plugins.security_pattern_files = expect_str_array(value, path, line_no, key)?
                .into_iter()
                .map(PathBuf::from)
                .collect();
        }
        ("plugins", "extractor_files") => {
            cfg.plugins.extractor_files = expect_str_array(value, path, line_no, key)?
                .into_iter()
                .map(PathBuf::from)
                .collect();
        }
        _ => {
            return Err(PatrolError::InvalidArgument(format!(
                "unknown config key '{}' in section '{}' at {}:{}",
                key,
                section,
                path.display(),
                line_no
            )));
        }
    }
    Ok(())
}

fn relativize_paths(cfg: &mut FileConfig, base: &Path) {
    if let Some(path) = cfg.output.clone() {
        cfg.output = Some(resolve_path(base, &path));
    }
    if let Some(path) = cfg.security_tags_file.clone() {
        cfg.security_tags_file = Some(resolve_path(base, &path));
    }
    cfg.security_plugin_files = cfg
        .security_plugin_files
        .iter()
        .map(|p| resolve_path(base, p))
        .collect();
    cfg.extractor_plugin_files = cfg
        .extractor_plugin_files
        .iter()
        .map(|p| resolve_path(base, p))
        .collect();
    cfg.plugins.security_pattern_files = cfg
        .plugins
        .security_pattern_files
        .iter()
        .map(|p| resolve_path(base, p))
        .collect();
    cfg.plugins.extractor_files = cfg
        .plugins
        .extractor_files
        .iter()
        .map(|p| resolve_path(base, p))
        .collect();
}

fn resolve_path(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

fn resolve_security_plugin_paths(cfg: &FileConfig) -> Vec<PathBuf> {
    let mut paths = cfg.security_plugin_files.clone();
    paths.extend(cfg.plugins.security_pattern_files.clone());
    dedup_paths(&mut paths);
    paths
}

fn resolve_extractor_plugin_paths(cfg: &FileConfig) -> Vec<PathBuf> {
    let mut paths = cfg.extractor_plugin_files.clone();
    paths.extend(cfg.plugins.extractor_files.clone());
    dedup_paths(&mut paths);
    paths
}

fn dedup_paths(paths: &mut Vec<PathBuf>) {
    let mut seen = std::collections::BTreeSet::new();
    paths.retain(|p| seen.insert(p.clone()));
}

fn parse_pull_strategy(raw: &str) -> Result<PullStrategy> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "ff-only" | "ffonly" => Ok(PullStrategy::FfOnly),
        "rebase" => Ok(PullStrategy::Rebase),
        "merge" => Ok(PullStrategy::Merge),
        other => Err(PatrolError::InvalidArgument(format!(
            "invalid pull_strategy '{}' in config (expected ff-only|rebase|merge)",
            other
        ))),
    }
}

fn parse_summary_formats(raw: &[String]) -> Result<Vec<SummaryFormat>> {
    let mut out = Vec::new();
    for item in raw {
        let fmt = match item.trim().to_ascii_lowercase().as_str() {
            "json" => SummaryFormat::Json,
            "txt" | "text" => SummaryFormat::Txt,
            "md" | "markdown" => SummaryFormat::Md,
            "sarif" => SummaryFormat::Sarif,
            other => {
                return Err(PatrolError::InvalidArgument(format!(
                    "invalid summary format '{}' in config",
                    other
                )));
            }
        };
        if !out.contains(&fmt) {
            out.push(fmt);
        }
    }
    if out.is_empty() {
        return Ok(default_summary_formats());
    }
    Ok(out)
}

fn default_summary_formats() -> Vec<SummaryFormat> {
    vec![SummaryFormat::Json, SummaryFormat::Md]
}

fn pick_with_default<T>(cli_value: T, default: T, cfg_value: Option<T>) -> T
where
    T: Clone + PartialEq,
{
    if cli_value != default {
        cli_value
    } else {
        cfg_value.unwrap_or(default)
    }
}

fn validate_runtime_settings(settings: &RuntimeSettings) -> Result<()> {
    if settings.history_depth == 0 {
        return Err(PatrolError::InvalidArgument(
            "--history-depth must be >= 1".to_string(),
        ));
    }
    if settings.history_depth > 10 {
        return Err(PatrolError::InvalidArgument(
            "--history-depth must be <= 10".to_string(),
        ));
    }
    if settings.parallel == 0 {
        return Err(PatrolError::InvalidArgument(
            "--parallel must be >= 1".to_string(),
        ));
    }
    if settings.watch && settings.watch_interval == 0 {
        return Err(PatrolError::InvalidArgument(
            "--watch-interval must be >= 1 when --watch is enabled".to_string(),
        ));
    }
    if settings.force_pull && !settings.pull {
        return Err(PatrolError::InvalidArgument(
            "--force-pull requires --pull".to_string(),
        ));
    }
    if settings.pull && settings.no_pull {
        return Err(PatrolError::InvalidArgument(
            "--pull and --no-pull are mutually exclusive".to_string(),
        ));
    }

    Ok(())
}
