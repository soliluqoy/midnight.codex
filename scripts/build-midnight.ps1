param(
    [switch]$Release,
    [string]$OutputDirectory = (Join-Path (Split-Path $PSScriptRoot -Parent) 'midnight-dist')
)

$ErrorActionPreference = 'Stop'
$sourceRoot = Split-Path $PSScriptRoot -Parent
$outputRoot = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
$codeModeUrl = 'https://github.com/openai/codex/releases/download/rust-v0.162.1/codex-code-mode-host-x86_64-pc-windows-msvc.exe'
$codeModeSha256 = '9f865a748c984492ca9b69aac75680e4954f31311c43ec33ddbdae0ed77bd4bd'
$binaries = @(
    'codex', 'codex-code-mode-host', 'codex-responses-api-proxy',
    'codex-windows-sandbox-setup', 'codex-windows-sandbox-service',
    'codex-command-runner', 'codex-app-server'
)
$packages = @(
    'codex-cli', 'codex-responses-api-proxy',
    'codex-windows-sandbox', 'codex-windows-sandbox-service', 'codex-app-server'
)
$cargoArgs = @('build', '--locked')
foreach ($package in $packages) { $cargoArgs += @('-p', $package) }
foreach ($binary in $binaries) {
    if ($binary -ne 'codex-code-mode-host') { $cargoArgs += @('--bin', $binary) }
}
$profile = 'debug'
if ($Release) { $cargoArgs += '--release'; $profile = 'release' }
if (-not $env:CARGO_PROFILE_DEV_DEBUG) { $env:CARGO_PROFILE_DEV_DEBUG = '0' }

Push-Location (Join-Path $sourceRoot 'codex-rs')
try {
    $targetRoot = if ($env:CARGO_TARGET_DIR) {
        $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($env:CARGO_TARGET_DIR)
    } else { Join-Path $PWD 'target' }
    $runtimeCache = Join-Path $targetRoot 'midnight-upstream'
    New-Item -ItemType Directory -Path $runtimeCache -Force | Out-Null
    $codeModeHost = Join-Path $runtimeCache 'codex-code-mode-host.exe'
    if (-not (Test-Path -LiteralPath $codeModeHost) -or
        (Get-FileHash -Algorithm SHA256 -LiteralPath $codeModeHost).Hash.ToLowerInvariant() -ne $codeModeSha256) {
        # Rusty V8 150.4.0 has no prebuilt sandboxed Windows archive. Reuse the
        # unchanged upstream host, with its sandbox intact and an exact digest.
        $previousProgress = $ProgressPreference
        try {
            $ProgressPreference = 'SilentlyContinue'
            Invoke-WebRequest -UseBasicParsing -Uri $codeModeUrl -OutFile $codeModeHost -TimeoutSec 180
        } finally { $ProgressPreference = $previousProgress }
    }
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath $codeModeHost).Hash.ToLowerInvariant() -ne $codeModeSha256) {
        throw 'Upstream code-mode host checksum mismatch'
    }
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw "Cargo build failed with exit code $LASTEXITCODE" }
    New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null
    $checksums = foreach ($binary in $binaries) {
        $name = if ($binary -eq 'codex') { 'midnight-codex.exe' } else { "$binary.exe" }
        $destination = Join-Path $outputRoot $name
        $source = if ($binary -eq 'codex-code-mode-host') { $codeModeHost } else { Join-Path $targetRoot "$profile/$binary.exe" }
        Copy-Item -LiteralPath $source -Destination $destination
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
