use std::path::PathBuf;

use clap::{ArgAction, Parser, ValueEnum};

#[derive(Debug, Clone, ValueEnum)]
pub enum PullStrategy {
    FfOnly,
    Rebase,
    Merge,
}

impl PullStrategy {
    pub fn as_git_flag(&self) -> &'static str {
        match self {
            Self::FfOnly => "--ff-only",
            Self::Rebase => "--rebase",
            Self::Merge => "--no-rebase",
        }
    }
}

#[derive(Debug, Clone, ValueEnum, PartialEq, Eq)]
pub enum SummaryFormat {
    Json,
    Txt,
    Md,
    Sarif,
}

#[derive(Debug, Parser)]
#[command(
    name = "diffcatcher",
    version,
    about = "Scan git repositories and produce security-focused diff reports"
)]
pub struct Cli {
    #[arg(
        value_name = "ROOT_DIR",
        help = "Directory to scan recursively (not required with --diff)"
    )]
    pub root_dir: Option<PathBuf>,

    #[arg(
        long = "diff",
        value_name = "BASE..HEAD",
        help = "Diff two refs in a single repo (e.g. main..feature, abc123..def456). Use ROOT_DIR as the repo path."
    )]
    pub diff_refs: Option<String>,

    #[arg(
        short = 'o',
        long = "output",
        value_name = "DIR",
        help = "Report output directory (default: ./reports/<timestamp>)"
    )]
    pub output: Option<PathBuf>,

    #[arg(
        short = 's',
        long = "pull-strategy",
        value_enum,
        default_value_t = PullStrategy::FfOnly,
        help = "Pull strategy: ff-only, rebase, merge"
    )]
    pub pull_strategy: PullStrategy,

    #[arg(
        short = 't',
        long = "timeout",
        default_value_t = 120,
        help = "Timeout per repo for git operations (seconds)"
    )]
    pub timeout: u64,

    #[arg(long, action = ArgAction::SetTrue, help = "Recurse into repos to find nested repos")]
    pub nested: bool,

    #[arg(
        long = "follow-symlinks",
        action = ArgAction::SetTrue,
        help = "Follow symbolic links during scan"
    )]
    pub follow_symlinks: bool,

    #[arg(
        long = "skip-hidden",
        action = ArgAction::SetTrue,
        help = "Skip hidden directories (dot-prefixed) except .git"
    )]
    pub skip_hidden: bool,

    #[arg(
        long = "pull",
        action = ArgAction::SetTrue,
        help = "Actually pull (modify working tree) instead of fetch-only"
    )]
    pub pull: bool,

    #[arg(
        long = "force-pull",
        action = ArgAction::SetTrue,
        help = "Stash dirty repos before pull, pop after (requires --pull)"
    )]
    pub force_pull: bool,

    #[arg(
        long = "no-pull",
        action = ArgAction::SetTrue,
        help = "Skip fetching/pulling; only capture state and generate historical diffs"
    )]
    pub no_pull: bool,

    #[arg(
        short = 'd',
        long = "history-depth",
        default_value_t = 2,
        help = "Number of historical commits to diff (min 1, max 10)"
    )]
    pub history_depth: u32,

    #[arg(
        short = 'j',
        long = "parallel",
        default_value_t = 4,
        help = "Number of repos to process concurrently"
    )]
    pub parallel: usize,

    #[arg(
        short = 'q',
        long = "quiet",
        action = ArgAction::SetTrue,
        help = "Suppress stdout progress; only write report files"
    )]
    pub quiet: bool,

    #[arg(
        short = 'v',
        long = "verbose",
        action = ArgAction::SetTrue,
        help = "Print detailed processing output to terminal"
    )]
    pub verbose: bool,

    #[arg(
        long = "dry-run",
        action = ArgAction::SetTrue,
        help = "Discover repos and report state; do not pull or modify anything"
    )]
    pub dry_run: bool,

    #[arg(
        long = "json",
        action = ArgAction::SetTrue,
        help = "Print final summary to stdout as JSON (for piping)"
    )]
    pub json_stdout: bool,

    #[arg(
        long = "branch-filter",
        default_value = "*",
        help = "Only process repos on branches matching glob pattern"
    )]
    pub branch_filter: String,

    #[arg(
        long = "no-summary-extraction",
        action = ArgAction::SetTrue,
        help = "Skip element extraction; only produce raw diffs and file lists"
    )]
    pub no_summary_extraction: bool,

    #[arg(
        long = "no-snippets",
        action = ArgAction::SetTrue,
        help = "Extract elements but do not capture code snippets"
    )]
    pub no_snippets: bool,

    #[arg(
        long = "no-security-tags",
        action = ArgAction::SetTrue,
        help = "Skip security pattern tagging"
    )]
    pub no_security_tags: bool,

    #[arg(
        long = "snippet-context",
        default_value_t = 5,
        help = "Lines of context above/below changed lines in snippets"
    )]
    pub snippet_context: u32,

    #[arg(
        long = "max-snippet-lines",
        default_value_t = 200,
        help = "Max lines per individual snippet"
    )]
    pub max_snippet_lines: u32,

    #[arg(
        long = "max-elements",
        default_value_t = 500,
        help = "Max elements to extract per diff (safety cap)"
    )]
    pub max_elements: usize,

    #[arg(
        long = "summary-format",
        value_delimiter = ',',
        default_values = ["json", "md"],
        help = "Comma-separated list of summary formats to generate"
    )]
    pub summary_formats: Vec<SummaryFormat>,

    #[arg(
        long = "incremental",
        action = ArgAction::SetTrue,
        help = "Skip repos unchanged since the last run"
    )]
    pub incremental: bool,

    #[arg(
        long = "security-tags-file",
        help = "Custom JSON file defining security tag patterns"
    )]
    pub security_tags_file: Option<PathBuf>,

    #[arg(
        long = "overwrite",
        action = ArgAction::SetTrue,
        help = "Overwrite an existing output directory"
    )]
    pub overwrite: bool,

    #[arg(
        long = "include-detached",
        action = ArgAction::SetTrue,
        help = "Process repositories in detached HEAD state"
    )]
    pub include_detached: bool,

    #[arg(
        long = "include-bare",
        action = ArgAction::SetTrue,
        help = "Include bare repositories during discovery"
    )]
    pub include_bare: bool,

    #[arg(
        long = "include-test-security",
        action = ArgAction::SetTrue,
        help = "Include test-path elements when computing security tags"
    )]
    pub include_test_security: bool,
}

impl Cli {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(ref diff_refs) = self.diff_refs {
            if self.root_dir.is_none() {
                return Err("--diff requires ROOT_DIR as the repo path".to_string());
            }
            if !diff_refs.contains("..") {
                return Err("--diff value must be in BASE..HEAD format (e.g. main..feature)".to_string());
            }
            if self.pull || self.force_pull {
                return Err("--diff cannot be used with --pull or --force-pull".to_string());
            }
        } else if self.root_dir.is_none() {
            return Err("ROOT_DIR is required (or use --diff)".to_string());
        }
        if self.history_depth == 0 {
            return Err("--history-depth must be >= 1".to_string());
        }
        if self.history_depth > 10 {
            return Err("--history-depth must be <= 10".to_string());
        }
        if self.force_pull && !self.pull {
            return Err("--force-pull requires --pull".to_string());
        }
        if self.pull && self.no_pull {
            return Err("--pull and --no-pull are mutually exclusive".to_string());
        }
        if self.parallel == 0 {
            return Err("--parallel must be >= 1".to_string());
        }
        Ok(())
    }

    pub fn parsed_diff_refs(&self) -> Option<(&str, &str)> {
        self.diff_refs.as_deref().and_then(|s| {
            let parts: Vec<&str> = s.splitn(2, "..").collect();
            if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
                Some((parts[0], parts[1]))
            } else {
                None
            }
        })
    }

    pub fn effective_pull_mode(&self) -> bool {
        if self.no_pull {
            return false;
        }
        self.pull
    }
}
