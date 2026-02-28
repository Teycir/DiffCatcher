#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${1:-$(mktemp -d)}"
REPORT_DIR="${ROOT_DIR}/perf-report"
REPOS_DIR="${ROOT_DIR}/repos"
mkdir -p "${REPOS_DIR}"

echo "[perf] creating 50 local repositories under ${REPOS_DIR}..."
for i in $(seq 1 50); do
  repo="${REPOS_DIR}/repo-${i}"
  mkdir -p "${repo}"
  git -C "${repo}" init >/dev/null
  git -C "${repo}" config user.name "Perf Test"
  git -C "${repo}" config user.email "perf@example.com"

  printf 'fn v() -> i32 { 1 }\n' > "${repo}/main.rs"
  git -C "${repo}" add .
  git -C "${repo}" commit -m "c1" >/dev/null

  printf 'fn v() -> i32 { 2 }\n' > "${repo}/main.rs"
  git -C "${repo}" add .
  git -C "${repo}" commit -m "c2" >/dev/null

  printf 'fn v() -> i32 { 3 }\n' > "${repo}/main.rs"
  git -C "${repo}" add .
  git -C "${repo}" commit -m "c3" >/dev/null
done

echo "[perf] running git-patrol with --parallel 8..."
start="$(date +%s)"
./target/debug/git-patrol \
  "${REPOS_DIR}" \
  -o "${REPORT_DIR}" \
  --no-pull \
  --history-depth 2 \
  --parallel 8 \
  --summary-format json
end="$(date +%s)"
elapsed="$((end - start))"

echo "[perf] elapsed=${elapsed}s"
if [[ "${elapsed}" -gt 300 ]]; then
  echo "[perf] FAIL: expected < 300s"
  exit 1
fi

echo "[perf] PASS: completed in under 5 minutes"
