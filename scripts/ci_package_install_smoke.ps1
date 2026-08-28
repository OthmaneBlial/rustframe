param(
  [Parameter(Mandatory = $true)]
  [string]$ProjectPath,

  [Parameter(Mandatory = $true)]
  [ValidateSet('nsis', 'msi')]
  [string]$HostFormat,

  [Parameter(Mandatory = $true)]
  [string]$ProductName
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ($env:RUSTFRAME_ALLOW_INSTALL_SMOKE -ne '1') {
  throw 'refusing to install a package without RUSTFRAME_ALLOW_INSTALL_SMOKE=1'
}

$resolvedProject = (Resolve-Path $ProjectPath).Path
$packageDir = (Resolve-Path (Join-Path $resolvedProject 'dist/packages')).Path
$binaryName = "$(Split-Path $resolvedProject -Leaf).exe"
$temporaryRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [IO.Path]::GetTempPath() }
$smokeRoot = Join-Path $temporaryRoot "rustframe-$HostFormat-$([Guid]::NewGuid().ToString('N'))"
$smokeOutput = Join-Path $smokeRoot 'runtime.json'
$smokeDataDir = Join-Path $smokeRoot 'data'
$receiptOutput = Join-Path $packageDir "rustframe-offline-$HostFormat-receipt.json"
$verifier = Join-Path $PSScriptRoot 'assert_offline_smoke.mjs'
New-Item -ItemType Directory -Path $smokeRoot -Force | Out-Null

function Invoke-CheckedProcess {
  param(
    [Parameter(Mandatory = $true)]
    [string]$FilePath,

    [Parameter(Mandatory = $true)]
    [string]$ArgumentList,

    [Parameter(Mandatory = $true)]
    [string]$Description
  )

  $process = Start-Process -FilePath $FilePath -ArgumentList $ArgumentList -Wait -PassThru
  if ($process.ExitCode -ne 0) {
    throw "$Description failed with exit code $($process.ExitCode)"
  }
}

if ($HostFormat -eq 'nsis') {
  $artifact = Get-ChildItem -Path $packageDir -File -Filter '*-setup.exe' | Select-Object -First 1
  if (-not $artifact) { throw "no NSIS installer found in $packageDir" }

  $installRoot = Join-Path $env:LOCALAPPDATA $ProductName
  $binary = Join-Path $installRoot $binaryName
  $uninstaller = Join-Path $installRoot 'uninstall.exe'
  Invoke-CheckedProcess -FilePath $artifact.FullName -ArgumentList '/S' -Description 'NSIS install'
  if (-not (Test-Path $binary)) { throw "NSIS did not install $binary" }
  if (-not (Test-Path $uninstaller)) { throw "NSIS did not install $uninstaller" }
} else {
  $artifact = Get-ChildItem -Path $packageDir -File -Filter '*.msi' | Select-Object -First 1
  if (-not $artifact) { throw "no MSI installer found in $packageDir" }

  $installArguments = "/i `"$($artifact.FullName)`" /qn /norestart"
  Invoke-CheckedProcess -FilePath 'msiexec.exe' -ArgumentList $installArguments -Description 'MSI install'
  $uninstallRoots = @(
    'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*'
  )
  $entry = Get-ItemProperty $uninstallRoots -ErrorAction SilentlyContinue |
    Where-Object {
      $_.PSObject.Properties['DisplayName'] -and $_.DisplayName -eq $ProductName
    } |
    Select-Object -First 1
  if (-not $entry) { throw 'MSI uninstall registration was not found' }

  $installRoot = $entry.InstallLocation.Trim('"')
  $binary = Join-Path $installRoot $binaryName
  if (-not (Test-Path $binary)) { throw "MSI did not install $binary" }
}

$env:RUSTFRAME_SMOKE_TEST = '1'
$env:RUSTFRAME_SMOKE_OUTPUT = $smokeOutput
$env:RUSTFRAME_SMOKE_DATA_DIR = $smokeDataDir
$smokeArguments = '--rustframe-smoke-output="{0}" --rustframe-smoke-data-dir="{1}"' -f $smokeOutput, $smokeDataDir
Invoke-CheckedProcess -FilePath $binary -ArgumentList $smokeArguments -Description "$HostFormat application smoke"
if (-not (Test-Path $smokeOutput)) { throw "$HostFormat application did not write $smokeOutput" }

& node $verifier --input $smokeOutput --format $HostFormat --output $receiptOutput
if ($LASTEXITCODE -ne 0) { throw "$HostFormat offline receipt verifier failed with exit code $LASTEXITCODE" }
if (-not (Test-Path $receiptOutput)) { throw "$HostFormat offline receipt was not written" }

if ($HostFormat -eq 'nsis') {
  Invoke-CheckedProcess -FilePath $uninstaller -ArgumentList '/S' -Description 'NSIS uninstall'
} else {
  $uninstallArguments = "/x `"$($artifact.FullName)`" /qn /norestart"
  Invoke-CheckedProcess -FilePath 'msiexec.exe' -ArgumentList $uninstallArguments -Description 'MSI uninstall'
}

Start-Sleep -Seconds 2
if (Test-Path $binary) { throw "$HostFormat uninstall left $binary behind" }
if (-not (Test-Path $receiptOutput)) { throw "$HostFormat uninstall removed the transported smoke receipt" }
Remove-Item $smokeOutput -Force
Write-Host "Verified $HostFormat install, embedded launch, offline receipt, and uninstall: $receiptOutput"
