<#
.SYNOPSIS
Builds UnderBoss and refreshes the folder that gets zipped and sent to friends.

.DESCRIPTION
Builds the release binary, then copies it as UnderBoss.exe, together with
dist\README.txt, into the shared folder (UnderBoss\ in the repo root by
default, which git ignores).
Run it after every change so the shared folder always holds the latest build.

.EXAMPLE
powershell -ExecutionPolicy Bypass -File scripts\package.ps1
Builds and refreshes UnderBoss\.

.EXAMPLE
powershell -ExecutionPolicy Bypass -File scripts\package.ps1 -Zip
Also writes UnderBoss-v<version>.zip in the repo root, ready to send.
#>
param(
    [string]$Destination,
    [switch]$Zip,
    [switch]$NoBuild
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
if (-not $Destination) { $Destination = Join-Path $repo 'UnderBoss' }
$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $repo 'target' }
$built = Join-Path $target 'release\cs2-dumper.exe'

$version = (Select-String -Path (Join-Path $repo 'overlay\Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value

if (-not $NoBuild) {
    Write-Host "Building UnderBoss v$version..."
    cargo build --release -p cs2-dumper --manifest-path (Join-Path $repo 'Cargo.toml')
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
}
if (-not (Test-Path $built)) { throw "No build found at $built. Run without -NoBuild." }

# A running exe is locked, so the copy below would fail halfway.
$running = Get-Process -Name UnderBoss -ErrorAction SilentlyContinue
if ($running) { throw 'UnderBoss is running. Press END in game (or close its console), then run this again.' }

New-Item -ItemType Directory -Force -Path $Destination | Out-Null
Copy-Item $built (Join-Path $Destination 'UnderBoss.exe') -Force
Copy-Item (Join-Path $repo 'dist\README.txt') (Join-Path $Destination 'README.txt') -Force

# The folder should hold only what friends need; point out anything else that crept in.
$extra = Get-ChildItem $Destination | Where-Object { $_.Name -notin 'UnderBoss.exe', 'README.txt' }
if ($extra) { Write-Warning ("Not part of the release, remove before zipping: " + ($extra.Name -join ', ')) }

$exe = Get-Item (Join-Path $Destination 'UnderBoss.exe')
Write-Host ("Refreshed {0}: UnderBoss.exe v{1}, {2:N0} KB, built {3}" -f $Destination, $version, ($exe.Length / 1KB), $exe.LastWriteTime)

if ($Zip) {
    $zipPath = Join-Path (Split-Path $Destination -Parent) "UnderBoss-v$version.zip"
    Compress-Archive -Path $Destination -DestinationPath $zipPath -Force
    Write-Host "Zipped to $zipPath"
}
