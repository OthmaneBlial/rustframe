#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository_root"

if [[ "${1:-}" != "--skip-cargo" ]]; then
  cargo build -p rustframe-cli
fi

cli_binary="$repository_root/target/debug/rustframe"
if [[ ! -x "$cli_binary" ]]; then
  echo "RustFrame CLI is missing; run cargo build -p rustframe-cli" >&2
  exit 1
fi

# Validate the declarative catalog before executing any fixed verification profile.
node scripts/validate_template_registry.mjs

static_projects=(
  apps/daybreak-notes
  apps/prism-gallery
  apps/meridian-inventory
  apps/dispatch-room
  apps/hello-rustframe
  apps/quill-studio
)

for project_root in "${static_projects[@]}"; do
  (
    cd "$project_root"
    npm run build
  )
  "$cli_binary" --project "$project_root" validate
done

npm --prefix apps/research-desk run build
"$cli_binary" --project apps/research-desk validate

# The generator check proves site/showcase.json still comes only from accepted manifests.
node scripts/validate_template_registry.mjs
echo "Verified 7 template manifests with fixed v1 profiles."
