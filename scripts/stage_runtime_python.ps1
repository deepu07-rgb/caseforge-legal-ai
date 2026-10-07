<#
.SYNOPSIS
  Stage the portable Python runtime that ships inside the CaseForge installer (B-S4-00 packaging).

.DESCRIPTION
  The CaseForge sidecars (docpipe, infer_llm, infer_speech) are Python scripts. A clean Windows PC has no Python,
  so the installer bundles the official Python embeddable package plus the small pure-wheel dependencies of docpipe.

  Output layout (repo root):  runtime-python/   <- becomes <install>/_up_/runtime-python/ via tauri.conf.json
    python.exe, python312.zip, ... (official embeddable package, SHA-256 pinned below)
    Lib/site-packages/            (pypdfium2, Pillow from sidecars/docpipe/requirements.txt, binary wheels only)
    python312._pth                (sys.path: stdlib zip, site-packages, and the three sidecar folders)

  infer_speech's heavy dependencies (faster-whisper, pywin32) are intentionally NOT bundled: the speech sidecar reports
  itself unavailable and the app falls back to typed input with an honest notice.

.PARAMETER Dest     Output folder (default: <repo>/runtime-python)
.PARAMETER ZipPath  Use an already-downloaded embeddable zip instead of downloading (still hash-checked)
#>
param(
  [string]$Dest = (Join-Path (Split-Path -Parent $PSScriptRoot) 'runtime-python'),
  [string]$ZipPath = ''
)
$ErrorActionPreference = 'Stop'

$PythonVersion = '3.12.10'
$EmbedUrl      = "https://www.python.org/ftp/python/$PythonVersion/python-$PythonVersion-embed-amd64.zip"
# Pinned 2026-10-08 (11,133,606 bytes). A different download fails the build instead of being bundled.
$EmbedSha256   = '4acbed6dd1c744b0376e3b1cf57ce906f9dc9e95e68824584c8099a63025a3c3'
$Repo          = Split-Path -Parent $PSScriptRoot
$Requirements  = Join-Path $Repo 'sidecars/docpipe/requirements.txt'

if (-not $ZipPath) {
  $ZipPath = Join-Path ([System.IO.Path]::GetTempPath()) "python-$PythonVersion-embed-amd64.zip"
  Write-Host "Downloading $EmbedUrl"
  Invoke-WebRequest -Uri $EmbedUrl -OutFile $ZipPath -UseBasicParsing
}
$actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $ZipPath).Hash.ToLowerInvariant()
if ($actual -ne $EmbedSha256) { throw "Python embeddable package hash mismatch: got $actual, expected $EmbedSha256" }
Write-Host "Embeddable package hash OK ($actual)"

# Keep the placeholder README that makes the Tauri resource glob match on a fresh checkout.
$readme = Join-Path $Dest 'README.txt'
$readmeText = if (Test-Path $readme) { Get-Content -Raw -LiteralPath $readme } else { $null }
if (Test-Path $Dest) { Get-ChildItem -LiteralPath $Dest -Force | Remove-Item -Recurse -Force }
New-Item -ItemType Directory -Force -Path $Dest | Out-Null
if ($readmeText) { Set-Content -LiteralPath $readme -Value $readmeText -NoNewline -Encoding utf8 }

Expand-Archive -LiteralPath $ZipPath -DestinationPath $Dest -Force

$site = Join-Path $Dest 'Lib/site-packages'
New-Item -ItemType Directory -Force -Path $site | Out-Null

# Cross-install for the embedded interpreter's version regardless of which Python runs this script.
$pip = @('-m','pip','install','--disable-pip-version-check','--no-compile','--only-binary=:all:',
         '--platform','win_amd64','--python-version','3.12','--implementation','cp','--abi','cp312',
         '--target',$site,'-r',$Requirements)
& python @pip
if ($LASTEXITCODE -ne 0) { throw "pip install of sidecar requirements failed ($LASTEXITCODE)" }

# sys.path for the embedded interpreter (paths are relative to this file). The embeddable distribution does not add the
# script directory to sys.path, so the sidecar folders are listed explicitly; layout: <root>/{runtime-python,sidecars,contracts}.
$pth = @(
  'python312.zip',
  '.',
  'Lib\site-packages',
  '..\sidecars\docpipe',
  '..\sidecars\infer_llm',
  '..\sidecars\infer_speech'
) -join "`r`n"
Set-Content -LiteralPath (Join-Path $Dest 'python312._pth') -Value $pth -Encoding ascii

Get-ChildItem -LiteralPath $Dest -Recurse -Directory -Filter '__pycache__' | Remove-Item -Recurse -Force
$mb = [math]::Round(((Get-ChildItem -LiteralPath $Dest -Recurse -File | Measure-Object Length -Sum).Sum) / 1MB, 1)
Write-Host "Runtime staged at $Dest ($mb MB)"
