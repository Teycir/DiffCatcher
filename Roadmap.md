# Git Patrol — Implementation Roadmap (v1.2)

> This roadmap references the full specification in [Plan.md](./Plan.md).
> Each phase builds on the previous one. A phase is complete when all its items are checked off.

---

## Execution Protocol

- [ ] Follow this roadmap strictly in order from top to bottom.
- [ ] Do not mark a phase complete until every checkbox under it is done.
- [ ] Keep implementation evidence (commands run, outputs, generated report artifacts) for each completed step.
- [ ] Final validation target: run the tool against `/media/elements/Repos`.
- [ ] Mark final completion only after acceptance criteria in Plan.md §12 pass on `/media/elements/Repos`.

---

## Phase 0 — Project Bootstrap

- [ ] Initialize Rust project: `cargo init --name git-patrol`
- [ ] Set up `Cargo.toml` with dependencies: `git2`, `clap` (v4 derive), `walkdir`, `rayon`, `serde`, `serde_json`, `chrono`, `thiserror`, `indicatif`, `tracing`, `tracing-subscriber`, `regex`, `once_cell`, `glob`
- [ ] Create module structure per Plan.md §5.1
- [ ] Set up `error.rs` with `thiserror` error types (Plan.md §8)
- [ ] Set up `types.rs` with all data types from Plan.md §6
- [ ] Set up `cli.rs` with all flags from Plan.md §4 using clap derive
- [ ] Verify: `cargo build` succeeds, `--help` prints all flags

---

## Phase 1 — Repository Discovery (Plan.md §3.1)

- [ ] Implement `scanner.rs`: recursive walkdir to find `.git` directories
- [ ] Handle `--nested`, `--follow-symlinks`, `--skip-hidden` flags
- [ ] Skip `.git` internals during traversal
- [ ] Handle bare repo detection (configurable)
- [ ] Unit tests: temp dirs with nested repos, hidden dirs, symlinks
- [ ] Verify: discovers all repos under a test directory, prints paths

---

## Phase 2 — State Capture & Fetch (Plan.md §3.2, §3.3, §3.4, §3.5)

- [ ] Implement `git/commands.rs`: wrappers around `git2` operations
- [ ] Implement `git/state.rs`: pre-fetch state capture (hash, message, branch, dirty check)
- [ ] Implement `git fetch origin` as default update mechanism
- [ ] Implement `--pull` mode: `git pull` with strategy selection (`--pull-strategy`)
- [ ] Implement `--force-pull` (stash/pop, requires `--pull`)
- [ ] Implement `--no-pull` (skip fetch entirely, historical diffs only)
- [ ] Implement post-fetch state capture
- [ ] Implement up-to-date detection (pre vs post hash comparison)
- [ ] Implement `--timeout` for git operations
- [ ] Implement `--dry-run` (discover + state capture only)
- [ ] Handle errors: detached HEAD, bare repo, permission denied, network timeout
- [ ] Integration tests: temp repos with known commits, verify state capture accuracy
- [ ] Verify: fetches repos, captures correct pre/post state, detects UPDATED vs UP_TO_DATE

---

## Phase 3 — Diff Generation (Plan.md §3.6)

- [ ] Implement `git/diff.rs`: generate diffs between commit pairs
- [ ] Generate N vs N-1 and N-1 vs N-2 diffs (when UPDATED)
- [ ] Generate historical diffs for UP_TO_DATE repos (controlled by `--history-depth`)
- [ ] Generate file change manifests (`--stat`, `--name-status`)
- [ ] Handle edge cases: repos with <3 commits, merge commits
- [ ] Implement `git/file_retrieval.rs`: `git show <commit>:<path>` for full file content
- [ ] Write raw `.patch` files and `changes_*.txt` files
- [ ] Unit tests: known commit pairs → expected diff output
- [ ] Verify: correct `.patch` and `changes_*.txt` files generated per repo

---

## Phase 4 — Element Extraction & Snippets (Plan.md §3.7)

- [ ] Implement `extraction/parser.rs`: unified diff parser (files, hunks, headers)
- [ ] Implement `extraction/classifier.rs`: file extension → Language mapping (Plan.md §3.7.5)
- [ ] Implement `extraction/elements.rs`: element detection via regex patterns
  - [ ] Function/Method detection
  - [ ] Struct/Class/Type detection
  - [ ] Enum, Trait/Interface, Impl block detection
  - [ ] Constant/Static, Import/Use, Module detection
  - [ ] Config block, Test, Macro detection
  - [ ] Change type classification (Added/Modified/Removed)
- [ ] Implement language-specific patterns in `extraction/languages/`:
  - [ ] `rust.rs`
  - [ ] `python.rs`
  - [ ] `javascript.rs` (+ TypeScript)
  - [ ] `go.rs`
  - [ ] `c_cpp.rs`
  - [ ] `java_kotlin.rs`
  - [ ] `ruby.rs`
  - [ ] `config.rs` (TOML, YAML, JSON)
  - [ ] `fallback.rs`
- [ ] Implement `extraction/snippets.rs`: code snippet extraction (before/after/diff)
- [ ] Implement `extraction/boundary.rs`: bracket/indentation tracking for full element capture
- [ ] Implement `--snippet-context`, `--max-snippet-lines`, `--max-elements` caps
- [ ] Implement cross-diff caching for `git show` file retrieval (LRU cache by commit+path)
- [ ] Unit tests: golden-file tests per language — known `.patch` → expected elements + snippets
- [ ] Unit tests: boundary detection across code styles (K&R, Allman, Python indentation)
- [ ] Verify: `summary_*.json` correctly lists elements with accurate snippets for test repos

---

## Phase 5 — Security Tagging (Plan.md §3.8)

- [ ] Implement `security/patterns.rs`: built-in security tag definitions (Plan.md §3.8.1)
- [ ] Implement `security/tagger.rs`: pattern matching engine against element fields
- [ ] Implement noise reduction (Plan.md §3.8.1.1):
  - [ ] Minimum match threshold for broad tags
  - [ ] Negative patterns support
  - [ ] Test file path exclusions with `in_test` flag
- [ ] Implement `security-removal` meta-tag for removed security controls
- [ ] Implement `security/custom.rs`: custom patterns file loader (`--security-tags-file`, `extend`/`replace` modes)
- [ ] Implement per-diff security summary (Plan.md §3.8.3)
- [ ] Implement `high_attention_items` logic (Plan.md §3.8.3)
- [ ] Implement `--no-security-tags` flag
- [ ] Implement `--include-test-security` flag
- [ ] Unit tests: known code snippets → expected tags, false positive/negative cases
- [ ] Golden-file tests: known changes → expected security summary
- [ ] Verify: security tags correctly applied, high attention items flagged

---

## Phase 6 — Report Generation (Plan.md §3.9, §3.10, §3.11)

- [ ] Implement `report/writer.rs`: create report directory structure (Plan.md §3.9)
- [ ] Implement `report/json.rs`: JSON serialization for all report types
  - [ ] `status.json` per repo
  - [ ] `summary_*.json` per diff (with element summary + security review)
  - [ ] Top-level `summary.json`
  - [ ] `security_overview.json`
- [ ] Implement `report/markdown.rs`: markdown formatting
  - [ ] `summary_*.md` per diff (with snippet previews referencing snippet files)
  - [ ] Top-level `summary.md`
  - [ ] `security_overview.md`
- [ ] Implement `report/snippet_writer.rs`: individual snippet files in `snippets/`
  - [ ] Naming convention: `<NNN>_<element_name>_<change_type>.<ext>`
  - [ ] BEFORE/AFTER/ADDED/REMOVED variants
  - [ ] `.diff` files per element
- [ ] Implement repo naming collision handling (`--` separator)
- [ ] Implement `security/overview.rs`: cross-repo security overview aggregation
- [ ] Implement `--summary-format` flag (default: `json,md`)
- [ ] Implement `--no-snippets`, `--no-summary-extraction` flags
- [ ] Implement `--overwrite` and auto-suffix for existing output dirs
- [ ] Unit tests: known inputs → expected file structure and content
- [ ] Verify: complete report directory matches Plan.md §3.9 structure

---

## Phase 7 — Orchestration & Concurrency (Plan.md §5.2, §5.4)

- [ ] Implement `processor.rs`: per-repo pipeline orchestration (state → fetch → diff → extract → tag → report)
- [ ] Implement `main.rs`: top-level orchestration (discover → parallel process → aggregate → write global reports)
- [ ] Implement rayon-based parallel repo processing (`--parallel`)
- [ ] Implement intra-repo parallelism for per-file extraction/tagging
- [ ] Implement progress bars with `indicatif` (suppressed by `--quiet`)
- [ ] Implement `--verbose` logging with `tracing`
- [ ] Implement `--json` stdout output for CI/CD piping
- [ ] Implement `--branch-filter` glob matching
- [ ] Implement incremental mode (`--incremental` + `.git-patrol-state.json`)
- [ ] Implement exit codes: 0 (success), 1 (fatal), 2 (partial)
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
- [ ] `cargo test` — all tests pass
- [ ] Tag as v1.0.0
