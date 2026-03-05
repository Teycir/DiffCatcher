use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use clap::Parser;
use rayon::prelude::*;
use tracing::info;
use tracing_subscriber::EnvFilter;

use diffcatcher::cli::{Cli, SummaryFormat};
use diffcatcher::config::{RuntimeSettings, resolve_runtime_settings};
use diffcatcher::error::{PatrolError, Result};
use diffcatcher::extraction::ExtractionOptions;
use diffcatcher::extraction::plugins::{ExtractorPlugin, load_extractor_plugins};
use diffcatcher::git::commands::run_git_expect_stdout;
use diffcatcher::processor::{
    DiffRefsConfig, ProcessorConfig, process_diff_refs, process_repository,
};
use diffcatcher::progress::{ProgressReporter, Verbosity};
use diffcatcher::report::writer::{prepare_report_dir, write_repo_report, write_top_level_reports};
use diffcatcher::scanner::{ScanOptions, discover_repositories};
use diffcatcher::security::{load_tag_definitions, overview::build_global_security_overview};
use diffcatcher::types::{GlobalSummary, RepoResult, RepoStatus};

fn main() {
    let exit_code = match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("fatal: {}", err);
            1
        }
    };

    std::process::exit(exit_code);
}

fn run() -> Result<i32> {
    init_tracing();

    let cli = Cli::parse();
    cli.validate().map_err(PatrolError::InvalidArgument)?;
    ensure_git_available()?;

    let root_dir = cli.root_dir.clone().unwrap_or_else(|| PathBuf::from("."));
    if !root_dir.exists() {
        return Err(PatrolError::MissingRoot(root_dir.clone()));
    }

    let settings = resolve_runtime_settings(&cli, &root_dir)?;
    let tag_definitions = load_tag_definitions(
        settings.security_tags_file.as_deref(),
        &settings.security_plugin_files,
    )?;
    let extractor_plugins = load_extractor_plugins(&settings.extractor_plugin_files)?;

    if settings.watch {
        return run_watch_mode(
            &cli,
            &settings,
            &root_dir,
            &tag_definitions,
            &extractor_plugins,
        );
    }

    run_once(
        &cli,
        &settings,
        &root_dir,
        &tag_definitions,
        &extractor_plugins,
    )
}

fn run_watch_mode(
    cli: &Cli,
    settings: &RuntimeSettings,
    root_dir: &Path,
    tag_definitions: &[diffcatcher::types::SecurityTagDefinition],
    extractor_plugins: &[ExtractorPlugin],
) -> Result<i32> {
    loop {
        let exit_code = match run_once(cli, settings, root_dir, tag_definitions, extractor_plugins)
        {
            Ok(code) => code,
            Err(err) => {
                eprintln!("watch iteration failed: {}", err);
                1
            }
        };

        if !settings.quiet {
            println!(
                "Watch iteration finished with exit code {}. Sleeping {}s before next run.",
                exit_code, settings.watch_interval
            );
        }
        thread::sleep(Duration::from_secs(settings.watch_interval));
    }
}

fn run_once(
    cli: &Cli,
    settings: &RuntimeSettings,
    root_dir: &Path,
    tag_definitions: &[diffcatcher::types::SecurityTagDefinition],
    extractor_plugins: &[ExtractorPlugin],
) -> Result<i32> {
    let report_dir = prepare_report_dir(settings.output.as_deref(), settings.overwrite)?;

    // Branch-diff mode: diff two refs in a single repo.
    if let Some((base, head)) = cli.parsed_diff_refs() {
        return run_diff_mode(
            settings,
            root_dir,
            &report_dir,
            tag_definitions,
            extractor_plugins,
            base,
            head,
        );
    }

    // Standard scan mode.
    let scan_options = ScanOptions {
        nested: settings.nested,
        follow_symlinks: settings.follow_symlinks,
        skip_hidden: settings.skip_hidden,
        include_bare: settings.include_bare,
    };

    let mut repos = discover_repositories(root_dir, &scan_options)?;
    repos.sort();

    if settings.verbose && !settings.quiet {
        for repo in &repos {
            println!("{}", repo.display());
        }
    }

    if settings.incremental {
        repos = filter_incremental_repos(&report_dir, repos, settings.timeout)?;
    }

    info!("discovered {} repositories", repos.len());

    let extraction = ExtractionOptions {
        no_summary_extraction: settings.no_summary_extraction,
        no_snippets: settings.no_snippets,
        snippet_context: settings.snippet_context,
        max_snippet_lines: settings.max_snippet_lines,
        max_elements: settings.max_elements,
        include_vendor: settings.include_vendor,
        plugin_extractors: extractor_plugins.to_vec(),
    };

    let processor_cfg = ProcessorConfig {
        root_dir: root_dir.to_path_buf(),
        report_dir: report_dir.clone(),
        timeout_secs: settings.timeout,
        pull_mode: settings.pull && !settings.no_pull,
        force_pull: settings.force_pull,
        pull_strategy: settings.pull_strategy.clone(),
        no_pull: settings.no_pull,
        dry_run: settings.dry_run,
        history_depth: settings.history_depth,
        branch_filter: settings.branch_filter.clone(),
        extraction,
        no_security_tags: settings.no_security_tags,
        include_detached: settings.include_detached,
        include_test_security: settings.include_test_security,
        tag_definitions: tag_definitions.to_vec(),
        verbose: settings.verbose,
    };

    rayon::ThreadPoolBuilder::new()
        .num_threads(settings.parallel)
        .build_global()
        .ok();

    let verbosity = if settings.json_stdout {
        Verbosity::Json
    } else if settings.quiet {
        Verbosity::Quiet
    } else if settings.verbose {
        Verbosity::Verbose
    } else {
        Verbosity::Default
    };

    let reporter = Arc::new(ProgressReporter::new(
        repos.len() as u32,
        verbosity,
        settings.parallel,
    ));
    let reporter_for_workers = reporter.clone();

    let mut results: Vec<RepoResult> = repos
        .par_iter()
        .map(|repo_path| {
            let repo_name = repo_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("repo")
                .to_string();
            reporter_for_workers.repo_started(&repo_name);
            let start = Instant::now();
            let cb = |name: &str, state| {
                reporter_for_workers.repo_state_changed(name, state);
            };
            let result = process_repository(repo_path, &processor_cfg, Some(&cb));
            reporter_for_workers.repo_completed(&result, start.elapsed());
            result
        })
        .collect();
    reporter.finish();

    results.sort_by(|a, b| a.report_folder_name.cmp(&b.report_folder_name));

    results
        .par_iter_mut()
        .try_for_each(|repo| write_repo_report(&report_dir, repo, &settings.summary_formats))?;

    let summary = GlobalSummary::from_results(root_dir.to_path_buf(), report_dir.clone(), &results);
    let security_overview = if settings.no_security_tags {
        None
    } else {
        Some(build_global_security_overview(&results))
    };

    write_top_level_reports(&report_dir, &summary, security_overview.as_ref())?;
    write_sarif_if_requested(
        &settings.summary_formats,
        &report_dir,
        &results,
        tag_definitions,
    )?;
    persist_incremental_state(&report_dir, &results)?;

    let exit_code = exit_code_for_results(&results);

    if settings.json_stdout {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    }

    reporter.print_summary(&summary, &results, &report_dir, exit_code);

    Ok(exit_code)
}

fn run_diff_mode(
    settings: &RuntimeSettings,
    repo_path: &Path,
    report_dir: &Path,
    tag_definitions: &[diffcatcher::types::SecurityTagDefinition],
    extractor_plugins: &[ExtractorPlugin],
    base: &str,
    head: &str,
) -> Result<i32> {
    let extraction = ExtractionOptions {
        no_summary_extraction: settings.no_summary_extraction,
        no_snippets: settings.no_snippets,
        snippet_context: settings.snippet_context,
        max_snippet_lines: settings.max_snippet_lines,
        max_elements: settings.max_elements,
        include_vendor: settings.include_vendor,
        plugin_extractors: extractor_plugins.to_vec(),
    };

    let config = DiffRefsConfig {
        timeout_secs: settings.timeout,
        extraction: &extraction,
        no_security_tags: settings.no_security_tags,
        include_test_security: settings.include_test_security,
        tag_definitions,
        verbose: settings.verbose,
    };

    let mut result = process_diff_refs(repo_path, report_dir, base, head, &config);

    write_repo_report(report_dir, &mut result, &settings.summary_formats)?;

    let results = vec![result];
    let summary =
        GlobalSummary::from_results(repo_path.to_path_buf(), report_dir.to_path_buf(), &results);
    let security_overview = if settings.no_security_tags {
        None
    } else {
        Some(build_global_security_overview(&results))
    };

    write_top_level_reports(report_dir, &summary, security_overview.as_ref())?;
    write_sarif_if_requested(
        &settings.summary_formats,
        report_dir,
        &results,
        tag_definitions,
    )?;

    if settings.json_stdout {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else if !settings.quiet {
        let sec_count = summary.total_security_tagged_elements;
        println!(
            "DiffCatcher diff complete: {}..{}, {} security-tagged elements. Report: {}",
            base,
            head,
            sec_count,
            report_dir.display(),
        );
    }

    Ok(exit_code_for_results(&results))
}

fn write_sarif_if_requested(
    summary_formats: &[SummaryFormat],
    report_dir: &Path,
    results: &[RepoResult],
    tag_definitions: &[diffcatcher::types::SecurityTagDefinition],
) -> Result<()> {
    if !summary_formats.contains(&SummaryFormat::Sarif) {
        return Ok(());
    }

    let sarif = diffcatcher::report::sarif::build_sarif_from_results(results, tag_definitions);
    let content = serde_json::to_string_pretty(&sarif)?;
    fs::write(report_dir.join("results.sarif"), content)?;
    Ok(())
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init();
}

fn exit_code_for_results(results: &[RepoResult]) -> i32 {
    let partial = results.iter().any(|repo| {
        matches!(
            repo.status,
            RepoStatus::FetchFailed { .. } | RepoStatus::PullFailed { .. }
        )
    });

    if partial { 2 } else { 0 }
}

fn persist_incremental_state(report_dir: &Path, repos: &[RepoResult]) -> Result<()> {
    let mut state = BTreeMap::new();
    for repo in repos {
        if let Some(post) = &repo.post_pull {
            state.insert(repo.repo_path.display().to_string(), post.hash.clone());
        }
    }

    let content = serde_json::to_string_pretty(&state)?;
    fs::write(report_dir.join(".diffcatcher-state.json"), content)?;
    Ok(())
}

fn filter_incremental_repos(
    report_dir: &Path,
    repos: Vec<PathBuf>,
    timeout_secs: u64,
) -> Result<Vec<PathBuf>> {
    let state_path = report_dir.join(".diffcatcher-state.json");
    if !state_path.exists() {
        return Ok(repos);
    }

    let raw = fs::read_to_string(state_path)?;
    let previous: BTreeMap<String, String> = serde_json::from_str(&raw)?;

    let filtered: Vec<PathBuf> = repos
        .into_par_iter()
        .filter(|repo| {
            let key = repo.display().to_string();
            let current_head = run_git_expect_stdout(repo, timeout_secs, &["rev-parse", "HEAD"])
                .unwrap_or_default();
            previous
                .get(&key)
                .is_none_or(|last_hash| last_hash != &current_head)
        })
        .collect();

    Ok(filtered)
}

fn ensure_git_available() -> Result<()> {
    let status = Command::new("git")
        .arg("--version")
        .status()
        .map_err(|err| PatrolError::InvalidArgument(format!("git not found on PATH: {}", err)))?;

    if status.success() {
        Ok(())
    } else {
        Err(PatrolError::InvalidArgument(
            "git command is available but failed to execute".to_string(),
        ))
    }
}
