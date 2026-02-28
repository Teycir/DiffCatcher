use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use tracing::info;
use tracing_subscriber::EnvFilter;

use diffcatcher::cli::{Cli, SummaryFormat};
use diffcatcher::error::{PatrolError, Result};
use diffcatcher::extraction::ExtractionOptions;
use diffcatcher::git::commands::run_git_expect_stdout;
use diffcatcher::processor::{ProcessorConfig, process_repository, process_diff_refs};
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

    let report_dir = prepare_report_dir(cli.output.as_deref(), cli.overwrite)?;
    let tag_definitions = load_tag_definitions(cli.security_tags_file.as_deref())?;

    // Branch-diff mode: diff two refs in a single repo.
    if let Some((base, head)) = cli.parsed_diff_refs() {
        return run_diff_mode(&cli, &root_dir, &report_dir, &tag_definitions, base, head);
    }

    // Standard scan mode.
    let scan_options = ScanOptions {
        nested: cli.nested,
        follow_symlinks: cli.follow_symlinks,
        skip_hidden: cli.skip_hidden,
        include_bare: cli.include_bare,
    };

    let mut repos = discover_repositories(&root_dir, &scan_options)?;
    repos.sort();

    if cli.verbose && !cli.quiet {
        for repo in &repos {
            println!("{}", repo.display());
        }
    }

    if cli.incremental {
        repos = filter_incremental_repos(&report_dir, repos, cli.timeout)?;
    }

    info!("discovered {} repositories", repos.len());

    let extraction = ExtractionOptions {
        no_summary_extraction: cli.no_summary_extraction,
        no_snippets: cli.no_snippets,
        snippet_context: cli.snippet_context,
        max_snippet_lines: cli.max_snippet_lines,
        max_elements: cli.max_elements,
    };

    let processor_cfg = ProcessorConfig {
        root_dir: root_dir.clone(),
        report_dir: report_dir.clone(),
        timeout_secs: cli.timeout,
        pull_mode: cli.effective_pull_mode(),
        force_pull: cli.force_pull,
        pull_strategy: cli.pull_strategy.clone(),
        no_pull: cli.no_pull,
        dry_run: cli.dry_run,
        history_depth: cli.history_depth,
        branch_filter: cli.branch_filter.clone(),
        extraction,
        no_security_tags: cli.no_security_tags,
        include_detached: cli.include_detached,
        include_test_security: cli.include_test_security,
        tag_definitions: tag_definitions.clone(),
        verbose: cli.verbose,
    };

    rayon::ThreadPoolBuilder::new()
        .num_threads(cli.parallel)
        .build_global()
        .ok();

    let progress = build_progress_bar(repos.len() as u64, cli.quiet);
    let progress_for_workers = progress.clone();
    let mut results: Vec<RepoResult> = repos
        .par_iter()
        .map(|repo_path| {
            let result = process_repository(repo_path, &processor_cfg);
            if let Some(pb) = &progress_for_workers {
                pb.inc(1);
            }
            result
        })
        .collect();
    if let Some(pb) = &progress {
        pb.finish_and_clear();
    }

    results.sort_by(|a, b| a.report_folder_name.cmp(&b.report_folder_name));

    results.par_iter_mut().try_for_each(|repo| {
        write_repo_report(&report_dir, repo, &cli.summary_formats)
    })?;

    let summary = GlobalSummary::from_results(root_dir.clone(), report_dir.clone(), &results);
    let security_overview = if cli.no_security_tags {
        None
    } else {
        Some(build_global_security_overview(&results))
    };

    write_top_level_reports(&report_dir, &summary, security_overview.as_ref())?;
    write_sarif_if_requested(&cli, &report_dir, &results, &tag_definitions)?;
    persist_incremental_state(&report_dir, &results)?;

    if cli.json_stdout {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else if !cli.quiet {
        println!(
            "DiffCatcher complete: {} repos scanned, {} updated, {} security-tagged elements. Report: {}",
            summary.total_repos_found,
            summary.updated,
            summary.total_security_tagged_elements,
            report_dir.display(),
        );
    }

    Ok(exit_code_for_results(&results))
}

fn run_diff_mode(
    cli: &Cli,
    repo_path: &Path,
    report_dir: &Path,
    tag_definitions: &[diffcatcher::types::SecurityTagDefinition],
    base: &str,
    head: &str,
) -> Result<i32> {
    let extraction = ExtractionOptions {
        no_summary_extraction: cli.no_summary_extraction,
        no_snippets: cli.no_snippets,
        snippet_context: cli.snippet_context,
        max_snippet_lines: cli.max_snippet_lines,
        max_elements: cli.max_elements,
    };

    let mut result = process_diff_refs(
        repo_path,
        report_dir,
        base,
        head,
        cli.timeout,
        &extraction,
        cli.no_security_tags,
        cli.include_test_security,
        tag_definitions,
        cli.verbose,
    );

    write_repo_report(report_dir, &mut result, &cli.summary_formats)?;

    let results = vec![result];
    let summary = GlobalSummary::from_results(
        repo_path.to_path_buf(),
        report_dir.to_path_buf(),
        &results,
    );
    let security_overview = if cli.no_security_tags {
        None
    } else {
        Some(build_global_security_overview(&results))
    };

    write_top_level_reports(report_dir, &summary, security_overview.as_ref())?;
    write_sarif_if_requested(cli, report_dir, &results, tag_definitions)?;

    if cli.json_stdout {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else if !cli.quiet {
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
    cli: &Cli,
    report_dir: &Path,
    results: &[RepoResult],
    tag_definitions: &[diffcatcher::types::SecurityTagDefinition],
) -> Result<()> {
    if !cli.summary_formats.contains(&SummaryFormat::Sarif) {
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
            let current_head =
                run_git_expect_stdout(repo, timeout_secs, &["rev-parse", "HEAD"])
                    .unwrap_or_default();
            previous
                .get(&key)
                .is_none_or(|last_hash| last_hash != &current_head)
        })
        .collect();

    Ok(filtered)
}

fn build_progress_bar(total: u64, quiet: bool) -> Option<ProgressBar> {
    if quiet {
        return None;
    }

    let pb = ProgressBar::new(total);
    let style = ProgressStyle::with_template(
        "[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} repos ({eta})",
    )
    .unwrap_or_else(|_| ProgressStyle::default_bar());
    pb.set_style(style.progress_chars("##-"));
    Some(pb)
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
