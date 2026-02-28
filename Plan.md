# Git Repo Scanner & Diff Reporter — Full Specification (v1.2)

## 1. Overview

**Name:** `diffcatcher`

A Rust CLI tool that recursively discovers all Git repositories under a given root directory, records their state, pulls updates, computes historical diffs, **extracts and summarizes all changed elements (functions, structs, types, files, modules) with their full code snippets**, and produces a structured output report directory. The primary downstream use case is **security review**: the extracted snippets and element context allow auditors or automated tools to assess whether changes introduced vulnerabilities, weakened access controls, altered cryptographic logic, modified authentication flows, or otherwise degraded the security posture of the codebase.

---

## 2. Glossary

| Term | Definition |
|------|-----------|
| **N** | The `HEAD` commit *after* `git pull` completes |
| **N-1** | The commit immediately before N (`HEAD~1`) |
| **N-2** | The commit two before N (`HEAD~2`) |
| **Root directory** | The user-supplied directory to scan |
| **Report directory** | The output directory containing all results |
| **Repo report folder** | A subfolder inside the report directory, one per discovered repo |
| **Diff summary** | A structured extraction of *what* changed: files, hunks, functions, types, net line impact |
| **Change element** | A discrete unit of change: a function, struct, enum, class, method, constant, import, or config block that was added, modified, or removed |
| **Code snippet** | The actual source code lines (before and after) for a changed element, extracted verbatim from the diff, presented in context for security review |
| **Security tag** | An automatic heuristic label applied to a change snippet when it touches security-sensitive patterns (crypto, auth, permissions, input validation, secrets, network, etc.) |

---

## 3. Functional Requirements

### 3.1 — Repository Discovery

```
INPUT:  root directory path (CLI argument)
OUTPUT: list of absolute paths to directories containing a `.git` folder
```

- Walk the root directory **recursively**.
- A directory qualifies as a git repo if it contains a `.git` subdirectory (or is a bare repo with a `HEAD` file — configurable flag).
- **Do not** recurse into `.git` directories themselves.
- **Do not** recurse into discovered repos looking for nested repos **unless** `--nested` flag is set (default: skip nested).
- Follow symlinks: **no** by default, enabled with `--follow-symlinks`.
- Respect `.gitignore` / hidden dirs: scan everything by default, `--skip-hidden` to ignore dot-prefixed directories (except `.git` detection itself).

### 3.2 — Pre-Fetch State Capture

For each discovered repo, **before** fetching/pulling, record:

| Field | Source |
|-------|--------|
| `pre_pull_hash` | `git rev-parse HEAD` |
| `pre_pull_message` | `git log -1 --pretty=%B` |
| `pre_pull_branch` | `git rev-parse --abbrev-ref HEAD` |
| `pre_pull_dirty` | `git status --porcelain` (non-empty = dirty) |

If the working tree is **dirty** (uncommitted changes):
- **Fetch-only mode (default)**: Dirty state is **recorded** but does **not** block the operation, since `git fetch` does not modify the working tree.
- **With `--pull`**: Default behavior **skips** the pull for that repo, logs a warning, and marks it `DIRTY_SKIPPED`.
- **With `--pull --force-pull`**: Stash before pull, pop after (`git stash push -m "diffcatcher auto-stash"` / `git stash pop`). The `--force-pull` flag only applies when `--pull` is also set.

### 3.3 — Update (git fetch / git pull)

**Default: fetch-only**

```
git fetch origin
```

- By default, only remote-tracking refs are updated. The working tree and local branch are **not** modified.
- Diffs are computed against `origin/<branch>` vs local `HEAD`, so the tool shows what *would* change on pull without touching the working tree.
- Timeout: default **120 seconds** per repo, configurable via `--timeout <seconds>`.
- On failure: record error in report, mark repo as `FETCH_FAILED`, continue to next repo.

**With `--pull`: pull mode**

```
git pull --ff-only
```

- Use `--ff-only` by default to avoid creating merge commits.
- Flag `--pull-strategy <ff-only|rebase|merge>` overrides this.
- Capture pull stdout/stderr for the report.
- On failure: record error in report, mark repo as `PULL_FAILED`, continue to next repo.

### 3.4 — Post-Pull State Capture

After pull, record:

| Field | Source |
|-------|--------|
| `post_pull_hash` | `git rev-parse HEAD` |
| `post_pull_message` | `git log -1 --pretty=%B` |

### 3.5 — Up-to-Date Detection

Compare `pre_pull_hash` with `post_pull_hash`:

- **If equal →** repo is **UP_TO_DATE**
- **If different →** repo was **UPDATED**

### 3.6 — Diff Generation

#### 3.6.1 — When UPDATED

Generate **two** diffs:

| Diff | Command | Output filename |
|------|---------|-----------------|
| N vs N-1 | `git diff <N-1>..<N>` | `diff_N_vs_N-1.patch` |
| N-1 vs N-2 | `git diff <N-2>..<N-1>` | `diff_N-1_vs_N-2.patch` |

Where:
- `N` = `post_pull_hash`
- `N-1` = `git rev-parse HEAD~1`
- `N-2` = `git rev-parse HEAD~2`

If `HEAD~1` or `HEAD~2` do not exist (repo has fewer than 2/3 commits), skip that diff and note it in the report.

Additionally, generate a **file change manifest** per diff:

```
git diff --stat <A>..<B>
git diff --name-status <A>..<B>
```

Store as `changes_N_vs_N-1.txt` and `changes_N-1_vs_N-2.txt`.

#### 3.6.2 — When UP_TO_DATE

Still optionally generate N-1 vs N-2 diff (controlled by `--history-depth`, default: generate it).

---

### 3.7 — Diff Summary Extraction with Code Snippets

For **every** generated diff, the tool **parses the unified diff output** and produces a structured **diff summary** that enumerates every changed element **together with the actual code that changed**.

#### 3.7.1 — Extraction Pipeline

```
Raw .patch
    → Parse unified diff
    → Identify hunks per file
    → Classify file type
    → Extract changed elements
    → Attach code snippets (before/after/diff) to each element
    → Run security pattern tagger
    → Build summary
```

#### 3.7.2 — Per-File Extraction

For each file in a diff:

| Extracted Field | Source |
|----------------|--------|
| File path | Diff header (`a/...` / `b/...`) |
| Change type | `git diff --name-status` → `A`dded, `M`odified, `D`eleted, `R`enamed, `C`opied |
| Language | Inferred from file extension (see §3.7.5) |
| Hunks | Each `@@` block in the unified diff |
| Lines added | Count of `+` lines |
| Lines removed | Count of `-` lines |
| Net change | `added - removed` |
| Changed elements | Functions, types, constants etc. touched (see §3.7.3) |
| Raw hunk code | Verbatim diff lines for each hunk (see §3.7.4) |

#### 3.7.3 — Changed Element Detection

The tool parses each hunk and extracts **what semantic code elements** were touched. Detection is done via **line-level pattern matching** on the diff content and the `@@` hunk header's function context (Git already provides this in the `@@ -a,b +c,d @@ fn_name` syntax).

**Element categories:**

| Category | Examples | Detection Method |
|----------|----------|-----------------|
| **Function/Method** | `fn foo()`, `def bar()`, `function baz()`, `func qux()` | Hunk header context + regex on `+`/`-` lines for function signatures |
| **Struct/Class/Type** | `struct Foo`, `class Bar`, `type Baz`, `interface Qux` | Regex on `+`/`-` lines |
| **Enum** | `enum Status`, `enum class Foo` | Regex on `+`/`-` lines |
| **Trait/Interface/Protocol** | `trait Foo`, `interface Bar`, `protocol Baz` | Regex on `+`/`-` lines |
| **Impl block** | `impl Foo for Bar` | Regex on `+`/`-` lines |
| **Constant/Static** | `const X`, `static Y`, `let Z =` (top-level) | Regex on `+`/`-` lines |
| **Import/Use** | `use crate::`, `import`, `from x import`, `#include` | Regex on `+`/`-` lines |
| **Module declaration** | `mod foo`, `module.exports`, `package` | Regex on `+`/`-` lines |
| **Configuration block** | TOML sections `[dependencies]`, YAML keys, JSON top-level keys | Key-level diffing |
| **Test** | `#[test]`, `#[cfg(test)]`, `describe(`, `it(`, `test_` prefix | Regex on `+`/`-` lines |
| **Macro** | `macro_rules!`, `#define` | Regex on `+`/`-` lines |
| **Other** | Lines that don't match any known pattern | Grouped as "body changes in <context>" |

**Change type classification:**
- **Added**: element signature appears only in `+` lines
- **Removed**: element signature appears only in `-` lines
- **Modified**: element exists in both old and new, but hunk touches lines within its body

#### 3.7.4 — Code Snippet Extraction (NEW — Core Feature)

For **every** detected changed element, the tool extracts **three code representations**:

##### 3.7.4.1 — Snippet Types

| Snippet Type | Description | Content |
|-------------|-------------|---------|
| **`before`** | The code **as it was** before the change | All `-` lines and context lines belonging to this element's hunk(s), with `-` prefix stripped. `null` for Added elements. |
| **`after`** | The code **as it is now** after the change | All `+` lines and context lines belonging to this element's hunk(s), with `+` prefix stripped. `null` for Removed elements. |
| **`diff`** | The raw unified diff lines for this element | Verbatim `+`, `-`, and ` ` (context) lines from the hunk(s), preserving the diff format exactly. |

##### 3.7.4.2 — Context Window

Each snippet includes surrounding context for security review comprehension:

- **Default context**: 5 lines above and below the changed lines (configurable via `--snippet-context <N>`, default `5`).
- If the element is a complete function/struct/block, the tool attempts to capture the **entire element body** (up to `--max-snippet-lines`, default `200` lines). This uses bracket/indentation tracking to find the element's boundaries.
- For **Modified** elements, both the old and new complete element bodies are captured when possible.

##### 3.7.4.3 — Full File Retrieval for Snippets

When bracket/indentation tracking is needed to capture full element bodies:

```bash
# Get old version of the file
git show <from_commit>:<file_path>

# Get new version of the file
git show <to_commit>:<file_path>
```

The tool uses these complete file contents to:
1. Locate the element boundary (open/close braces, indentation level).
2. Extract the full element from both versions.
3. Produce a clean before/after comparison.

##### 3.7.4.4 — Snippet Data Structure

```rust
#[derive(Debug, Clone, Serialize)]
pub struct CodeSnippet {
    /// The code before the change (None if element was Added)
    pub before: Option<SnippetContent>,
    /// The code after the change (None if element was Removed)
    pub after: Option<SnippetContent>,
    /// Raw unified diff lines for this element
    pub diff_lines: String,
    /// Was the full element body captured, or just the hunk window?
    pub capture_scope: CaptureScope,
}

#[derive(Debug, Clone, Serialize)]
pub struct SnippetContent {
    /// The actual source code lines
    pub code: String,
    /// Starting line number in the file
    pub start_line: u32,
    /// Ending line number in the file
    pub end_line: u32,
    /// The commit hash this code is from
    pub commit: String,
}

#[derive(Debug, Clone, Serialize)]
pub enum CaptureScope {
    /// Captured the entire element (function body, struct definition, etc.)
    FullElement,
    /// Captured only the diff hunk with context lines
    HunkWithContext { context_lines: u32 },
    /// Diff only, could not resolve element boundaries
    DiffOnly,
}
```

##### 3.7.4.5 — Snippet Example (JSON)

```json
{
  "kind": "Function",
  "name": "validate_token",
  "change_type": "Added",
  "file_path": "src/auth/jwt.rs",
  "lines_added": 15,
  "lines_removed": 0,
  "enclosing_context": "impl AuthHandler",
  "signature": "pub fn validate_token(&self, token: &str) -> Result<Claims, AuthError>",
  "snippet": {
    "before": null,
    "after": {
      "code": "    pub fn validate_token(&self, token: &str) -> Result<Claims, AuthError> {\n        let key = DecodingKey::from_secret(self.secret.as_bytes());\n        let validation = Validation::new(Algorithm::HS256);\n        let token_data = decode::<Claims>(token, &key, &validation)\n            .map_err(|e| AuthError::InvalidToken(e.to_string()))?;\n        if token_data.claims.exp < Utc::now().timestamp() as usize {\n            return Err(AuthError::TokenExpired);\n        }\n        Ok(token_data.claims)\n    }",
      "start_line": 45,
      "end_line": 54,
      "commit": "def5678abc..."
    },
    "diff_lines": "+    pub fn validate_token(&self, token: &str) -> Result<Claims, AuthError> {\n+        let key = DecodingKey::from_secret(self.secret.as_bytes());\n+        let validation = Validation::new(Algorithm::HS256);\n+        let token_data = decode::<Claims>(token, &key, &validation)\n+            .map_err(|e| AuthError::InvalidToken(e.to_string()))?;\n+        if token_data.claims.exp < Utc::now().timestamp() as usize {\n+            return Err(AuthError::TokenExpired);\n+        }\n+        Ok(token_data.claims)\n+    }",
    "capture_scope": "FullElement"
  },
  "security_tags": ["crypto", "authentication", "token-validation"]
}
```

##### 3.7.4.6 — Snippet Example (Modified Function)

```json
{
  "kind": "Function",
  "name": "check_permissions",
  "change_type": "Modified",
  "file_path": "src/middleware/auth.rs",
  "lines_added": 3,
  "lines_removed": 5,
  "enclosing_context": "impl AuthMiddleware",
  "signature": "pub async fn check_permissions(&self, user: &User, resource: &Resource) -> bool",
  "snippet": {
    "before": {
      "code": "    pub async fn check_permissions(&self, user: &User, resource: &Resource) -> bool {\n        if user.is_admin() {\n            return true;\n        }\n        let allowed_roles = self.acl.get_roles(resource);\n        allowed_roles.iter().any(|r| user.has_role(r))\n    }",
      "start_line": 78,
      "end_line": 84,
      "commit": "abc1234def..."
    },
    "after": {
      "code": "    pub async fn check_permissions(&self, user: &User, resource: &Resource) -> bool {\n        let allowed_roles = self.acl.get_roles(resource);\n        allowed_roles.iter().any(|r| user.has_role(r))\n    }",
      "start_line": 78,
      "end_line": 81,
      "commit": "def5678abc..."
    },
    "diff_lines": "     pub async fn check_permissions(&self, user: &User, resource: &Resource) -> bool {\n-        if user.is_admin() {\n-            return true;\n-        }\n         let allowed_roles = self.acl.get_roles(resource);\n         allowed_roles.iter().any(|r| user.has_role(r))\n     }",
    "capture_scope": "FullElement"
  },
  "security_tags": ["authorization", "access-control", "privilege-change"]
}
```

---

### 3.8 — Security Tagging (NEW — Core Feature)

Every extracted element with its code snippet is automatically scanned for **security-relevant patterns**. This is **heuristic, not a replacement for a security scanner** — it flags changes that *deserve human or automated attention*.

#### 3.8.1 — Security Tag Categories

| Tag | Triggers on (patterns in code or file path) |
|-----|---------------------------------------------|
| `crypto` | `encrypt`, `decrypt`, `hash`, `hmac`, `sha`, `md5`, `aes`, `rsa`, `sign`, `verify`, `digest`, `cipher`, `bcrypt`, `scrypt`, `argon2`, `pbkdf`, `salt`, `nonce`, `iv` |
| `authentication` | `login`, `logout`, `authenticate`, `auth`, `credential`, `password`, `passwd`, `token`, `jwt`, `oauth`, `saml`, `session`, `cookie`, `bearer` |
| `authorization` | `permission`, `role`, `acl`, `rbac`, `policy`, `authorize`, `allowed`, `denied`, `forbidden`, `privilege`, `access_control`, `can_access`, `is_admin`, `is_authorized` |
| `token-validation` | `validate_token`, `verify_token`, `decode.*token`, `token.*expir`, `refresh_token`, `claims` |
| `input-validation` | `sanitize`, `validate`, `escape`, `encode`, `filter`, `whitelist`, `blacklist`, `allowlist`, `blocklist`, `regex.*input`, `parse.*input`, `user_input`, `untrusted` |
| `sql-injection` | `query`, `execute`, `raw_sql`, `sql!`, `format!.*SELECT`, `format!.*INSERT`, `format!.*UPDATE`, `format!.*DELETE`, `string.*concatenat.*query`, `.query(&format` |
| `xss` | `innerHTML`, `outerHTML`, `document.write`, `dangerouslySetInnerHTML`, `v-html`, `\|safe`, `raw\|`, `Markup::`, `Html::raw` |
| `secrets` | `secret`, `api_key`, `apikey`, `private_key`, `access_key`, `password`, `credentials`, `connection_string`, `database_url`, `.env`, `dotenv`, `vault` |
| `network` | `http`, `https`, `url`, `endpoint`, `request`, `fetch`, `axios`, `cors`, `proxy`, `redirect`, `tls`, `ssl`, `certificate`, `socket`, `listen`, `bind` |
| `file-system` | `open`, `read_file`, `write_file`, `path.*join`, `path.*traverse`, `../`, `fs::`, `File::`, `unlink`, `chmod`, `chown`, `tempfile`, `symlink` |
| `deserialization` | `deserialize`, `unmarshal`, `pickle`, `yaml.*load`, `from_json`, `from_bytes`, `serde`, `bincode`, `eval`, `exec` |
| `error-handling` | `unwrap()`, `expect(`, `panic!`, `unreachable!`, `.unwrap_or`, `catch`, `rescue`, `except`, `try!`, `?` operator removal, `unsafe` |
| `unsafe-code` | `unsafe {`, `unsafe fn`, `unsafe impl`, `#[allow(unsafe`, `*mut`, `*const`, `transmute`, `from_raw`, `as_ptr` |
| `dependency-change` | Changes in `Cargo.toml`, `package.json`, `go.mod`, `requirements.txt`, `Gemfile`, `pom.xml`, `build.gradle` |
| `ci-cd` | Changes in `.github/workflows/`, `.gitlab-ci.yml`, `Jenkinsfile`, `Dockerfile`, `docker-compose`, `.env`, `Makefile`, `deploy` |
| `logging` | `log`, `debug`, `trace`, `print`, `println`, `console.log`, `logger`, removal of logging lines |
| `concurrency` | `mutex`, `lock`, `atomic`, `thread`, `async`, `await`, `spawn`, `channel`, `sync`, `race`, `deadlock`, `semaphore` |
| `privilege-change` | `sudo`, `root`, `admin`, `superuser`, `escalat`, `setuid`, `capability`, `is_admin`, `role.*admin` |

#### 3.8.1.1 — Noise Reduction

To avoid overwhelming reports with false positives on common patterns:

- **Minimum match threshold**: Tags with broad patterns (e.g., `network`, `error-handling`, `logging`) require **2+ distinct pattern matches** within the same element to trigger. Tags with specific patterns (e.g., `crypto`, `unsafe-code`, `sql-injection`) trigger on a single match.
- **Negative patterns**: Each tag definition supports an optional `negative_patterns` list. If a negative pattern matches, the tag is suppressed. Example: `network` tag suppresses on `test_url`, `mock_request`, `example.com`.
- **File-path exclusions**: Elements in `test/`, `tests/`, `spec/`, `__tests__/`, `*_test.*`, `*_spec.*` paths are tagged but marked with `"in_test": true` and excluded from `high_attention_items` by default (overridable with `--include-test-security`).

#### 3.8.2 — Tagging Rules

1. **Pattern match** is case-insensitive.
2. Patterns match against:
   - The `diff_lines` content (the actual changed code)
   - The `signature` of the element
   - The `file_path`
   - The `name` of the element
3. A single element can have **multiple** tags.
4. Tags are **additive** — the tool never suppresses a tag.
5. **Removed** elements with security tags are tagged with an additional `security-removal` meta-tag to flag that a security control may have been deleted.
6. The security tags are **purely informational** — the tool does not block, fail, or warn on any specific tag.

#### 3.8.3 — Security Summary (per diff)

A dedicated section in the summary aggregates all security-tagged elements:

```json
{
  "security_review": {
    "total_security_tagged_elements": 4,
    "by_tag": {
      "authentication": 2,
      "authorization": 1,
      "crypto": 1,
      "privilege-change": 1,
      "security-removal": 1
    },
    "high_attention_items": [
      {
        "reason": "Security control REMOVED",
        "element": "check_permissions → admin bypass removed",
        "file": "src/middleware/auth.rs",
        "tags": ["authorization", "privilege-change", "security-removal"],
        "snippet_ref": "elements[3]"
      },
      {
        "reason": "New crypto/auth code added",
        "element": "validate_token",
        "file": "src/auth/jwt.rs",
        "tags": ["crypto", "authentication", "token-validation"],
        "snippet_ref": "elements[0]"
      }
    ],
    "flagged_elements": [ /* full list of ChangedElement objects with security_tags */ ]
  }
}
```

#### 3.8.4 — Security Summary (human-readable)

Appended to `summary_*.txt` and `summary_*.md`:

```
──────────────────────────────────────────────

🔒 SECURITY REVIEW FLAGS
========================

⚠ HIGH ATTENTION (2):

  1. REMOVED: admin bypass in check_permissions
     File:   src/middleware/auth.rs
     Tags:   authorization, privilege-change, security-removal
     Before:
     │  if user.is_admin() {
     │      return true;
     │  }
     After:  (lines removed entirely)

  2. ADDED: new token validation logic
     File:   src/auth/jwt.rs
     Tags:   crypto, authentication, token-validation
     Code:
     │  let key = DecodingKey::from_secret(self.secret.as_bytes());
     │  let validation = Validation::new(Algorithm::HS256);
     │  let token_data = decode::<Claims>(token, &key, &validation)
     │      .map_err(|e| AuthError::InvalidToken(e.to_string()))?;

  All security-tagged changes (4):
    🔐 validate_token            ✚ Added      src/auth/jwt.rs         crypto, authentication
    🔐 check_permissions         ✎ Modified   src/middleware/auth.rs   authorization, privilege-change
    🔐 hash_password             ✎ Modified   src/auth/password.rs    crypto
    🔐 [dep] jsonwebtoken        ✚ Added      Cargo.toml              dependency-change
```

---

### 3.9 — Report Output Structure

```
<report_dir>/
├── summary.json                                 # global summary across all repos
├── summary.txt                                  # human-readable global summary
├── summary.md                                   # markdown global summary
├── security_overview.json                       # aggregated security flags across ALL repos
├── security_overview.txt                        # human-readable security overview
├── security_overview.md                         # markdown security overview
├── <repo-name-1>/                               # UPDATED repo
│   ├── status.json
│   ├── status.txt
│   ├── pull_log.txt
│   └── diffs/
│       ├── diff_N_vs_N-1.patch                  # raw unified diff
│       ├── diff_N-1_vs_N-2.patch
│       ├── changes_N_vs_N-1.txt                 # git diff --stat + --name-status
│       ├── changes_N-1_vs_N-2.txt
│       ├── summary_N_vs_N-1.json                # element summary with snippets
│       ├── summary_N_vs_N-1.txt                 # human-readable with snippets
│       ├── summary_N_vs_N-1.md                  # markdown with snippets
│       ├── summary_N-1_vs_N-2.json
│       ├── summary_N-1_vs_N-2.txt
│       ├── summary_N-1_vs_N-2.md
│       └── snippets/                            # individual snippet files
│           ├── 001_validate_token_ADDED.rs       # clean source file of new element
│           ├── 002_check_permissions_BEFORE.rs   # old version
│           ├── 002_check_permissions_AFTER.rs    # new version
│           ├── 002_check_permissions.diff        # isolated diff for this element
│           └── ...
├── <repo-name-2>/                               # UP_TO_DATE repo
│   ├── status.json
│   ├── status.txt
│   └── pull_log.txt
└── ...
```

#### `snippets/` Directory

Each changed element gets **individual files** in addition to being embedded in the JSON/TXT/MD summaries. This enables:
- Piping individual snippets to security scanning tools.
- Grepping across all snippet files.
- Feeding snippets to LLM-based review tools one at a time.

**Naming convention:**
```
<NNN>_<element_name>_<change_type>.<ext>
```

Where:
- `NNN` = zero-padded sequence number
- `element_name` = sanitized element name (alphanumeric + underscore)
- `change_type` = `ADDED`, `BEFORE`, `AFTER`, `REMOVED`
- `ext` = original file extension (`.rs`, `.py`, `.js`, etc.) or `.diff` for the diff view

For **Modified** elements, three files are emitted: `_BEFORE.<ext>`, `_AFTER.<ext>`, `_.<ext>.diff`.

> **Deduplication**: Snippets are stored as individual files in `snippets/` and referenced by path in `summary_*.json`. The `summary_*.txt` and `summary_*.md` files reference snippet files rather than inlining full code to avoid 4× data duplication. Short previews (first 5 lines) are still inlined for quick scanning.

#### Naming Collisions

If two repos share the same directory name (e.g., `projects/a/mylib` and `vendors/b/mylib`), disambiguate by encoding the relative path with `--` as separator:

```
projects--a--mylib/
vendors--b--mylib/
```

### 3.10 — Cross-Repo Security Overview

The top-level `security_overview.*` files aggregate security-tagged changes from **all** repos into a single view:

#### `security_overview.json`

```json
{
  "timestamp": "2025-01-15T10:30:00Z",
  "total_repos_scanned": 12,
  "repos_with_security_flags": 3,
  "total_security_tagged_elements": 11,
  "by_tag_global": {
    "authentication": 4,
    "authorization": 2,
    "crypto": 3,
    "dependency-change": 5,
    "unsafe-code": 1,
    "security-removal": 1
  },
  "high_attention_items": [
    {
      "repo": "api-server",
      "reason": "Security control REMOVED",
      "element": "check_permissions → admin bypass removed",
      "file": "src/middleware/auth.rs",
      "tags": ["authorization", "privilege-change", "security-removal"],
      "before_code": "if user.is_admin() { return true; }",
      "after_code": null,
      "commit_from": "abc1234",
      "commit_to": "def5678"
    }
  ],
  "repos": [
    {
      "name": "api-server",
      "security_elements": 4,
      "tags": ["authentication", "authorization", "crypto", "security-removal"],
      "detail_path": "api-server/diffs/summary_N_vs_N-1.json"
    },
    {
      "name": "shared-utils",
      "security_elements": 2,
      "tags": ["crypto", "dependency-change"],
      "detail_path": "shared-utils/diffs/summary_N_vs_N-1.json"
    }
  ]
}
```

#### `security_overview.txt`

```
🔒 GIT PATROL — SECURITY OVERVIEW
===================================
Scanned: 12 repos | 2025-01-15 10:30:00 UTC
Repos with security-relevant changes: 3
Total security-tagged elements: 11

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

⚠ HIGH ATTENTION ITEMS:

  1. [api-server] REMOVED: admin bypass in check_permissions
     File:   src/middleware/auth.rs
     Commit: abc1234 → def5678
     Tags:   authorization, privilege-change, security-removal
     Code removed:
     │  if user.is_admin() {
     │      return true;
     │  }

  2. [api-server] ADDED: validate_token
     File:   src/auth/jwt.rs
     Tags:   crypto, authentication, token-validation
     Code added:
     │  let key = DecodingKey::from_secret(self.secret.as_bytes());
     │  let validation = Validation::new(Algorithm::HS256);
     │  ...

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

ALL SECURITY-TAGGED CHANGES BY REPO:

  api-server (4 elements):
    🔐 ✚ validate_token               crypto, authentication
    🔐 ✎ check_permissions            authorization, security-removal
    🔐 ✎ hash_password                crypto
    🔐 ✚ [dep] jsonwebtoken           dependency-change

  shared-utils (2 elements):
    🔐 ✎ encrypt_payload              crypto
    🔐 ✚ [dep] ring                   dependency-change

  frontend (5 elements):
    🔐 ✎ fetchUserData                authentication, network
    🔐 ✚ renderHtml                   xss
    🔐 ✎ setAuthCookie                authentication, secrets
    🔐 ✖ validateCSRFToken            security-removal
    🔐 ✎ [dep] axios                  dependency-change
```

---

### 3.11 — Report File Formats (Per-Repo Detail)

#### `status.json` (per repo)

```json
{
  "repo_path": "/home/user/projects/mylib",
  "repo_name": "mylib",
  "branch": "main",
  "status": "UPDATED",
  "pre_pull": {
    "hash": "abc1234...",
    "short_hash": "abc1234",
    "message": "fix: resolve race condition",
    "author": "Alice <alice@example.com>",
    "timestamp": "2025-01-14T18:00:00Z"
  },
  "post_pull": {
    "hash": "def5678...",
    "short_hash": "def5678",
    "message": "feat: add JWT token validation",
    "author": "Bob <bob@example.com>",
    "timestamp": "2025-01-15T09:00:00Z"
  },
  "diffs": [
    {
      "label": "N_vs_N-1",
      "from_hash": "abc1234...",
      "to_hash": "def5678...",
      "files_changed": 5,
      "insertions": 42,
      "deletions": 13,
      "elements_changed": 8,
      "elements_added": 3,
      "elements_modified": 3,
      "elements_removed": 2,
      "security_tagged_elements": 4,
      "patch_file": "diffs/diff_N_vs_N-1.patch",
      "changes_file": "diffs/changes_N_vs_N-1.txt",
      "summary_json": "diffs/summary_N_vs_N-1.json",
      "summary_txt": "diffs/summary_N_vs_N-1.txt",
      "summary_md": "diffs/summary_N_vs_N-1.md",
      "snippets_dir": "diffs/snippets/"
    }
  ],
  "errors": [],
  "timestamp": "2025-01-15T10:30:00Z"
}
```

#### `status.txt` (per repo — UP_TO_DATE example)

```
Repository: /home/user/projects/mylib
Branch:     main
Status:     UP TO DATE

Commit Hash:    abc1234abcdef1234abcdef1234abcdef1234abcd
Commit Message: fix: resolve race condition
Author:         Alice <alice@example.com>

No new changes pulled.
Scanned at: 2025-01-15T10:30:00Z
```

#### Per-Diff `summary_N_vs_N-1.json` (extended with snippets)

```json
{
  "diff_label": "N_vs_N-1",
  "from_commit": { "hash": "abc1234...", "message": "fix: resolve race condition" },
  "to_commit": { "hash": "def5678...", "message": "feat: add JWT token validation" },
  "total_files_changed": 5,
  "total_insertions": 42,
  "total_deletions": 13,
  "total_elements_changed": 8,
  "files": [
    {
      "path": "src/auth/jwt.rs",
      "change_type": "Added",
      "language": "Rust",
      "insertions": 55,
      "deletions": 0,
      "elements": [
        {
          "kind": "Function",
          "name": "validate_token",
          "change_type": "Added",
          "file_path": "src/auth/jwt.rs",
          "line_range": [45, 54],
          "lines_added": 10,
          "lines_removed": 0,
          "enclosing_context": "impl AuthHandler",
          "signature": "pub fn validate_token(&self, token: &str) -> Result<Claims, AuthError>",
          "snippet": {
            "before": null,
            "after": {
              "code": "    pub fn validate_token(&self, token: &str) -> Result<Claims, AuthError> {\n        let key = DecodingKey::from_secret(self.secret.as_bytes());\n        let validation = Validation::new(Algorithm::HS256);\n        let token_data = decode::<Claims>(token, &key, &validation)\n            .map_err(|e| AuthError::InvalidToken(e.to_string()))?;\n        if token_data.claims.exp < Utc::now().timestamp() as usize {\n            return Err(AuthError::TokenExpired);\n        }\n        Ok(token_data.claims)\n    }",
              "start_line": 45,
              "end_line": 54,
              "commit": "def5678..."
            },
            "diff_lines": "+    pub fn validate_token(...) ...",
            "capture_scope": "FullElement"
          },
          "security_tags": ["crypto", "authentication", "token-validation"],
          "snippet_files": {
            "after": "snippets/001_validate_token_ADDED.rs",
            "diff": "snippets/001_validate_token.diff"
          }
        }
      ]
    }
  ],
  "element_summary": {
    "total_elements": 8,
    "by_change_type": { "Added": 3, "Modified": 3, "Removed": 2 },
    "by_kind": {
      "Function": { "added": 1, "modified": 1, "removed": 0 },
      "Struct": { "added": 0, "modified": 1, "removed": 0 },
      "Impl": { "added": 1, "modified": 0, "removed": 0 },
      "Config": { "added": 1, "modified": 1, "removed": 0 },
      "Import": { "added": 2, "modified": 0, "removed": 0 }
    },
    "top_elements": ["validate_token", "check_permissions", "UserProfile"]
  },
  "security_review": {
    "total_security_tagged_elements": 4,
    "by_tag": {
      "authentication": 2,
      "authorization": 1,
      "crypto": 1,
      "privilege-change": 1,
      "security-removal": 1
    },
    "high_attention_items": [ "..." ],
    "flagged_elements": [ "..." ]
  }
}
```

#### Per-Diff `summary_N_vs_N-1.txt` (with inline snippets)

```
Diff Summary: N vs N-1
======================
From: abc1234 — "fix: resolve race condition"
To:   def5678 — "feat: add JWT token validation"

5 files changed, +42 insertions, -13 deletions
8 code elements changed | 4 security-tagged

──────────────────────────────────────────────

FILES CHANGED:
  A  src/auth/jwt.rs                (+55)
  M  src/server/handler.rs          (+20 -5)
  M  src/models/user.rs             (+5 -2)
  M  Cargo.toml                     (+2 -1)
  D  src/legacy/old_auth.rs         (-5)

──────────────────────────────────────────────

ELEMENTS CHANGED (with code snippets):

  ┌─────────────────────────────────────────┐
  │ ✚ ADDED: fn validate_token              │
  │ File: src/auth/jwt.rs:45-54             │
  │ Context: impl AuthHandler               │
  │ 🔐 Tags: crypto, authentication         │
  ├─────────────────────────────────────────┤
  │ NEW CODE:                               │
  │                                         │
  │  45 │ pub fn validate_token(            │
  │  46 │     &self,                        │
  │  47 │     token: &str,                  │
  │  48 │ ) -> Result<Claims, AuthError> {  │
  │  49 │     let key = DecodingKey::       │
  │     │         from_secret(              │
  │     │         self.secret.as_bytes());  │
  │  50 │     let validation =              │
  │     │         Validation::new(          │
  │     │         Algorithm::HS256);        │
  │  51 │     let token_data =              │
  │     │         decode::<Claims>(         │
  │     │         token, &key,              │
  │     │         &validation)              │
  │     │         .map_err(|e|              │
  │     │         AuthError::               │
  │     │         InvalidToken(             │
  │     │         e.to_string()))?;         │
  │  52 │     if token_data.claims.exp      │
  │     │         < Utc::now()              │
  │     │         .timestamp() as usize {   │
  │  53 │         return Err(               │
  │     │         AuthError::TokenExpired); │
  │  54 │     }                             │
  │  55 │     Ok(token_data.claims)         │
  │  56 │ }                                 │
  └─────────────────────────────────────────┘

  ┌─────────────────────────────────────────┐
  │ ✎ MODIFIED: fn check_permissions        │
  │ File: src/middleware/auth.rs:78-84      │
  │ Context: impl AuthMiddleware            │
  │ 🔐 Tags: authorization, privilege-change│
  │ 🔐 Tags: security-removal              │
  ├─────────────────────────────────────────┤
  │ BEFORE (abc1234):                       │
  │                                         │
  │  78 │ pub async fn check_permissions(   │
  │     │     &self, user: &User,           │
  │     │     resource: &Resource           │
  │     │ ) -> bool {                       │
  │  79 │     if user.is_admin() {          │
  │  80 │         return true;              │
  │  81 │     }                             │
  │  82 │     let allowed_roles =           │
  │     │         self.acl                  │
  │     │         .get_roles(resource);     │
  │  83 │     allowed_roles.iter()          │
  │     │         .any(|r| user.has_role(r))│
  │  84 │ }                                 │
  │                                         │
  │ AFTER (def5678):                        │
  │                                         │
  │  78 │ pub async fn check_permissions(   │
  │     │     &self, user: &User,           │
  │     │     resource: &Resource           │
  │     │ ) -> bool {                       │
  │  79 │     let allowed_roles =           │
  │     │         self.acl                  │
  │     │         .get_roles(resource);     │
  │  80 │     allowed_roles.iter()          │
  │     │         .any(|r| user.has_role(r))│
  │  81 │ }                                 │
  │                                         │
  │ DIFF:                                   │
  │  -    if user.is_admin() {              │
  │  -        return true;                  │
  │  -    }                                 │
  └─────────────────────────────────────────┘

  ┌─────────────────────────────────────────┐
  │ ✎ MODIFIED: struct UserProfile          │
  │ File: src/models/user.rs:12-20         │
  │ (no security tags)                      │
  ├─────────────────────────────────────────┤
  │ DIFF:                                   │
  │   pub struct UserProfile {              │
  │       pub id: Uuid,                     │
  │       pub name: String,                 │
  │  +    pub email_verified: bool,         │
  │  +    pub last_login: Option<DateTime>, │
  │  -    pub active: bool,                 │
  │   }                                     │
  └─────────────────────────────────────────┘

  ... (remaining elements)

──────────────────────────────────────────────

ELEMENT TOTALS:
  Added:     3 (1 function, 1 impl, 1 config)
  Modified:  3 (1 function, 1 struct, 1 config)
  Removed:   2 (1 function, 1 import)

──────────────────────────────────────────────

🔒 SECURITY REVIEW FLAGS
========================

⚠ HIGH ATTENTION (2):

  1. REMOVED: admin bypass in check_permissions
     Authorization logic was removed — admin users
     no longer bypass permission checks.
     File: src/middleware/auth.rs

  2. ADDED: token validation with HS256
     New JWT validation logic uses symmetric key.
     Verify key management is appropriate.
     File: src/auth/jwt.rs

All security-tagged (4): see above element details.

LEGEND: ✚ Added  ✎ Modified  ✖ Removed  🔐 Security-tagged
```

#### Top-Level `summary.json`

```json
{
  "scan_root": "/home/user/projects",
  "report_dir": "/home/user/patrol-report",
  "timestamp": "2025-01-15T10:30:00Z",
  "total_repos_found": 12,
  "updated": 3,
  "up_to_date": 7,
  "dirty_skipped": 1,
  "pull_failed": 1,
  "total_elements_changed_across_all_repos": 24,
  "total_security_tagged_elements": 11,
  "repos": [
    {
      "name": "mylib",
      "path": "/home/user/projects/mylib",
      "status": "UPDATED",
      "branch": "main",
      "latest_diff": {
        "files_changed": 5,
        "insertions": 42,
        "deletions": 13,
        "elements_added": 3,
        "elements_modified": 3,
        "elements_removed": 2,
        "security_tagged": 4,
        "top_elements": [
          "✚ fn validate_token (src/auth/jwt.rs) 🔐",
          "✎ fn check_permissions (src/middleware/auth.rs) 🔐",
          "✎ struct UserProfile (src/models/user.rs)"
        ]
      }
    }
  ]
}
```

#### Top-Level `summary.txt`

```
Git Patrol Report
=================
Root:     /home/user/projects
Date:     2025-01-15 10:30:00 UTC
Repos:    12 found
Elements: 24 changed across all repos
Security: 11 security-tagged elements in 3 repos

UPDATED (3):
  ✓ mylib          main  abc1234 → def5678  (+42 -13, 5 files, 8 elements, 4🔐)
    ├─ 🔐 ✚ fn validate_token               src/auth/jwt.rs
    ├─ 🔐 ✎ fn check_permissions             src/middleware/auth.rs
    ├─ ✎ struct UserProfile                  src/models/user.rs
    └─ ... 5 more elements

  ✓ api-server     main  111aaaa → 222bbbb  (+100 -20, 12 files, 15 elements, 3🔐)
    ├─ 🔐 ✖ fn deprecated_handler           src/legacy.rs
    ├─ 🔐 ✚ fn handle_webhook               src/webhooks.rs
    └─ ... 13 more elements

  ✓ shared-utils   dev   333cccc → 444dddd  (+5 -2, 1 file, 1 element)
    └─ ✎ fn parse_config                     src/config.rs

UP TO DATE (7):
  — frontend       main  aaa1111
  — docs           main  bbb2222
  ...

SKIPPED — DIRTY (1):
  ⚠ experiments    main  ccc3333  (uncommitted changes)

FAILED (1):
  ✗ legacy-app     main  ddd4444  (authentication required)
```

---

## 4. CLI Interface

```
diffcatcher [OPTIONS] <ROOT_DIR>
```

### Positional Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `ROOT_DIR` | Yes | Directory to scan recursively |

### Options

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--output <DIR>` | `-o` | `./reports/<timestamp>` | Report output directory |
| `--pull-strategy <STRATEGY>` | `-s` | `ff-only` | Pull strategy: `ff-only`, `rebase`, `merge` |
| `--timeout <SECONDS>` | `-t` | `120` | Timeout per repo for git operations |
| `--nested` | | `false` | Recurse into repos to find nested repos |
| `--follow-symlinks` | | `false` | Follow symbolic links during scan |
| `--skip-hidden` | | `false` | Skip hidden directories (dot-prefixed) except `.git` |
| `--pull` | | `false` | Actually pull (modify working tree) instead of fetch-only. Enables `--force-pull` and `--pull-strategy`. |
| `--force-pull` | | `false` | Stash dirty repos before pull, pop after (requires `--pull`) |
| `--no-pull` | | `false` | Skip fetching/pulling; only capture state and generate historical diffs |
| `--history-depth <N>` | `-d` | `2` | Number of historical commits to diff (min 1, max 10) |
| `--parallel <N>` | `-j` | `4` | Number of repos to process concurrently |
| `--quiet` | `-q` | `false` | Suppress stdout progress; only write report files |
| `--verbose` | `-v` | `false` | Print detailed git command output to terminal |
| `--dry-run` | | `false` | Discover repos and report state; do not pull or modify anything |
| `--json` | | `false` | Print final summary to stdout as JSON (for piping) |
| `--branch-filter <PATTERN>` | | `*` | Only process repos on branches matching glob pattern |
| `--no-summary-extraction` | | `false` | Skip element extraction; only produce raw diffs and file lists |
| `--no-snippets` | | `false` | Extract elements but do not capture code snippets |
| `--no-security-tags` | | `false` | Skip security pattern tagging |
| `--snippet-context <N>` | | `5` | Lines of context above/below changed lines in snippets |
| `--max-snippet-lines <N>` | | `200` | Max lines per individual snippet (truncate with note) |
| `--max-elements <N>` | | `500` | Max elements to extract per diff (safety cap) |
| `--summary-format <FORMATS>` | | `json,md` | Comma-separated list of summary formats to generate |
| `--incremental` | | `false` | Skip repos unchanged since the last run (requires previous report directory via `-o`) |
| `--security-tags-file <PATH>` | | (built-in) | Custom JSON file defining security tag patterns (overrides defaults) |
| `--help` | `-h` | | Print help |
| `--version` | `-V` | | Print version |

### Example Invocations

```bash
# Basic: scan ~/projects, full security report
diffcatcher ~/projects

# Custom output, force-pull dirty repos, 8 parallel
diffcatcher ~/projects -o ./report -j 8 --force-pull

# Dry run — just discover and report state, no modifications
diffcatcher ~/projects --dry-run -o ./snapshot

# Only repos on main branch, deeper history, more context
diffcatcher ~/projects --branch-filter "main" --history-depth 5 --snippet-context 10

# Quiet mode with JSON output for CI/CD piping
diffcatcher ~/projects -q --json > result.json

# Fast mode: skip snippets and security tagging
diffcatcher ~/projects --no-snippets --no-security-tags

# Custom security patterns for domain-specific review
diffcatcher ~/projects --security-tags-file ./our-security-patterns.json
```

---

## 5. Architecture

### 5.1 — Module Structure

```
src/
├── main.rs                         # CLI entrypoint, argument parsing
├── cli.rs                          # Clap argument definitions
├── scanner.rs                      # Recursive repo discovery (walkdir)
├── git/
│   ├── mod.rs
│   ├── commands.rs                 # Wrappers around git CLI invocations
│   ├── state.rs                    # Pre/post pull state structs
│   ├── diff.rs                     # Diff generation logic
│   └── file_retrieval.rs           # git show <commit>:<path> for full file content
├── extraction/
│   ├── mod.rs                      # Orchestrates element extraction
│   ├── parser.rs                   # Unified diff parser (hunks, headers)
│   ├── classifier.rs              # File extension → Language mapping
│   ├── elements.rs                 # Element detection logic
│   ├── snippets.rs                 # Code snippet extraction + element boundary detection
│   ├── boundary.rs                 # Bracket/indentation tracking for full element capture
│   └── languages/
│       ├── mod.rs
│       ├── rust.rs
│       ├── python.rs
│       ├── javascript.rs
│       ├── go.rs
│       ├── c_cpp.rs
│       ├── java_kotlin.rs
│       ├── ruby.rs
│       ├── config.rs
│       └── fallback.rs
├── security/
│   ├── mod.rs                      # Security tagging orchestration
│   ├── tagger.rs                   # Pattern matching engine
│   ├── patterns.rs                 # Built-in security pattern definitions
│   ├── custom.rs                   # Custom pattern file loader
│   └── overview.rs                 # Cross-repo security overview aggregation
├── processor.rs                    # Per-repo orchestration
├── report/
│   ├── mod.rs
│   ├── writer.rs                   # Writes report directory structure
│   ├── json.rs                     # JSON serialization (serde)
│   ├── text.rs                     # Human-readable text formatting
│   ├── markdown.rs                 # Markdown formatting
│   └── snippet_writer.rs           # Individual snippet file writer
├── types.rs                        # Shared types, enums
└── error.rs                        # Error types (thiserror)
```

### 5.2 — Core Pipeline (per repo)

```
┌───────────┐   ┌────────────┐   ┌──────────┐   ┌────────────┐   ┌──────────┐   ┌───────────┐   ┌──────────┐   ┌────────────┐
│ Discover  │──▶│ Capture    │──▶│ Git Pull │──▶│ Capture    │──▶│ Generate │──▶│ Extract   │──▶│ Security │──▶│ Write      │
│ .git dirs │   │ Pre-Pull   │   │ (or skip)│   │ Post-Pull  │   │ Diffs    │   │ Elements  │   │ Tagging  │   │ Report     │
└───────────┘   └────────────┘   └──────────┘   └────────────┘   └──────────┘   │ + Snippets│   └──────────┘   └────────────┘
                                                                                 └───────────┘
```

### 5.3 — Snippet Extraction Engine Detail

```
                    ┌──────────────────┐
                    │ Raw .patch file   │
                    └────────┬─────────┘
                             │
                    ┌────────▼─────────┐
                    │ Unified Diff     │  → splits into per-file diffs
                    │ Parser           │  → identifies hunk boundaries
                    └────────┬─────────┘  → extracts @@ context lines
                             │
              ┌──────────────┼──────────────┐
              │              │              │
     ┌────────▼───┐  ┌──────▼─────┐  ┌─────▼──────┐
     │ .rs file   │  │ .py file   │  │ .toml file │  ...
     │ Rust       │  │ Python     │  │ Config     │
     │ Patterns   │  │ Patterns   │  │ Patterns   │
     └────────┬───┘  └──────┬─────┘  └─────┬──────┘
              │              │              │
              └──────────────┼──────────────┘
                             │
                    ┌────────▼─────────┐
                    │ Element          │  → deduplicates
                    │ Aggregator       │  → classifies Added/Modified/Removed
                    └────────┬─────────┘
                             │
                    ┌────────▼─────────┐
                    │ Snippet          │  → for each element:
                    │ Extractor        │     1. get diff_lines from hunk
                    └────────┬─────────┘     2. git show old/new file
                             │               3. find element boundaries
                             │               4. extract before/after code
                    ┌────────▼─────────┐
                    │ Boundary         │  → uses bracket counting
                    │ Detector         │  → or indentation tracking
                    └────────┬─────────┘  → language-specific rules
                             │
                    ┌────────▼─────────┐
                    │ Security         │  → regex match against:
                    │ Tagger           │     - diff_lines
                    └────────┬─────────┘     - signature, name, path
                             │
                    ┌────────▼─────────┐
                    │ Summary +        │  → JSON, TXT, MD output
                    │ Snippet Writer   │  → individual snippet files
                    └──────────────────┘
```

### 5.4 — Concurrency Model

- Use **`rayon`** thread pool for parallel repo processing.
- Each repo is processed independently — no shared mutable state.
- Results collected into a `Vec<RepoResult>` then passed to report writer.
- Git operations use `std::process::Command` — each call is a child process, inherently thread-safe.
- Element extraction and security tagging are CPU-bound regex work, benefits from parallelism.
- File retrieval (`git show`) may add I/O; bounded by `--parallel`.

### 5.5 — Key Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` (v4, derive) | CLI argument parsing |
| `walkdir` | Recursive directory traversal |
| `rayon` | Parallel repo processing |
| `serde` + `serde_json` | JSON serialization |
| `chrono` | Timestamps |
| `thiserror` | Error types |
| `indicatif` | Progress bars (terminal UX) |
| `tracing` + `tracing-subscriber` | Structured logging |
| `regex` | Element + security pattern matching |
| `lazy_static` or `once_cell` | Compiled regex caching |
| `glob` | Branch filter pattern matching |
| `git2` | Git operations (state capture, fetch, diff generation, file retrieval) |

**Uses `git2` (libgit2 bindings)** for all git operations. This avoids per-operation process spawning overhead, provides structured error handling, and eliminates stdout parsing fragility. The `git` CLI binary is NOT required.

---

## 6. Data Types

```rust
// ── Repo-level types ──

#[derive(Debug, Clone, Serialize)]
pub enum RepoStatus {
    Updated,
    UpToDate,
    DirtySkipped,
    PullFailed { error: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct CommitInfo {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub full_message: String,
    pub author: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffResult {
    pub label: String,
    pub from_commit: CommitInfo,
    pub to_commit: CommitInfo,
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
    pub file_changes: Vec<FileChangeDetail>,
    pub element_summary: Option<ElementSummary>,
    pub security_review: Option<SecurityReview>,
    pub patch_filename: String,
    pub changes_filename: String,
    pub summary_json_filename: Option<String>,
    pub summary_txt_filename: Option<String>,
    pub summary_md_filename: Option<String>,
    pub snippets_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepoResult {
    pub repo_path: PathBuf,
    pub repo_name: String,
    pub report_folder_name: String,
    pub branch: String,
    pub status: RepoStatus,
    pub pre_pull: Option<CommitInfo>,
    pub post_pull: Option<CommitInfo>,
    pub diffs: Vec<DiffResult>,
    pub pull_log: String,
    pub errors: Vec<String>,
    pub timestamp: DateTime<Utc>,
}

// ── File-level types ──

#[derive(Debug, Clone, Serialize)]
pub struct FileChangeDetail {
    pub path: String,
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub language: Language,
    pub insertions: u32,
    pub deletions: u32,
    pub elements: Vec<ChangedElement>,
    pub raw_hunks: Vec<RawHunk>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RawHunk {
    pub header: String,               // the @@ line
    pub old_start: u32,
    pub old_count: u32,
    pub new_start: u32,
    pub new_count: u32,
    pub context_function: Option<String>, // function name from @@ header
    pub lines: String,                 // verbatim hunk content
}

#[derive(Debug, Clone, Serialize)]
pub enum FileStatus { Added, Modified, Deleted, Renamed, Copied }

#[derive(Debug, Clone, Serialize)]
pub enum Language {
    Rust, Python, JavaScript, TypeScript, Go, C, Cpp,
    Java, Kotlin, Ruby, Toml, Yaml, Json, Markdown,
    Shell, Dockerfile, Unknown(String),
}

// ── Element-level types ──

#[derive(Debug, Clone, Serialize)]
pub struct ChangedElement {
    pub kind: ElementKind,
    pub name: String,
    pub change_type: ChangeType,
    pub file_path: String,
    pub line_range: Option<(u32, u32)>,
    pub lines_added: u32,
    pub lines_removed: u32,
    pub enclosing_context: Option<String>,
    pub signature: Option<String>,
    pub snippet: CodeSnippet,
    pub security_tags: Vec<String>,
    pub snippet_files: Option<SnippetFileRefs>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodeSnippet {
    pub before: Option<SnippetContent>,
    pub after: Option<SnippetContent>,
    pub diff_lines: String,
    pub capture_scope: CaptureScope,
}

#[derive(Debug, Clone, Serialize)]
pub struct SnippetContent {
    pub code: String,
    pub start_line: u32,
    pub end_line: u32,
    pub commit: String,
}

#[derive(Debug, Clone, Serialize)]
pub enum CaptureScope {
    FullElement,
    HunkWithContext { context_lines: u32 },
    DiffOnly,
    Truncated { actual_lines: u32, max_lines: u32 },
}

#[derive(Debug, Clone, Serialize)]
pub struct SnippetFileRefs {
    pub before: Option<String>,  // path relative to repo report folder
    pub after: Option<String>,
    pub diff: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
pub enum ElementKind {
    Function, Method, Struct, Class, Enum, Trait, Interface,
    Impl, Module, Import, Constant, Static, TypeAlias,
    Macro, Test, Config, Other,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
pub enum ChangeType { Added, Modified, Removed }

// ── Summary types ──

#[derive(Debug, Clone, Serialize, Default)]
pub struct ElementSummary {
    pub total_elements: u32,
    pub by_change_type: HashMap<ChangeType, u32>,
    pub by_kind: HashMap<ElementKind, KindCounts>,
    pub elements: Vec<ChangedElement>,
    pub top_elements: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct KindCounts {
    pub added: u32,
    pub modified: u32,
    pub removed: u32,
}

// ── Security types ──

#[derive(Debug, Clone, Serialize)]
pub struct SecurityReview {
    pub total_security_tagged_elements: u32,
    pub by_tag: HashMap<String, u32>,
    pub high_attention_items: Vec<HighAttentionItem>,
    pub flagged_elements: Vec<ChangedElement>, // subset with non-empty security_tags
}

#[derive(Debug, Clone, Serialize)]
pub struct HighAttentionItem {
    pub reason: String,
    pub element_name: String,
    pub element_kind: ElementKind,
    pub change_type: ChangeType,
    pub file_path: String,
    pub tags: Vec<String>,
    pub code_preview: String,  // first ~5 lines of the relevant snippet
    pub snippet_ref: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SecurityTagDefinition {
    pub tag: String,
    pub patterns: Vec<String>,        // regex patterns
    pub description: String,
    pub severity: TagSeverity,
}

#[derive(Debug, Clone, Serialize)]
pub enum TagSeverity {
    High,      // crypto, auth, authz, secrets
    Medium,    // input validation, deserialization, network
    Low,       // logging, dependency changes, config
    Info,      // concurrency, error handling
}

// ── Cross-repo security overview ──

#[derive(Debug, Clone, Serialize)]
pub struct GlobalSecurityOverview {
    pub timestamp: DateTime<Utc>,
    pub total_repos_scanned: u32,
    pub repos_with_security_flags: u32,
    pub total_security_tagged_elements: u32,
    pub by_tag_global: HashMap<String, u32>,
    pub by_severity: HashMap<TagSeverity, u32>,
    pub high_attention_items: Vec<GlobalHighAttentionItem>,
    pub repos: Vec<RepoSecuritySummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlobalHighAttentionItem {
    pub repo: String,
    pub reason: String,
    pub element_name: String,
    pub file_path: String,
    pub tags: Vec<String>,
    pub before_code_preview: Option<String>,
    pub after_code_preview: Option<String>,
    pub commit_from: String,
    pub commit_to: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepoSecuritySummary {
    pub name: String,
    pub security_elements: u32,
    pub tags: Vec<String>,
    pub detail_path: String,
}
```

---

## 7. Custom Security Tags File Format

Users can provide `--security-tags-file` to override or extend built-in patterns:

```json
{
  "version": 1,
  "mode": "extend",
  "tags": [
    {
      "tag": "pii-handling",
      "description": "Changes touching personally identifiable information",
      "severity": "High",
      "patterns": [
        "(?i)social_security", "(?i)ssn", "(?i)date_of_birth",
        "(?i)passport", "(?i)email.*address", "(?i)phone.*number",
        "(?i)credit.*card", "(?i)gdpr", "(?i)pii"
      ]
    },
    {
      "tag": "payment",
      "description": "Payment processing logic",
      "severity": "High",
      "patterns": [
        "(?i)stripe", "(?i)payment", "(?i)charge", "(?i)invoice",
        "(?i)billing", "(?i)refund", "(?i)transaction"
      ]
    }
  ]
}
```

`mode`: `extend` (add to built-in) or `replace` (use only these).

---

## 8. Error Handling

| Scenario | Behavior |
|----------|----------|
| `git` not found on PATH | Exit with error code 1, clear message |
| Root dir doesn't exist | Exit with error code 1 |
| Output dir already exists | Append `-1`, `-2`, etc. suffix; or `--overwrite` flag |
| Single repo fails | Log error, record in report, continue with remaining repos |
| Permission denied on a subdirectory | Log warning, skip, continue |
| Network timeout on pull | Record as `PullFailed`, continue |
| Repo has <3 commits | Generate only the diffs that are possible, note the rest as skipped |
| Detached HEAD | Record branch as `DETACHED`, skip pull (or `--include-detached`) |
| Bare repository | Skip by default, `--include-bare` to include |
| Diff too large (>50MB patch) | Truncate element extraction, note in report, still write raw patch |
| Element extraction regex panic | Catch, log error, fallback to file-level-only report for that file |
| Binary files in diff | Skip element/snippet extraction, note as `(binary file)` |
| `git show` fails for file retrieval | Fall back to `DiffOnly` capture scope, log warning |
| Snippet exceeds `--max-snippet-lines` | Truncate with `CaptureScope::Truncated`, include note in output |
| Custom security tags file invalid | Exit with error code 1 and clear parse error message |

Exit codes:

| Code | Meaning |
|------|---------|
| 0 | All repos processed successfully |
| 1 | Fatal error (bad args, git not found, root not found) |
| 2 | Partial success (some repos failed, report still generated) |

---

## 9. Performance Considerations

- **Scanner**: `walkdir` with `max_depth` option, skip `.git` internals immediately.
- **Parallelism**: Default 4 threads, user-tunable. Avoid overwhelming git hosting providers with simultaneous pulls.
- **Large diffs**: Stream `git diff` output directly to file, then parse the file for extraction (don't buffer full diff in memory twice).
- **Snippet file retrieval**: `git show` calls are batched — one per unique file per commit, not one per element. If 5 elements are in `handler.rs`, the file is fetched once.
- **Regex compilation**: All language patterns and security patterns compiled once at startup via `OnceLock`.
- **Element cap**: `--max-elements` prevents runaway extraction on generated code or vendor directories.
- **Snippet cap**: `--max-snippet-lines` prevents enormous snippets from bloating the report.
- **Large repos**: The `--timeout` flag prevents hanging on massive pulls.
- **Disk usage**: For repos with massive diffs, the `snippets/` directory can grow large. Consider adding `--no-snippet-files` to skip writing individual files (keep them inline in JSON only).
- **Cross-diff caching**: When generating multiple diffs (N vs N-1, N-1 vs N-2), file contents from shared commits (e.g., N-1) are fetched once and reused across diffs via an in-memory LRU cache keyed by `(commit_hash, file_path)`.
- **Intra-repo parallelism**: For repos with many changed files, per-file element extraction and security tagging are parallelized using `rayon`'s nested thread pool, bounded by `--parallel`.
- **Incremental mode**: The tool writes a `.diffcatcher-state.json` in the report directory recording `(repo_path, last_seen_hash)`. On subsequent runs with `--incremental`, repos whose HEAD matches the last-seen hash are skipped entirely. This dramatically speeds up repeated scans.

---

## 10. Testing Strategy

| Layer | Approach |
|-------|----------|
| **Unit — extraction** | Feed known unified diff strings into parser, assert correct elements extracted. Cover all supported languages. |
| **Unit — snippets** | Feed known diff + known full file content, assert correct before/after extraction and element boundary detection. |
| **Unit — security tagging** | Feed known code snippets, assert correct tags applied. Include edge cases: false positives (`password_field` in a UI label vs actual password handling). |
| **Unit — boundary detection** | Test bracket counting and indentation tracking on various code styles (K&R, Allman, Python indentation, single-line functions). |
| **Unit — classification** | Assert file extension → language mapping correctness. |
| **Unit — reporting** | Assert JSON/TXT/MD output matches expected format for known inputs. |
| **Integration** | Create temp directories with `git init`, make commits with known code changes (including security-sensitive code), run tool, assert report structure, file contents, extracted elements, snippets, and security tags. |
| **E2E** | Shell script that clones real small repos, runs tool, validates output. |
| **Edge cases** | Repos with 0 commits, 1 commit, merge commits, detached HEAD, submodules, binary files, empty diffs, enormous diffs, non-UTF8 files, renamed files with changes. |
| **Extraction accuracy** | Golden-file tests: known `.patch` + known source files → expected `summary.json` with snippets. One per supported language. |
| **Security tag accuracy** | Golden-file tests: known code changes → expected security tags. Include false-positive and false-negative test cases. |

---

## 11. Future Enhancements (Out of Scope for v1)

- **Watch mode**: re-run on interval
- **Webhook/notification**: Slack/email on security-tagged changes detected
- **HTML report**: interactive diff viewer with syntax highlighting and security annotations
- **Tree-sitter integration**: replace regex-based element extraction with AST-based parsing for higher accuracy
- **Git submodule** tracking
- **Config file** (`.diffcatcher.toml`) per root directory
- **Ignore list**: skip specific repo paths
- **Commit range** diffs (beyond just N, N-1, N-2)
- **Branch comparison**: diff `main` vs `develop`
- **Remote-only check** (`git fetch --dry-run`) without actual pull
- **Dependency diff**: special handling for `Cargo.toml`, `package.json`, `go.mod` to show added/removed/upgraded deps as first-class elements with CVE cross-reference
- **AI summary**: optional LLM-generated natural language summary of changes with security assessment
- **SARIF output**: emit results in SARIF format for integration with GitHub Code Scanning, VS Code, etc.
- **Policy engine**: configurable rules that can cause the tool to exit with a non-zero code if certain security patterns are detected (e.g., "fail if any `unsafe` block is added")
- **Diff against known-good baseline**: compare current state against a recorded "approved" snapshot

---

## 12. Acceptance Criteria (v1 Definition of Done)

1. Given a directory containing 3+ nested git repos, the tool discovers all of them.
2. Repos that receive new commits on pull get `UPDATED` status with two valid `.patch` files and change manifests.
3. Repos already at latest get `UP_TO_DATE` status with a status file containing hash and message.
4. Dirty repos are skipped with a clear warning unless `--force-pull` is set.
5. The output directory matches the structure defined in §3.9 exactly.
6. **For every generated diff, a `summary_*.json` is produced that correctly lists all changed files and extracts changed code elements with accurate `kind`, `name`, `change_type`, and `snippet` fields.**
7. **Each extracted element includes a `snippet` object with `before`, `after`, and `diff_lines` fields. For `Modified` elements, both `before` and `after` contain the actual source code. For `Added` elements, `before` is null. For `Removed` elements, `after` is null.**
8. **When `CaptureScope::FullElement` is achieved, the snippet contains the complete function/struct/block body, not just the changed lines.**
9. **Individual snippet files are written to the `snippets/` directory with correct file extensions and naming.**
10. **Element extraction correctly identifies at minimum: functions, structs/classes, enums, traits/interfaces, imports, and constants for Rust, Python, JavaScript/TypeScript, and Go source files.**
11. **Every extracted element is scanned against security tag patterns. Elements touching crypto, authentication, authorization, secrets, or input validation code are tagged appropriately.**
12. **A `security_overview.*` file at the report root aggregates all security-tagged elements across all repos.**
13. **`high_attention_items` correctly flags: (a) security-tagged elements that are Removed, (b) new crypto/auth code, (c) changes to authorization logic.**
14. **The human-readable `summary_*.txt` and `summary_*.md` files include inline code snippets in the element listing, boxed/indented for readability.**
15. The top-level `summary.txt` includes per-repo element highlights and security tag counts.
16. `summary.json` is valid JSON parseable by `jq`.
17. The tool completes a scan of 50 repos in under 5 minutes with `--parallel 8` on a standard connection.
18. `--dry-run` makes zero modifications to any repository.
19. The tool exits with code 2 (not 1) if some repos fail but others succeed.
20. `--no-summary-extraction` produces reports without element summaries, snippets, or security tags.
21. `--no-snippets` produces element summaries but omits `before`/`after` code and skips the `snippets/` directory.
22. `--no-security-tags` produces element summaries with snippets but no security tagging or security overview files.
23. `--security-tags-file` with a valid custom file correctly applies custom patterns in `extend` or `replace` mode.
24. All CLI flags documented in `--help` match this spec.
