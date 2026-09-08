#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

mkdir -p "$repo_root/site/docs"
while IFS= read -r -d '' source_file; do
    relative_file="${source_file#"$repo_root/"}"
    mkdir -p "$(dirname "$repo_root/site/$relative_file")"
    cp "$source_file" "$repo_root/site/$relative_file"
done < <(find "$repo_root/docs" -type f -name '*.md' -print0)

mkdir -p "$repo_root/site/schemas/v1"
cp "$repo_root/schemas/v1/rustframe.schema.json" "$repo_root/site/schemas/v1/"

mkdir -p "$repo_root/site/schemas/file-associations/v1"
cp "$repo_root/schemas/file-associations/v1/file-associations.schema.json" \
  "$repo_root/site/schemas/file-associations/v1/"

echo "Synced docs and public schemas into site/"
