#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: ./scripts/ci_package_install_smoke.sh <project-path> <host-format> <product-name>" >&2
  exit 1
fi

if [[ "${RUSTFRAME_ALLOW_INSTALL_SMOKE:-}" != "1" ]]; then
  echo "refusing to install a package without RUSTFRAME_ALLOW_INSTALL_SMOKE=1" >&2
  exit 1
fi

project_path="$1"
host_format="$2"
product_name="$3"
binary_name="$(basename "$project_path")"
package_dir="$(cd "$project_path/dist/packages" && pwd)"
smoke_root="$(mktemp -d)"
receipt_output="$package_dir/rustframe-offline-${host_format}-receipt.json"

find_artifact() {
  local pattern="$1"
  local artifact
  artifact="$(find "$package_dir" -maxdepth 1 -type f -name "$pattern" -print -quit)"
  if [[ -z "$artifact" ]]; then
    echo "no package artifact matching '$pattern' in $package_dir" >&2
    exit 1
  fi
  printf '%s\n' "$artifact"
}

run_smoke_binary() {
  local binary="$1"
  local output="$2"
  local data_dir="$3"
  RUSTFRAME_SMOKE_TEST=1 \
    RUSTFRAME_SMOKE_OUTPUT="$output" \
    RUSTFRAME_SMOKE_DATA_DIR="$data_dir" \
    "$binary"
  test -s "$output"
}

write_offline_receipt() {
  local input="$1"
  node scripts/assert_offline_smoke.mjs \
    --input "$input" \
    --format "$host_format" \
    --output "$receipt_output"
  test -s "$receipt_output"
}

case "$host_format" in
  appimage)
    artifact="$(find_artifact '*.AppImage')"
    chmod +x "$artifact"
    APPIMAGE_EXTRACT_AND_RUN=1 run_smoke_binary \
      "$artifact" "$smoke_root/appimage.json" "$smoke_root/appimage-data"
    smoke_output="$smoke_root/appimage.json"
    ;;

  deb)
    artifact="$(find_artifact '*.deb')"
    package_name="$(dpkg-deb --field "$artifact" Package)"
    cleanup_deb() {
      sudo dpkg --remove "$package_name" >/dev/null 2>&1 || true
    }
    trap cleanup_deb EXIT
    sudo dpkg --install "$artifact"
    installed_binary="$(dpkg --listfiles "$package_name" | grep -E "/(usr/)?bin/${binary_name}$" | head -n 1)"
    test -n "$installed_binary"
    run_smoke_binary \
      "$installed_binary" "$smoke_root/deb.json" "$smoke_root/deb-data"
    smoke_output="$smoke_root/deb.json"
    sudo dpkg --remove "$package_name"
    trap - EXIT
    test ! -e "$installed_binary"
    ;;

  app)
    app_bundle="$(find "$package_dir" -maxdepth 1 -type d -name '*.app' -print -quit)"
    test -n "$app_bundle"
    install_root="$smoke_root/Applications"
    mkdir -p "$install_root"
    cp -R "$app_bundle" "$install_root/"
    installed_app="$install_root/$(basename "$app_bundle")"
    installed_binary="$(find "$installed_app/Contents/MacOS" -maxdepth 1 -type f -perm -111 -print -quit)"
    test -n "$installed_binary"
    run_smoke_binary \
      "$installed_binary" "$smoke_root/app.json" "$smoke_root/app-data"
    smoke_output="$smoke_root/app.json"
    rm -rf "$installed_app"
    test ! -e "$installed_app"
    ;;

  dmg)
    artifact="$(find_artifact '*.dmg')"
    mount_root="$smoke_root/mount"
    install_root="$smoke_root/Applications"
    mkdir -p "$mount_root" "$install_root"
    cleanup_dmg() {
      hdiutil detach "$mount_root" -quiet >/dev/null 2>&1 || true
    }
    trap cleanup_dmg EXIT
    hdiutil attach "$artifact" -nobrowse -readonly -mountpoint "$mount_root" -quiet
    app_bundle="$(find "$mount_root" -maxdepth 1 -type d -name '*.app' -print -quit)"
    test -n "$app_bundle"
    cp -R "$app_bundle" "$install_root/"
    installed_app="$install_root/$(basename "$app_bundle")"
    installed_binary="$(find "$installed_app/Contents/MacOS" -maxdepth 1 -type f -perm -111 -print -quit)"
    test -n "$installed_binary"
    run_smoke_binary \
      "$installed_binary" "$smoke_root/dmg.json" "$smoke_root/dmg-data"
    smoke_output="$smoke_root/dmg.json"
    rm -rf "$installed_app"
    hdiutil detach "$mount_root" -quiet
    trap - EXIT
    test ! -e "$installed_app"
    ;;

  nsis)
    powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass \
      -File scripts/ci_package_install_smoke.ps1 \
      -ProjectPath "$project_path" \
      -HostFormat "$host_format" \
      -ProductName "$product_name"
    smoke_verified=1
    ;;

  msi)
    powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass \
      -File scripts/ci_package_install_smoke.ps1 \
      -ProjectPath "$project_path" \
      -HostFormat "$host_format" \
      -ProductName "$product_name"
    smoke_verified=1
    ;;

  *)
    echo "unsupported host package format '$host_format'" >&2
    exit 1
    ;;
esac

if [[ "${smoke_verified:-0}" != "1" ]]; then
  write_offline_receipt "$smoke_output"
fi
