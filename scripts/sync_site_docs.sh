#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

mkdir -p "$repo_root/site/docs"
cp "$repo_root"/docs/*.md "$repo_root/site/docs/"

echo "Synced docs/ -> site/docs/"
