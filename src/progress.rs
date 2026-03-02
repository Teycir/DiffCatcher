use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::types::{
    ChangeType, GlobalSummary, RepoResult, RepoStatus,
};

// ── Processing states ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingState {
    Discovering,
    CapturingState,
    Fetching,
    Pulling,
    GeneratingDiffs,
    ExtractingElements,
    SecurityTagging,
    WritingReports,
    Complete,
    Failed,
}

impl fmt::Display for ProcessingState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Discovering => write!(f, "Discovering..."),
            Self::CapturingState => write!(f, "Capturing state..."),
            Self::Fetching => write!(f, "Fetching..."),
            Self::Pulling => write!(f, "Pulling..."),
            Self::GeneratingDiffs => write!(f, "Generating diffs..."),
            Self::ExtractingElements => write!(f, "Extracting elements..."),
            Self::SecurityTagging => write!(f, "Security tagging..."),
            Self::WritingReports => write!(f, "Writing reports..."),
            Self::Complete => write!(f, "Complete ✓"),
            Self::Failed => write!(f, "Failed ✗"),
        }
    }
}

// ── Per-repo statistics ────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct RepoStats {
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
    pub elements_total: u32,
    pub elements_added: u32,
    pub elements_modified: u32,
    pub elements_removed: u32,
    pub security_tagged: u32,
    pub processing_time: Duration,
}

impl RepoStats {
    pub fn from_result(result: &RepoResult, elapsed: Duration) -> Self {
        let mut stats = Self {
            processing_time: elapsed,
            ..Default::default()
        };

        for diff in &result.diffs {
            stats.files_changed += diff.files_changed;
            stats.insertions += diff.insertions;
            stats.deletions += diff.deletions;

            if let Some(es) = &diff.element_summary {
                stats.elements_total += es.total_elements;
                stats.elements_added +=
                    *es.by_change_type.get(&ChangeType::Added).unwrap_or(&0);
                stats.elements_modified +=
                    *es.by_change_type.get(&ChangeType::Modified).unwrap_or(&0);
                stats.elements_removed +=
                    *es.by_change_type.get(&ChangeType::Removed).unwrap_or(&0);
            }

            if let Some(sr) = &diff.security_review {
                stats.security_tagged += sr.total_security_tagged_elements;
            }
        }

        stats
    }
}

// ── Verbosity ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verbosity {
    Quiet,
    Default,
    Verbose,
    Json,
}

// ── ProgressReporter ───────────────────────────────────────────────────────

pub struct ProgressReporter {
    verbosity: Verbosity,
    total: u32,
    completed: AtomicU32,
    _multi: MultiProgress,
    main_bar: Option<ProgressBar>,
    start_time: Instant,
    output: Mutex<OutputState>,
    parallel: usize,
}

struct OutputState {
    errors: Vec<(String, String)>, // (repo_name, error)
    repo_times: Vec<Duration>,
}

impl ProgressReporter {
    pub fn new(total: u32, verbosity: Verbosity, parallel: usize) -> Self {
        let multi = MultiProgress::new();

        let main_bar = if verbosity == Verbosity::Quiet || verbosity == Verbosity::Json {
            None
        } else {
            let pb = multi.add(ProgressBar::new(total as u64));
            let style = ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} repos ({eta}) {msg}",
            )
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("█▓░");
            pb.set_style(style);
            pb.enable_steady_tick(Duration::from_millis(120));
            Some(pb)
        };

        Self {
            verbosity,
            total,
            completed: AtomicU32::new(0),
            _multi: multi,
            main_bar,
            start_time: Instant::now(),
            output: Mutex::new(OutputState {
                errors: Vec::new(),
                repo_times: Vec::new(),
            }),
            parallel,
        }
    }

    /// Called when a repo starts processing.
    pub fn repo_started(&self, repo_name: &str) {
        let idx = self.completed.load(Ordering::Relaxed) + 1;
        if let Some(pb) = &self.main_bar {
            pb.set_message(repo_name.to_string());
        }
        if self.verbosity == Verbosity::Verbose
            && let Some(pb) = &self.main_bar
        {
            pb.println(format!(
                "  [{}/{}] Processing: {}",
                idx, self.total, repo_name
            ));
        }
    }

    /// Called on state transitions within a repo (verbose only).
    pub fn repo_state_changed(&self, repo_name: &str, state: ProcessingState) {
        if self.verbosity == Verbosity::Verbose {
            if let Some(pb) = &self.main_bar {
                pb.set_message(format!("{}: {}", repo_name, state));
                pb.println(format!("    {} → {}", repo_name, state));
            }
        } else if let Some(pb) = &self.main_bar {
            pb.set_message(format!("{}: {}", repo_name, state));
        }
    }

    /// Called when a repo finishes.
    pub fn repo_completed(&self, result: &RepoResult, elapsed: Duration) {
        let idx = self.completed.fetch_add(1, Ordering::Relaxed) + 1;
        let stats = RepoStats::from_result(result, elapsed);

        {
            let mut state = self.output.lock().unwrap();
            state.repo_times.push(elapsed);

            if let RepoStatus::FetchFailed { ref error } = result.status {
                state
                    .errors
                    .push((result.repo_name.clone(), error.clone()));
            } else if let RepoStatus::PullFailed { ref error } = result.status {
                state
                    .errors
                    .push((result.repo_name.clone(), error.clone()));
            }
            for err in &result.errors {
                if !matches!(
                    result.status,
                    RepoStatus::FetchFailed { .. } | RepoStatus::PullFailed { .. }
                ) {
                    state.errors.push((result.repo_name.clone(), err.clone()));
                }
            }
        }

        if let Some(pb) = &self.main_bar {
            pb.inc(1);

            let status_marker = match &result.status {
                RepoStatus::Updated => "\x1b[32m✓ Updated\x1b[0m",
                RepoStatus::UpToDate => "\x1b[34m· UpToDate\x1b[0m",
                RepoStatus::DirtySkipped => "\x1b[33m⚠ DirtySkipped\x1b[0m",
                RepoStatus::FetchFailed { .. } => "\x1b[31m✗ FetchFailed\x1b[0m",
                RepoStatus::PullFailed { .. } => "\x1b[31m✗ PullFailed\x1b[0m",
                RepoStatus::Skipped { .. } => "\x1b[33m⊘ Skipped\x1b[0m",
            };

            let detail = if self.verbosity == Verbosity::Verbose {
                format!(
                    "  [{}/{}] {} {} | +{} -{} | {} elements ({} added, {} modified, {} removed) | {} security | {:.1}s",
                    idx,
                    self.total,
                    result.repo_name,
                    status_marker,
                    stats.insertions,
                    stats.deletions,
                    stats.elements_total,
                    stats.elements_added,
                    stats.elements_modified,
                    stats.elements_removed,
                    stats.security_tagged,
                    elapsed.as_secs_f64(),
                )
            } else {
                format!(
                    "  [{}/{}] {} {}",
                    idx, self.total, result.repo_name, status_marker,
                )
            };

            pb.println(detail);

            // Show inline errors
            if matches!(
                result.status,
                RepoStatus::FetchFailed { .. } | RepoStatus::PullFailed { .. }
            ) {
                let err_msg = match &result.status {
                    RepoStatus::FetchFailed { error } => error,
                    RepoStatus::PullFailed { error } => error,
                    _ => unreachable!(),
                };
                let truncated = truncate_error(err_msg, 120);
                pb.println(format!("         \x1b[31m└─ {}\x1b[0m", truncated));
            }
        }
    }

    /// Finish the progress bar.
    pub fn finish(&self) {
        if let Some(pb) = &self.main_bar {
            pb.finish_and_clear();
        }
    }

    /// Print comprehensive summary.
    pub fn print_summary(
        &self,
        summary: &GlobalSummary,
        results: &[RepoResult],
        report_dir: &std::path::Path,
        exit_code: i32,
    ) {
        if self.verbosity == Verbosity::Json {
            return; // JSON mode handles its own output
        }

        let total_elapsed = self.start_time.elapsed();
        let state = self.output.lock().unwrap();

        let total_files_changed: u32 = results
            .iter()
            .flat_map(|r| r.diffs.iter())
            .map(|d| d.files_changed)
            .sum();

        let total_elements: u32 = results
            .iter()
            .flat_map(|r| r.diffs.iter())
            .filter_map(|d| d.element_summary.as_ref())
            .map(|es| es.total_elements)
            .sum();

        let total_security: u32 = results
            .iter()
            .flat_map(|r| r.diffs.iter())
            .filter_map(|d| d.security_review.as_ref())
            .map(|sr| sr.total_security_tagged_elements)
            .sum();

        let high_attention: usize = results
            .iter()
            .flat_map(|r| r.diffs.iter())
            .filter_map(|d| d.security_review.as_ref())
            .map(|sr| sr.high_attention_items.len())
            .sum();

        let avg_time = if !state.repo_times.is_empty() {
            total_elapsed.as_secs_f64() / state.repo_times.len() as f64
        } else {
            0.0
        };

        let mut out = String::new();
        out.push_str("\n\x1b[1m═══════════════════════════════════════════════════════\x1b[0m\n");
        out.push_str("\x1b[1m  DiffCatcher Summary\x1b[0m\n");
        out.push_str("\x1b[1m═══════════════════════════════════════════════════════\x1b[0m\n\n");

        // Status breakdown
        out.push_str(&format!(
            "  Repositories:  {} total\n",
            summary.total_repos_found
        ));
        if summary.updated > 0 {
            out.push_str(&format!(
                "    \x1b[32m▸ Updated:       {}\x1b[0m\n",
                summary.updated
            ));
        }
        if summary.up_to_date > 0 {
            out.push_str(&format!(
                "    \x1b[34m▸ Up-to-date:    {}\x1b[0m\n",
                summary.up_to_date
            ));
        }
        if summary.fetch_failed > 0 {
            out.push_str(&format!(
                "    \x1b[31m▸ Fetch failed:  {}\x1b[0m\n",
                summary.fetch_failed
            ));
        }
        if summary.pull_failed > 0 {
            out.push_str(&format!(
                "    \x1b[31m▸ Pull failed:   {}\x1b[0m\n",
                summary.pull_failed
            ));
        }
        if summary.dirty_skipped > 0 {
            out.push_str(&format!(
                "    \x1b[33m▸ Dirty skipped: {}\x1b[0m\n",
                summary.dirty_skipped
            ));
        }
        if summary.skipped > 0 {
            out.push_str(&format!(
                "    \x1b[33m▸ Skipped:       {}\x1b[0m\n",
                summary.skipped
            ));
        }

        out.push('\n');

        // Aggregate statistics
        out.push_str("  Statistics:\n");
        out.push_str(&format!("    Files changed:           {}\n", total_files_changed));
        out.push_str(&format!("    Elements extracted:       {}\n", total_elements));
        out.push_str(&format!("    Security-tagged elements: {}\n", total_security));
        if high_attention > 0 {
            out.push_str(&format!(
                "    \x1b[31mHigh-attention items:      {}\x1b[0m\n",
                high_attention
            ));
        }

        out.push('\n');

        // Performance metrics
        out.push_str("  Performance:\n");
        out.push_str(&format!(
            "    Total time:     {:.1}s\n",
            total_elapsed.as_secs_f64()
        ));
        out.push_str(&format!("    Avg per repo:   {:.2}s\n", avg_time));
        out.push_str(&format!("    Workers:        {}\n", self.parallel));

        out.push('\n');

        // Report location
        out.push_str(&format!(
            "  Report: \x1b[4m{}\x1b[0m\n",
            report_dir.display()
        ));

        // Errors summary
        if !state.errors.is_empty() && self.verbosity != Verbosity::Quiet {
            out.push('\n');
            out.push_str(&format!(
                "  \x1b[31mErrors ({}):\x1b[0m\n",
                state.errors.len()
            ));
            let max_errors = if self.verbosity == Verbosity::Verbose {
                state.errors.len()
            } else {
                5
            };
            for (i, (repo, err)) in state.errors.iter().enumerate() {
                if i >= max_errors {
                    out.push_str(&format!(
                        "    ... and {} more errors\n",
                        state.errors.len() - max_errors
                    ));
                    break;
                }
                let truncated = truncate_error(err, 100);
                out.push_str(&format!("    \x1b[31m✗\x1b[0m {}: {}\n", repo, truncated));
            }

            // Suggest fixes for common issues
            let has_permission = state
                .errors
                .iter()
                .any(|(_, e)| e.contains("Permission denied") || e.contains("permission denied"));
            let has_timeout = state
                .errors
                .iter()
                .any(|(_, e)| e.contains("timeout") || e.contains("Timeout"));
            let has_auth = state
                .errors
                .iter()
                .any(|(_, e)| e.contains("Authentication") || e.contains("authentication") || e.contains("could not read Username"));

            if has_permission || has_timeout || has_auth {
                out.push('\n');
                out.push_str("  \x1b[33mSuggestions:\x1b[0m\n");
                if has_permission {
                    out.push_str(
                        "    • Permission denied: check filesystem permissions or run with appropriate access\n",
                    );
                }
                if has_timeout {
                    out.push_str(
                        "    • Timeout: increase --timeout value or check network connectivity\n",
                    );
                }
                if has_auth {
                    out.push_str(
                        "    • Authentication failure: configure SSH keys or credential helpers\n",
                    );
                }
            }
        }

        // Exit code explanation
        if exit_code != 0 {
            out.push('\n');
            out.push_str(&format!(
                "  Exit code: {} ({})\n",
                exit_code,
                match exit_code {
                    1 => "fatal error",
                    2 => "partial failures — some repos could not be processed",
                    _ => "unknown",
                }
            ));
        }

        out.push_str("\n\x1b[1m═══════════════════════════════════════════════════════\x1b[0m\n");

        // Use stderr for progress/summary to avoid polluting stdout
        eprint!("{}", out);
    }
}

fn truncate_error(err: &str, max_len: usize) -> String {
    let first_line = err.lines().next().unwrap_or(err);
    if first_line.len() > max_len {
        format!("{}...", &first_line[..max_len])
    } else {
        first_line.to_string()
    }
}

// ── State callback type ────────────────────────────────────────────────────

/// A thread-safe callback for reporting state changes during repo processing.
pub type StateCallback<'a> = dyn Fn(&str, ProcessingState) + Send + Sync + 'a;
