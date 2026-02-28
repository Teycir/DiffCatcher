#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${1:-}"
if [[ -z "${ROOT_DIR}" ]]; then
  ROOT_DIR="$(mktemp -d)"
  CREATED_ROOT=1
else
  mkdir -p "${ROOT_DIR}"
  CREATED_ROOT=0
fi

REPORT_DIR="${ROOT_DIR}/e2e-report"
REPO_DIR="${ROOT_DIR}/repos"
mkdir -p "${REPO_DIR}"

echo "[e2e] root: ${ROOT_DIR}"
echo "[e2e] cloning small public repositories..."
git clone --depth 20 https://github.com/octocat/Hello-World.git "${REPO_DIR}/hello-world"
git clone --depth 20 https://github.com/rust-lang/mdBook.git "${REPO_DIR}/mdbook"

echo "[e2e] running git-patrol..."
cargo run --quiet -- \
  "${REPO_DIR}" \
  -o "${REPORT_DIR}" \
  --no-pull \
  --history-depth 2 \
  --summary-format json,txt,md

echo "[e2e] validating report outputs..."
test -f "${REPORT_DIR}/summary.json"
test -f "${REPORT_DIR}/summary.txt"
test -f "${REPORT_DIR}/summary.md"
test -f "${REPORT_DIR}/security_overview.json"
test -f "${REPORT_DIR}/security_overview.txt"
test -f "${REPORT_DIR}/security_overview.md"

python3 - <<'PY' "${REPORT_DIR}/summary.json"
import json, pathlib, sys
p = pathlib.Path(sys.argv[1])
obj = json.loads(p.read_text(encoding="utf-8"))
assert "total_repos_found" in obj, "missing total_repos_found"
assert "repos" in obj and isinstance(obj["repos"], list), "missing repos[]"
print(f"[e2e] summary.json valid; repos={obj['total_repos_found']}")
PY

echo "[e2e] success. report: ${REPORT_DIR}"

if [[ "${CREATED_ROOT}" -eq 1 ]]; then
  echo "[e2e] temporary root retained at ${ROOT_DIR}"
fi
