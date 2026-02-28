# DiffCatcher

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Security](https://img.shields.io/badge/security-focused-green.svg)](docs/security.md)

A Rust CLI tool that recursively discovers Git repositories, captures state changes, generates diffs, extracts code elements with full snippets, and produces security-focused reports for code review and audit workflows.

## 🎯 Key Features

- **Repository Discovery**: Recursively scan directories for Git repos with configurable filters
- **State Tracking**: Capture pre/post-pull state with commit hashes, messages, and dirty detection
- **Diff Generation**: Automatic N vs N-1 and historical diff creation with file manifests
- **Element Extraction**: Parse diffs to identify functions, structs, classes, imports, and more across 10+ languages
- **Code Snippets**: Extract full before/after code with boundary detection and context windows
- **Security Tagging**: 18 built-in security patterns (crypto, auth, secrets, SQL injection, XSS, etc.)
- **Multi-Format Reports**: JSON, Markdown, and text outputs with cross-repo security overview
- **Performance**: Parallel processing with progress bars, LRU caching, and incremental mode

## 📋 Table of Contents

- [Installation](#installation)
- [Quick Start](#quick-start)
- [Usage](#usage)
  - [Basic Scanning](#basic-scanning)
  - [Pull Modes](#pull-modes)
  - [Extraction Options](#extraction-options)
  - [Security Tagging](#security-tagging)
  - [Advanced Features](#advanced-features)
- [Report Structure](#report-structure)
- [Configuration](#configuration)
- [Architecture](#architecture)
- [Testing](#testing)
- [Documentation](#documentation)
- [Contributing](#contributing)

## 🚀 Installation

### From Source

```bash
git clone https://github.com/Teycir/DiffCatcher.git
cd DiffCatcher
cargo build --release
./target/release/diffcatcher --help
```

### Requirements

- Rust 1.70+
- Git 2.0+

## ⚡ Quick Start

```bash
# Scan all repos in a directory (fetch-only, no modifications)
diffcatcher ~/projects

# Pull updates and generate security report
diffcatcher ~/projects --pull -o ./report

# Dry run to see what would be scanned
diffcatcher ~/projects --dry-run

# Fast scan with 8 parallel workers
diffcatcher ~/projects -j 8 --quiet
```

## 📖 Usage

### Basic Scanning

```bash
# Scan with default settings (fetch-only)
diffcatcher <ROOT_DIR>

# Custom output directory
diffcatcher ~/projects -o ./my-report

# Include nested repos and follow symlinks
diffcatcher ~/projects --nested --follow-symlinks

# Skip hidden directories
diffcatcher ~/projects --skip-hidden
```

### Pull Modes

```bash
# Fetch only (default - no working tree changes)
diffcatcher ~/projects

# Actually pull changes
diffcatcher ~/projects --pull

# Force pull with stash/pop for dirty repos
diffcatcher ~/projects --pull --force-pull

# Use rebase strategy
diffcatcher ~/projects --pull --pull-strategy rebase

# Skip fetch/pull entirely (historical diffs only)
diffcatcher ~/projects --no-pull
```

### Extraction Options

```bash
# Skip element extraction (raw diffs only)
diffcatcher ~/projects --no-summary-extraction

# Extract elements but skip code snippets
diffcatcher ~/projects --no-snippets

# Adjust snippet context and limits
diffcatcher ~/projects --snippet-context 10 --max-snippet-lines 300

# Limit elements per diff
diffcatcher ~/projects --max-elements 1000
```

### Security Tagging

```bash
# Skip security tagging
diffcatcher ~/projects --no-security-tags

# Include test files in security analysis
diffcatcher ~/projects --include-test-security

# Use custom security patterns
diffcatcher ~/projects --security-tags-file ./custom-patterns.json
```

### Advanced Features

```bash
# Incremental mode (skip unchanged repos)
diffcatcher ~/projects --incremental -o ./report

# Filter by branch pattern
diffcatcher ~/projects --branch-filter "main"

# Adjust history depth
diffcatcher ~/projects --history-depth 5

# JSON output for CI/CD
diffcatcher ~/projects --quiet --json > result.json

# Verbose output with discovered paths
diffcatcher ~/projects --verbose
```

## 📁 Report Structure

```
<report_dir>/
├── summary.json                    # Global summary
├── summary.md                      # Markdown summary
├── security_overview.json          # Cross-repo security aggregation
├── security_overview.md
├── <repo-name>/
│   ├── status.json                 # Repo state
│   ├── pull_log.txt
│   └── diffs/
│       ├── diff_N_vs_N-1.patch     # Raw unified diff
│       ├── changes_N_vs_N-1.txt    # File manifest
│       ├── summary_N_vs_N-1.json   # Element extraction
│       ├── summary_N_vs_N-1.md
│       └── snippets/
│           ├── 001_validate_token_ADDED.rs
│           ├── 002_check_permissions_BEFORE.rs
│           ├── 002_check_permissions_AFTER.rs
│           └── 002_check_permissions.diff
└── ...
```

## ⚙️ Configuration

### CLI Flags

| Flag | Default | Description |
|------|---------|-------------|
| `-o, --output` | `./reports/<timestamp>` | Report output directory |
| `-j, --parallel` | `4` | Concurrent repo processing |
| `-t, --timeout` | `120` | Git operation timeout (seconds) |
| `-d, --history-depth` | `2` | Historical commits to diff |
| `--snippet-context` | `5` | Context lines around changes |
| `--max-snippet-lines` | `200` | Max lines per snippet |
| `--max-elements` | `500` | Max elements per diff |

See `diffcatcher --help` for all options.

### Custom Security Patterns

Create a JSON file with custom patterns:

```json
{
  "version": 1,
  "mode": "extend",
  "tags": [
    {
      "tag": "pii-handling",
      "description": "PII data processing",
      "severity": "High",
      "patterns": ["ssn", "social_security", "passport"]
    }
  ]
}
```

Use with `--security-tags-file ./patterns.json`

## 🏗️ Architecture

```
src/
├── cli.rs              # Argument parsing
├── scanner.rs          # Repository discovery
├── git/                # Git operations
│   ├── commands.rs     # Git wrappers
│   ├── state.rs        # State capture
│   ├── diff.rs         # Diff generation
│   └── file_retrieval.rs
├── extraction/         # Element extraction
│   ├── parser.rs       # Unified diff parser
│   ├── elements.rs     # Element detection
│   ├── snippets.rs     # Code snippet extraction
│   ├── boundary.rs     # Bracket/indentation tracking
│   └── languages/      # Language-specific patterns
├── security/           # Security tagging
│   ├── tagger.rs       # Pattern matching
│   ├── patterns.rs     # Built-in patterns
│   └── overview.rs     # Cross-repo aggregation
└── report/             # Report generation
    ├── writer.rs       # Directory structure
    ├── json.rs         # JSON serialization
    ├── markdown.rs     # Markdown formatting
    └── snippet_writer.rs
```

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run specific test suite
cargo test security_tagger

# Run with output
cargo test -- --nocapture
```

Test coverage includes:
- Unit tests for extraction, security tagging, boundary detection
- Integration tests for state capture, diff generation, reports
- Golden-file tests for extraction accuracy
- Edge case tests (detached HEAD, bare repos, single-commit)

## 📚 Documentation

- [Plan.md](Plan.md) - Full specification (v1.2)
- [Roadmap.md](Roadmap.md) - Implementation roadmap and progress
- Security patterns reference (see `src/security/patterns.rs`)

## 🏷️ Tags

`#rust` `#git` `#security` `#code-review` `#diff-analysis` `#static-analysis` `#devops` `#cli-tool` `#audit` `#vulnerability-detection` `#code-quality` `#snippet-extraction` `#parallel-processing` `#security-scanning`

## 🤝 Contributing

Contributions welcome! Please:
1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure `cargo test` passes
5. Submit a pull request

## 📄 License

MIT License - see LICENSE file for details

## 🔗 Links

- [GitHub Repository](https://github.com/Teycir/DiffCatcher)
- [Issue Tracker](https://github.com/Teycir/DiffCatcher/issues)
- [Changelog](CHANGELOG.md)
