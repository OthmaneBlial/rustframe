#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: ./scripts/ci_package_verify.sh <app-name>" >&2
  exit 1
fi

app_name="$1"

cargo test -p rustframe-cli
cargo run -p rustframe-cli -- doctor
cargo run -p rustframe-cli -- package "$app_name" --verify
