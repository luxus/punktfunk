#!/usr/bin/env bash
# Idempotent repository bootstrap for the Cloud Agent environment. Runs from the repo root after
# checkout. System libraries and toolchains live in the Dockerfile; this only warms
# repository-scoped dependencies so a fresh agent starts ready.
set -euo pipefail

echo "==> cargo fetch (workspace dependencies)"
cargo fetch --locked

# JS dependencies for the surfaces that use bun. Each guarded so a missing lockfile or directory
# never fails setup.
for dir in web docs-site sdk plugin-kit; do
  if [ -f "$dir/package.json" ]; then
    echo "==> bun install ($dir)"
    (cd "$dir" && bun install --frozen-lockfile 2>/dev/null || bun install)
  fi
done

echo "==> install complete"
