# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Improved changelog structure to include complete release notes and verified release metadata.

## [0.1.0] - 2026-04-02

### Added

- Recursive Git repository discovery and diff capture workflow for audit/review pipelines.
- Unified diff parser with hunk extraction, language-aware element detection, and full snippet extraction.
- Security-focused report generation with risk scoring, confidence scoring, and escalation support.
- CLI support for `--watch`, `--include-vendor`, branch-diff mode, and SARIF output mode.
- Runtime configuration and plugin extension system.
- Benchmark harness in `benches/core_bench.rs`.

### Changed

- Progress reporting UX for long-running scans.
- Default report output location to timestamped paths (`reports/<timestamp>`).
- Cross-platform Git command building behavior for better portability.

### Fixed

- Binary file path extraction in unified diff headers.
- `/dev/null` handling in parser edge cases.
- Snippet extraction edge case when diff lines are absent.

### CI/CD

- Added GitHub Actions workflows for CI validation and release packaging.

### Documentation

- Expanded README and inline documentation coverage across core modules.

### Release Metadata

- GitHub Release published: `v0.1.0` at `2026-04-02T16:21:42Z`.
- Git tag published: `v0.1.0`.
- Latest repository push timestamp: `2026-04-02T16:20:53Z` (GitHub API `pushed_at` for `Teycir/DiffCatcher`).
