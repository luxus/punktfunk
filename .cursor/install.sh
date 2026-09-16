#!/usr/bin/env bash
# Idempotent repository bootstrap for the Cloud Agent environment. Runs from the repo root after
# checkout. System libraries and toolchains live in the Dockerfile; this only warms
# repository-scoped dependencies so a fresh agent starts ready.
set -euo pipefail

echo "==> cargo fetch (workspace dependencies)"
cargo fetch --locked

# JS dependencies for the surfaces that use bun. Frozen when bun.lock is present (a mismatch
# fails setup); unfrozen only when it is missing. Always --ignore-scripts.
for dir in web docs-site sdk plugin-kit; do
  if [ -f "$dir/package.json" ]; then
    echo "==> bun install ($dir)"
    if [ -f "$dir/bun.lock" ]; then
      (cd "$dir" && bun install --frozen-lockfile --ignore-scripts)
    else
      (cd "$dir" && bun install --ignore-scripts)
    fi
  fi
done

echo "==> install complete"
