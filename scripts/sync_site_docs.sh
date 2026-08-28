#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

mkdir -p "$repo_root/site/docs"
cp "$repo_root"/docs/*.md "$repo_root/site/docs/"

mkdir -p "$repo_root/site/schemas/v1"
cp "$repo_root/schemas/v1/rustframe.schema.json" "$repo_root/site/schemas/v1/"

echo "Synced docs and public schema into site/"
