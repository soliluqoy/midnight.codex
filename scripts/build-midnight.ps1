param(
    [switch]$Release,
    [string]$OutputDirectory = (Join-Path (Split-Path $PSScriptRoot -Parent) 'midnight-dist')
)

$ErrorActionPreference = 'Stop'
$sourceRoot = Split-Path $PSScriptRoot -Parent
$outputRoot = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
$binaries = @(
    'codex', 'codex-code-mode-host', 'codex-responses-api-proxy',
    'codex-windows-sandbox-setup', 'codex-windows-sandbox-service',
    'codex-command-runner', 'codex-app-server'
)
$packages = @(
    'codex-cli', 'codex-code-mode-host', 'codex-responses-api-proxy',
    'codex-windows-sandbox', 'codex-windows-sandbox-service', 'codex-app-server'
)
$cargoArgs = @('build', '--locked')
foreach ($package in $packages) { $cargoArgs += @('-p', $package) }
foreach ($binary in $binaries) { $cargoArgs += @('--bin', $binary) }
$profile = 'debug'
if ($Release) { $cargoArgs += '--release'; $profile = 'release' }
if (-not $env:CARGO_PROFILE_DEV_DEBUG) { $env:CARGO_PROFILE_DEV_DEBUG = '0' }

Push-Location (Join-Path $sourceRoot 'codex-rs')
try {
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw "Cargo build failed with exit code $LASTEXITCODE" }
    $targetRoot = if ($env:CARGO_TARGET_DIR) {
        $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($env:CARGO_TARGET_DIR)
    } else { Join-Path $PWD 'target' }
    New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null
    $checksums = foreach ($binary in $binaries) {
        $name = if ($binary -eq 'codex') { 'midnight-codex.exe' } else { "$binary.exe" }
        $destination = Join-Path $outputRoot $name
        Copy-Item -LiteralPath (Join-Path $targetRoot "$profile/$binary.exe") -Destination $destination
        $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $destination).Hash.ToLowerInvariant()
        "$hash  $name"
    }
    $checksums | Set-Content -Encoding ascii -LiteralPath (Join-Path $outputRoot 'SHA256SUMS.txt')
    foreach ($file in @('LICENSE', 'NOTICE', 'MIDNIGHT.md')) {
        Copy-Item -LiteralPath (Join-Path $sourceRoot $file) -Destination (Join-Path $outputRoot $file)
    }
    & (Join-Path $outputRoot 'midnight-codex.exe') --version
    if ($LASTEXITCODE -ne 0) { throw 'Built CLI version check failed' }
    Write-Output "Midnight Windows bundle: $outputRoot"
} finally {
    Pop-Location
}
