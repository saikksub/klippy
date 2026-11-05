# Builds a release Klippy.exe into windows\dist\ that you can keep anywhere
# and run; it needs nothing else installed (WebView2 ships with Windows 10
# and 11). Run it again to update.
#
# Needs Rust (MSVC toolchain, from https://rustup.rs, with the Visual Studio
# C++ Build Tools).
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

$missing = $false
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host 'missing: cargo (install Rust from https://rustup.rs)'
    $missing = $true
}
if ($missing) { exit 1 }

cargo build -p klippy-windows --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

New-Item -ItemType Directory -Force windows\dist | Out-Null
Copy-Item target\release\Klippy.exe windows\dist\Klippy.exe -Force

Write-Host ''
Write-Host "Done: $(Resolve-Path windows\dist\Klippy.exe)"
Write-Host 'Run it from a folder you keep it in: launch at login starts it from there.'
