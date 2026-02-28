use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;
use rayon::prelude::*;
use tracing::info;
use tracing_subscriber::EnvFilter;

use git_patrol::cli::Cli;
use git_patrol::error::{PatrolError, Result};
use git_patrol::extraction::ExtractionOptions;
use git_patrol::git::commands::run_git_expect_stdout;
use git_patrol::processor::{ProcessorConfig, process_repository};
use git_patrol::report::writer::{prepare_report_dir, write_repo_report, write_top_level_reports};
use git_patrol::scanner::{ScanOptions, discover_repositories};
use git_patrol::security::{load_tag_definitions, overview::build_global_security_overview};
use git_patrol::types::{GlobalSummary, RepoResult, RepoStatus};

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

    if !cli.root_dir.exists() {
        return Err(PatrolError::MissingRoot(cli.root_dir.clone()));
    }

    let report_dir = prepare_report_dir(cli.output.as_deref(), cli.overwrite)?;
    let tag_definitions = load_tag_definitions(cli.security_tags_file.as_deref())?;

    let scan_options = ScanOptions {
        nested: cli.nested,
        follow_symlinks: cli.follow_symlinks,
        skip_hidden: cli.skip_hidden,
        include_bare: cli.include_bare,
    };

    let mut repos = discover_repositories(&cli.root_dir, &scan_options)?;
    repos.sort();

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
        root_dir: cli.root_dir.clone(),
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
        tag_definitions,
        verbose: cli.verbose,
    };

    rayon::ThreadPoolBuilder::new()
        .num_threads(cli.parallel)
        .build_global()
        .ok();

    let mut results: Vec<RepoResult> = repos
        .par_iter()
        .map(|repo_path| process_repository(repo_path, &processor_cfg))
        .collect();

    results.sort_by(|a, b| a.report_folder_name.cmp(&b.report_folder_name));

    for repo in &mut results {
        write_repo_report(&report_dir, repo, &cli.summary_formats)?;
    }

    let summary = GlobalSummary::from_results(cli.root_dir.clone(), report_dir.clone(), &results);
    let security_overview = build_global_security_overview(&results);

    write_top_level_reports(&report_dir, &summary, &security_overview)?;
    persist_incremental_state(&report_dir, &results)?;

    if cli.json_stdout {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else if !cli.quiet {
        println!(
            "Git Patrol complete: {} repos scanned, {} updated, {} security-tagged elements. Report: {}",
            summary.total_repos_found,
            summary.updated,
            summary.total_security_tagged_elements,
            report_dir.display(),
        );
    }

    Ok(exit_code_for_results(&results))
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
    fs::write(report_dir.join(".git-patrol-state.json"), content)?;
    Ok(())
}

fn filter_incremental_repos(
    report_dir: &Path,
    repos: Vec<PathBuf>,
    timeout_secs: u64,
) -> Result<Vec<PathBuf>> {
    let state_path = report_dir.join(".git-patrol-state.json");
    if !state_path.exists() {
        return Ok(repos);
    }

    let raw = fs::read_to_string(state_path)?;
    let previous: BTreeMap<String, String> = serde_json::from_str(&raw)?;

    let mut filtered = Vec::new();
    for repo in repos {
        let key = repo.display().to_string();
        let current_head =
            run_git_expect_stdout(&repo, timeout_secs, &["rev-parse", "HEAD"]).unwrap_or_default();

        if previous
            .get(&key)
            .is_some_and(|last_hash| last_hash == &current_head)
        {
            continue;
        }

        filtered.push(repo);
    }

    Ok(filtered)
}
