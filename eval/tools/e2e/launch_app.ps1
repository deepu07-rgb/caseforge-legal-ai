# Launches the desktop app in dev mode with WebView2's remote-debugging port open, for eval/tools/e2e/cdp.mjs.
#
# SAFETY: before running this, give the app a throwaway identifier so it cannot touch a real vault:
#   edit src-tauri/tauri.conf.json  "identifier": "app.caseforge.e2etest"   (do NOT commit that change)
# and delete %APPDATA%\app.caseforge.e2etest and %LOCALAPPDATA%\app.caseforge.e2etest afterwards.
#
# Usage (Windows PowerShell, from anywhere):  powershell -File eval\tools\e2e\launch_app.ps1 [-TargetDir C:\path\to\cargo-target]
param([string]$TargetDir = "$env:LOCALAPPDATA\Temp\ct")
$repo = (Resolve-Path "$PSScriptRoot\..\..\..").Path
Set-Location $repo
$conf = Get-Content "$repo\src-tauri\tauri.conf.json" -Raw
if ($conf -notmatch 'e2etest') { Write-Error "refusing to start: tauri.conf.json still has the real app identifier (see the header of this script)"; exit 2 }
. "$repo\src-tauri\msvc-env.ps1"
$ErrorActionPreference = "Continue"
$env:CARGO_TARGET_DIR = $TargetDir
$env:CARGO_INCREMENTAL = "0"          # smaller target directory
$env:CARGO_PROFILE_DEV_DEBUG = "0"    # smaller still
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9222"
$env:CASEFORGE_PYTHON = "python"
npm run tauri:dev 2>&1 | ForEach-Object { "$_" } | Out-File -FilePath "$env:TEMP\caseforge-e2e-app.log" -Encoding utf8
