#!/usr/bin/env bash
set -euo pipefail

version="1.7.12"
case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)
    platform="darwin_arm64"
    expected_sha256="aba9ced2dee8d27fecca3dc7feb1a7f9a52caefa1eb46f3271ea66b6e0e6953f"
    ;;
  Darwin-x86_64)
    platform="darwin_amd64"
    expected_sha256="5b44c3bc2255115c9b69e30efc0fecdf498fdb63c5d58e17084fd5f16324c644"
    ;;
  Linux-aarch64)
    platform="linux_arm64"
    expected_sha256="325e971b6ba9bfa504672e29be93c24981eeb1c07576d730e9f7c8805afff0c6"
    ;;
  Linux-x86_64)
    platform="linux_amd64"
    expected_sha256="8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8"
    ;;
  *)
    printf 'Unsupported actionlint host: %s-%s\n' "$(uname -s)" "$(uname -m)" >&2
    exit 1
    ;;
esac

archive="actionlint_${version}_${platform}.tar.gz"
tool_root="$(mktemp -d)"
download="$tool_root/$archive"

curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location \
  "https://github.com/rhysd/actionlint/releases/download/v${version}/${archive}" \
  --output "$download"
actual_sha256="$(shasum -a 256 "$download" | awk '{print $1}')"
if [[ "$actual_sha256" != "$expected_sha256" ]]; then
  printf 'actionlint checksum mismatch: expected %s, got %s\n' "$expected_sha256" "$actual_sha256" >&2
  exit 1
fi
tar -xzf "$download" -C "$tool_root" actionlint
"$tool_root/actionlint" -color
