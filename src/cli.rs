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
}

#[derive(Debug, Parser)]
#[command(
    name = "git-patrol",
    version,
    about = "Scan git repositories and produce security-focused diff reports"
)]
pub struct Cli {
    #[arg(value_name = "ROOT_DIR")]
    pub root_dir: PathBuf,

    #[arg(short = 'o', long = "output", value_name = "DIR")]
    pub output: Option<PathBuf>,

    #[arg(short = 's', long = "pull-strategy", value_enum, default_value_t = PullStrategy::FfOnly)]
    pub pull_strategy: PullStrategy,

    #[arg(short = 't', long = "timeout", default_value_t = 120)]
    pub timeout: u64,

    #[arg(long, action = ArgAction::SetTrue)]
    pub nested: bool,

    #[arg(long = "follow-symlinks", action = ArgAction::SetTrue)]
    pub follow_symlinks: bool,

    #[arg(long = "skip-hidden", action = ArgAction::SetTrue)]
    pub skip_hidden: bool,

    #[arg(long = "pull", action = ArgAction::SetTrue)]
    pub pull: bool,

    #[arg(long = "force-pull", action = ArgAction::SetTrue)]
    pub force_pull: bool,

    #[arg(long = "no-pull", action = ArgAction::SetTrue)]
    pub no_pull: bool,

    #[arg(short = 'd', long = "history-depth", default_value_t = 2)]
    pub history_depth: u32,

    #[arg(short = 'j', long = "parallel", default_value_t = 4)]
    pub parallel: usize,

    #[arg(short = 'q', long = "quiet", action = ArgAction::SetTrue)]
    pub quiet: bool,

    #[arg(short = 'v', long = "verbose", action = ArgAction::SetTrue)]
    pub verbose: bool,

    #[arg(long = "dry-run", action = ArgAction::SetTrue)]
    pub dry_run: bool,

    #[arg(long = "json", action = ArgAction::SetTrue)]
    pub json_stdout: bool,

    #[arg(long = "branch-filter", default_value = "*")]
    pub branch_filter: String,

    #[arg(long = "no-summary-extraction", action = ArgAction::SetTrue)]
    pub no_summary_extraction: bool,

    #[arg(long = "no-snippets", action = ArgAction::SetTrue)]
    pub no_snippets: bool,

    #[arg(long = "no-security-tags", action = ArgAction::SetTrue)]
    pub no_security_tags: bool,

    #[arg(long = "snippet-context", default_value_t = 5)]
    pub snippet_context: u32,

    #[arg(long = "max-snippet-lines", default_value_t = 200)]
    pub max_snippet_lines: u32,

    #[arg(long = "max-elements", default_value_t = 500)]
    pub max_elements: usize,

    #[arg(long = "summary-format", value_delimiter = ',', default_values = ["json", "md"])]
    pub summary_formats: Vec<SummaryFormat>,

    #[arg(long = "incremental", action = ArgAction::SetTrue)]
    pub incremental: bool,

    #[arg(long = "security-tags-file")]
    pub security_tags_file: Option<PathBuf>,

    #[arg(long = "overwrite", action = ArgAction::SetTrue)]
    pub overwrite: bool,

    #[arg(long = "include-detached", action = ArgAction::SetTrue)]
    pub include_detached: bool,

    #[arg(long = "include-bare", action = ArgAction::SetTrue)]
    pub include_bare: bool,

    #[arg(long = "include-test-security", action = ArgAction::SetTrue)]
    pub include_test_security: bool,
}

impl Cli {
    pub fn validate(&self) -> Result<(), String> {
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

    pub fn effective_pull_mode(&self) -> bool {
        if self.no_pull {
            return false;
        }
        self.pull
    }
}
