#!/usr/bin/env bash
set -euo pipefail

version="1.7.12"
archive="actionlint_${version}_linux_amd64.tar.gz"
expected_sha256="8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8"
tool_root="$(mktemp -d)"
download="$tool_root/$archive"

curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location \
  "https://github.com/rhysd/actionlint/releases/download/v${version}/${archive}" \
  --output "$download"
printf '%s  %s\n' "$expected_sha256" "$download" | sha256sum --check --status
tar -xzf "$download" -C "$tool_root" actionlint
"$tool_root/actionlint" -color
