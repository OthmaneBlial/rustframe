#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: ./scripts/ci_package_verify.sh <project-path> <host-format>" >&2
  exit 1
fi

project_path="$1"
host_format="$2"

cargo run -p rustframe-cli -- --project "$project_path" validate
cargo run -p rustframe-cli -- --project "$project_path" package --format "$host_format" --verify

test -s "$project_path/dist/packages/SHA256SUMS"
test -s "$project_path/dist/packages/rustframe-package-manifest.json"
test -s "$project_path/dist/packages/RELEASE_NOTES.md"

binary_name="$(basename "$project_path")"
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) binary_name="${binary_name}.exe" ;;
esac
binary_path="$project_path/target/rustframe/native/release/$binary_name"
smoke_dir="$project_path/target/rustframe/package-smoke"
mkdir -p "$smoke_dir"
RUSTFRAME_SMOKE_TEST=1 \
  RUSTFRAME_SMOKE_OUTPUT="$smoke_dir/runtime.json" \
  RUSTFRAME_SMOKE_DATA_DIR="$smoke_dir/data" \
  "$binary_path"
test -s "$smoke_dir/runtime.json"
