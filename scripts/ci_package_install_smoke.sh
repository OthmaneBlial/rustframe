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
package_dir="$project_path/dist/packages"
binary_name="$(basename "$project_path")"
smoke_root="$(mktemp -d)"

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
  local output="$package_dir/rustframe-offline-${host_format}-receipt.json"
  node scripts/assert_offline_smoke.mjs \
    --input "$input" \
    --format "$host_format" \
    --output "$output"
  test -s "$output"
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
    artifact="$(find_artifact '*.exe')"
    artifact_windows="$(cygpath -w "$artifact")"
    smoke_windows="$(cygpath -w "$smoke_root")"
    smoke_output="$package_dir/.rustframe-nsis-smoke.json"
    smoke_output_windows="$(cygpath -w "$smoke_output")"
    RUSTFRAME_INSTALLER="$artifact_windows" \
    RUSTFRAME_PRODUCT_NAME="$product_name" \
    RUSTFRAME_BINARY_NAME="${binary_name}.exe" \
    RUSTFRAME_INSTALL_SMOKE_ROOT="$smoke_windows" \
    RUSTFRAME_INSTALL_SMOKE_OUTPUT="$smoke_output_windows" \
      powershell.exe -NoLogo -NoProfile -NonInteractive -Command - <<'POWERSHELL'
$ErrorActionPreference = 'Stop'
$installer = $env:RUSTFRAME_INSTALLER
$installRoot = Join-Path $env:LOCALAPPDATA $env:RUSTFRAME_PRODUCT_NAME
$binary = Join-Path $installRoot $env:RUSTFRAME_BINARY_NAME
$uninstaller = Join-Path $installRoot 'uninstall.exe'
$result = Start-Process -FilePath $installer -ArgumentList '/S' -Wait -PassThru
if ($result.ExitCode -ne 0) { throw "NSIS install failed with exit code $($result.ExitCode)" }
if (-not (Test-Path $binary)) { throw "NSIS did not install $binary" }
$env:RUSTFRAME_SMOKE_TEST = '1'
$env:RUSTFRAME_SMOKE_OUTPUT = $env:RUSTFRAME_INSTALL_SMOKE_OUTPUT
$env:RUSTFRAME_SMOKE_DATA_DIR = Join-Path $env:RUSTFRAME_INSTALL_SMOKE_ROOT 'nsis-data'
$app = Start-Process -FilePath $binary -Wait -PassThru
if ($app.ExitCode -ne 0 -or -not (Test-Path $env:RUSTFRAME_SMOKE_OUTPUT)) { throw 'installed NSIS application smoke failed' }
$result = Start-Process -FilePath $uninstaller -ArgumentList '/S' -Wait -PassThru
if ($result.ExitCode -ne 0) { throw "NSIS uninstall failed with exit code $($result.ExitCode)" }
Start-Sleep -Seconds 2
if (Test-Path $binary) { throw "NSIS uninstall left $binary behind" }
POWERSHELL
    ;;

  msi)
    artifact="$(find_artifact '*.msi')"
    artifact_windows="$(cygpath -w "$artifact")"
    smoke_windows="$(cygpath -w "$smoke_root")"
    smoke_output="$package_dir/.rustframe-msi-smoke.json"
    smoke_output_windows="$(cygpath -w "$smoke_output")"
    RUSTFRAME_INSTALLER="$artifact_windows" \
    RUSTFRAME_PRODUCT_NAME="$product_name" \
    RUSTFRAME_BINARY_NAME="${binary_name}.exe" \
    RUSTFRAME_INSTALL_SMOKE_ROOT="$smoke_windows" \
    RUSTFRAME_INSTALL_SMOKE_OUTPUT="$smoke_output_windows" \
      powershell.exe -NoLogo -NoProfile -NonInteractive -Command - <<'POWERSHELL'
$ErrorActionPreference = 'Stop'
$installer = $env:RUSTFRAME_INSTALLER
$arguments = @('/i', $installer, '/qn', '/norestart')
$result = Start-Process -FilePath 'msiexec.exe' -ArgumentList $arguments -Wait -PassThru
if ($result.ExitCode -ne 0) { throw "MSI install failed with exit code $($result.ExitCode)" }
$uninstallRoots = @(
  'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*',
  'HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*'
)
$entry = Get-ItemProperty $uninstallRoots -ErrorAction SilentlyContinue |
  Where-Object { $_.DisplayName -eq $env:RUSTFRAME_PRODUCT_NAME } |
  Select-Object -First 1
if (-not $entry) { throw "MSI uninstall registration was not found" }
$installRoot = $entry.InstallLocation.Trim('"')
$binary = Join-Path $installRoot $env:RUSTFRAME_BINARY_NAME
if (-not (Test-Path $binary)) { throw "MSI did not install $binary" }
$env:RUSTFRAME_SMOKE_TEST = '1'
$env:RUSTFRAME_SMOKE_OUTPUT = $env:RUSTFRAME_INSTALL_SMOKE_OUTPUT
$env:RUSTFRAME_SMOKE_DATA_DIR = Join-Path $env:RUSTFRAME_INSTALL_SMOKE_ROOT 'msi-data'
$app = Start-Process -FilePath $binary -Wait -PassThru
if ($app.ExitCode -ne 0 -or -not (Test-Path $env:RUSTFRAME_SMOKE_OUTPUT)) { throw 'installed MSI application smoke failed' }
$arguments = @('/x', $installer, '/qn', '/norestart')
$result = Start-Process -FilePath 'msiexec.exe' -ArgumentList $arguments -Wait -PassThru
if ($result.ExitCode -ne 0) { throw "MSI uninstall failed with exit code $($result.ExitCode)" }
if (Test-Path $binary) { throw "MSI uninstall left $binary behind" }
POWERSHELL
    ;;

  *)
    echo "unsupported host package format '$host_format'" >&2
    exit 1
    ;;
esac

write_offline_receipt "$smoke_output"
if [[ "$host_format" == "nsis" || "$host_format" == "msi" ]]; then
  rm -f "$smoke_output"
fi
