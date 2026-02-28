# Git Patrol — Implementation Roadmap (v1.2)

> This roadmap references the full specification in [Plan.md](./Plan.md).
> Each phase builds on the previous one. A phase is complete when all its items are checked off.

---

## Execution Protocol

- [x] Follow this roadmap strictly in order from top to bottom.
- [ ] Do not mark a phase complete until every checkbox under it is done.
- [x] Keep implementation evidence (commands run, outputs, generated report artifacts) for each completed step.
- [ ] Final validation target: run the tool against `/media/elements/Repos`.
- [ ] Mark final completion only after acceptance criteria in Plan.md §12 pass on `/media/elements/Repos`.

### Progress Log

- [x] 2026-02-28: Created granular commits for core, extraction, security, reporting, and tests.
- [x] 2026-02-28: Implemented runnable `git-patrol` CLI with discovery, fetch/pull, diff generation, extraction, tagging, and reporting.
- [x] 2026-02-28: Added initial integration tests in `tests/basic.rs`.
- [x] 2026-02-28: Validated execution on `/media/elements/Repos/apt0d/libnvidia-container` (sample validation run).

---

## Phase 0 — Project Bootstrap

- [x] Initialize Rust project: `cargo init --name git-patrol`
- [ ] Set up `Cargo.toml` with dependencies: `git2`, `clap` (v4 derive), `walkdir`, `rayon`, `serde`, `serde_json`, `chrono`, `thiserror`, `indicatif`, `tracing`, `tracing-subscriber`, `regex`, `once_cell`, `glob`
- [x] Create module structure per Plan.md §5.1
- [x] Set up `error.rs` with `thiserror` error types (Plan.md §8)
- [x] Set up `types.rs` with all data types from Plan.md §6
- [x] Set up `cli.rs` with all flags from Plan.md §4 using clap derive
- [x] Verify: `cargo build` succeeds, `--help` prints all flags

---

## Phase 1 — Repository Discovery (Plan.md §3.1)

- [x] Implement `scanner.rs`: recursive walkdir to find `.git` directories
- [x] Handle `--nested`, `--follow-symlinks`, `--skip-hidden` flags
- [x] Skip `.git` internals during traversal
- [x] Handle bare repo detection (configurable)
- [ ] Unit tests: temp dirs with nested repos, hidden dirs, symlinks
- [ ] Verify: discovers all repos under a test directory, prints paths

---

## Phase 2 — State Capture & Fetch (Plan.md §3.2, §3.3, §3.4, §3.5)

- [ ] Implement `git/commands.rs`: wrappers around `git2` operations
- [x] Implement `git/state.rs`: pre-fetch state capture (hash, message, branch, dirty check)
- [x] Implement `git fetch origin` as default update mechanism
- [x] Implement `--pull` mode: `git pull` with strategy selection (`--pull-strategy`)
- [x] Implement `--force-pull` (stash/pop, requires `--pull`)
- [x] Implement `--no-pull` (skip fetch entirely, historical diffs only)
- [x] Implement post-fetch state capture
- [x] Implement up-to-date detection (pre vs post hash comparison)
- [x] Implement `--timeout` for git operations
- [x] Implement `--dry-run` (discover + state capture only)
- [ ] Handle errors: detached HEAD, bare repo, permission denied, network timeout
- [ ] Integration tests: temp repos with known commits, verify state capture accuracy
- [ ] Verify: fetches repos, captures correct pre/post state, detects UPDATED vs UP_TO_DATE

---

## Phase 3 — Diff Generation (Plan.md §3.6)

- [x] Implement `git/diff.rs`: generate diffs between commit pairs
- [x] Generate N vs N-1 and N-1 vs N-2 diffs (when UPDATED)
- [x] Generate historical diffs for UP_TO_DATE repos (controlled by `--history-depth`)
- [x] Generate file change manifests (`--stat`, `--name-status`)
- [ ] Handle edge cases: repos with <3 commits, merge commits
- [x] Implement `git/file_retrieval.rs`: `git show <commit>:<path>` for full file content
- [x] Write raw `.patch` files and `changes_*.txt` files
- [ ] Unit tests: known commit pairs → expected diff output
- [x] Verify: correct `.patch` and `changes_*.txt` files generated per repo

---

## Phase 4 — Element Extraction & Snippets (Plan.md §3.7)

- [x] Implement `extraction/parser.rs`: unified diff parser (files, hunks, headers)
- [x] Implement `extraction/classifier.rs`: file extension → Language mapping (Plan.md §3.7.5)
- [x] Implement `extraction/elements.rs`: element detection via regex patterns
  - [x] Function/Method detection
  - [x] Struct/Class/Type detection
  - [x] Enum, Trait/Interface, Impl block detection
  - [x] Constant/Static, Import/Use, Module detection
  - [x] Config block, Test, Macro detection
  - [x] Change type classification (Added/Modified/Removed)
- [x] Implement language-specific patterns in `extraction/languages/`:
  - [x] `rust.rs`
  - [x] `python.rs`
  - [x] `javascript.rs` (+ TypeScript)
  - [x] `go.rs`
  - [x] `c_cpp.rs`
  - [x] `java_kotlin.rs`
  - [x] `ruby.rs`
  - [x] `config.rs` (TOML, YAML, JSON)
  - [x] `fallback.rs`
- [x] Implement `extraction/snippets.rs`: code snippet extraction (before/after/diff)
- [ ] Implement `extraction/boundary.rs`: bracket/indentation tracking for full element capture
- [x] Implement `--snippet-context`, `--max-snippet-lines`, `--max-elements` caps
- [ ] Implement cross-diff caching for `git show` file retrieval (LRU cache by commit+path)
- [ ] Unit tests: golden-file tests per language — known `.patch` → expected elements + snippets
- [ ] Unit tests: boundary detection across code styles (K&R, Allman, Python indentation)
- [x] Verify: `summary_*.json` correctly lists elements with accurate snippets for test repos

---

## Phase 5 — Security Tagging (Plan.md §3.8)

- [x] Implement `security/patterns.rs`: built-in security tag definitions (Plan.md §3.8.1)
- [x] Implement `security/tagger.rs`: pattern matching engine against element fields
- [x] Implement noise reduction (Plan.md §3.8.1.1):
  - [x] Minimum match threshold for broad tags
  - [x] Negative patterns support
  - [x] Test file path exclusions with `in_test` flag
- [x] Implement `security-removal` meta-tag for removed security controls
- [x] Implement `security/custom.rs`: custom patterns file loader (`--security-tags-file`, `extend`/`replace` modes)
- [x] Implement per-diff security summary (Plan.md §3.8.3)
- [x] Implement `high_attention_items` logic (Plan.md §3.8.3)
- [x] Implement `--no-security-tags` flag
- [x] Implement `--include-test-security` flag
- [ ] Unit tests: known code snippets → expected tags, false positive/negative cases
- [ ] Golden-file tests: known changes → expected security summary
- [x] Verify: security tags correctly applied, high attention items flagged

---

## Phase 6 — Report Generation (Plan.md §3.9, §3.10, §3.11)

- [x] Implement `report/writer.rs`: create report directory structure (Plan.md §3.9)
- [x] Implement `report/json.rs`: JSON serialization for all report types
  - [x] `status.json` per repo
  - [x] `summary_*.json` per diff (with element summary + security review)
  - [x] Top-level `summary.json`
  - [x] `security_overview.json`
- [x] Implement `report/markdown.rs`: markdown formatting
  - [x] `summary_*.md` per diff (with snippet previews referencing snippet files)
  - [x] Top-level `summary.md`
  - [x] `security_overview.md`
- [x] Implement `report/snippet_writer.rs`: individual snippet files in `snippets/`
  - [x] Naming convention: `<NNN>_<element_name>_<change_type>.<ext>`
  - [x] BEFORE/AFTER/ADDED/REMOVED variants
  - [x] `.diff` files per element
- [x] Implement repo naming collision handling (`--` separator)
- [x] Implement `security/overview.rs`: cross-repo security overview aggregation
- [x] Implement `--summary-format` flag (default: `json,md`)
- [x] Implement `--no-snippets`, `--no-summary-extraction` flags
- [x] Implement `--overwrite` and auto-suffix for existing output dirs
- [ ] Unit tests: known inputs → expected file structure and content
- [ ] Verify: complete report directory matches Plan.md §3.9 structure

---

## Phase 7 — Orchestration & Concurrency (Plan.md §5.2, §5.4)

- [x] Implement `processor.rs`: per-repo pipeline orchestration (state → fetch → diff → extract → tag → report)
- [x] Implement `main.rs`: top-level orchestration (discover → parallel process → aggregate → write global reports)
- [x] Implement rayon-based parallel repo processing (`--parallel`)
- [ ] Implement intra-repo parallelism for per-file extraction/tagging
- [ ] Implement progress bars with `indicatif` (suppressed by `--quiet`)
- [x] Implement `--verbose` logging with `tracing`
- [x] Implement `--json` stdout output for CI/CD piping
- [x] Implement `--branch-filter` glob matching
- [x] Implement incremental mode (`--incremental` + `.git-patrol-state.json`)
- [x] Implement exit codes: 0 (success), 1 (fatal), 2 (partial)
- [ ] Verify: end-to-end run on multiple repos with all flags

---

## Phase 8 — Error Handling & Edge Cases (Plan.md §8)

- [ ] Verify all error scenarios from Plan.md §8 are handled:
  - [ ] `git` not found / `git2` initialization failure
  - [ ] Root dir doesn't exist
  - [ ] Single repo failure → continue processing
  - [ ] Permission denied on subdirectory
  - [ ] Repo with <3 commits
  - [ ] Detached HEAD
  - [ ] Bare repository
  - [ ] Diff >50MB → truncate extraction
  - [ ] Element extraction regex panic → fallback
  - [ ] Binary files in diff
  - [ ] `git show` failure → DiffOnly fallback
  - [ ] Snippet exceeds max lines → Truncated scope
  - [ ] Invalid custom security tags file
- [ ] Edge case tests: 0 commits, 1 commit, merge commits, submodules, binary files, empty diffs, non-UTF8 files, renamed files

---

## Phase 9 — Integration & E2E Testing (Plan.md §10)

- [ ] Integration tests: temp directories with `git init`, known commits, run tool, validate full report
- [ ] E2E test script: clone small real repos, run tool, validate output
- [ ] Performance test: 50 repos with `--parallel 8`, must complete in <5 minutes
- [ ] Golden-file tests: one per supported language for extraction accuracy
- [ ] Security tag accuracy tests: known changes → expected tags with false positive/negative cases
- [ ] Test all CLI flag combinations that interact (e.g., `--no-pull` + `--dry-run`, `--pull` + `--force-pull`)

---

## Phase 10 — Final Validation

- [ ] Run against `/media/elements/Repos` — full scan with default settings
- [ ] Verify all 24 acceptance criteria from Plan.md §12 pass
- [ ] Review generated reports for correctness and readability
- [ ] Verify `--help` output matches Plan.md §4 exactly
- [ ] `cargo clippy` — zero warnings
- [x] `cargo test` — all tests pass
- [ ] Tag as v1.0.0
