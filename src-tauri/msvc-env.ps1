# CaseForge MSVC + Perl env for Tauri / SQLCipher (vendored OpenSSL) builds.
# Usage (from repo root or src-tauri):
#   . .\src-tauri\msvc-env.ps1
#   cargo check
#   npm run tauri:dev

$ErrorActionPreference = "Stop"

# Discover every Visual Studio / Build Tools install (any edition or year) and pick the newest
# MSVC toolset that actually contains an x64 compiler; partial toolsets without cl.exe are skipped.
$vsRoots = @()
$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
if (Test-Path $vswhere) {
    $vsRoots += & $vswhere -all -products * -property installationPath
}
$vsRoots += Get-ChildItem "C:\Program Files\Microsoft Visual Studio", "C:\Program Files (x86)\Microsoft Visual Studio" -Directory -ErrorAction SilentlyContinue |
    ForEach-Object { Get-ChildItem $_.FullName -Directory -ErrorAction SilentlyContinue } | ForEach-Object { $_.FullName }
$toolsets = $vsRoots | Sort-Object -Unique | ForEach-Object {
    Get-ChildItem (Join-Path $_ "VC\Tools\MSVC") -Directory -ErrorAction SilentlyContinue
} | Where-Object { Test-Path (Join-Path $_.FullName "bin\Hostx64\x64\cl.exe") } |
    Sort-Object { [version]$_.Name } -Descending
if (-not $toolsets) {
    throw "No MSVC toolset with bin\Hostx64\x64\cl.exe found (install 'Desktop development with C++')"
}
$msvcTools = $toolsets[0].FullName
$msvcVer = $toolsets[0].Name
$msvcBin = Join-Path $msvcTools "bin\Hostx64\x64"

$sdkBase = "C:\Program Files (x86)\Windows Kits\10"
$sdkInclude = Join-Path $sdkBase "Include"
if (-not (Test-Path $sdkInclude)) {
    throw "Windows 10 SDK Include not found at $sdkInclude"
}
# Newest SDK that has both headers and x64 libraries.
$sdkVer = Get-ChildItem $sdkInclude -Directory | Where-Object {
    (Test-Path (Join-Path $_.FullName "um\windows.h")) -and (Test-Path (Join-Path $sdkBase "Lib\$($_.Name)\um\x64\kernel32.lib"))
} | Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1 -ExpandProperty Name
if (-not $sdkVer) {
    throw "No complete Windows SDK (headers + x64 libs) under $sdkBase"
}
if (-not (Test-Path "C:\Strawberry\perl\bin\perl.exe")) {
    throw "Strawberry Perl not found at C:\Strawberry (needed to build vendored OpenSSL): winget install StrawberryPerl.StrawberryPerl"
}

$kitBin = Join-Path $sdkBase "bin\$sdkVer\x64"
$perl = "C:\Strawberry\perl\bin"
$cargo = Join-Path $env:USERPROFILE ".cargo\bin"

# Prepend toolchain dirs; keep existing PATH so node/npm stay available.
$prepend = @(
    $perl
    (Join-Path $perl "..\site\bin")
    "C:\Strawberry\c\bin"
    $msvcBin
    $kitBin
    $cargo
) -join ";"
$env:PATH = "$prepend;$env:PATH"

$env:INCLUDE = @(
    (Join-Path $msvcTools "include")
    (Join-Path $sdkBase "Include\$sdkVer\ucrt")
    (Join-Path $sdkBase "Include\$sdkVer\um")
    (Join-Path $sdkBase "Include\$sdkVer\shared")
    (Join-Path $sdkBase "Include\$sdkVer\winrt")
) -join ";"

$env:LIB = @(
    (Join-Path $msvcTools "lib\x64")
    (Join-Path $sdkBase "Lib\$sdkVer\ucrt\x64")
    (Join-Path $sdkBase "Lib\$sdkVer\um\x64")
) -join ";"

$env:LIBPATH = Join-Path $msvcTools "lib\x64"

Write-Host "MSVC $msvcVer + WinSDK $sdkVer + Perl on PATH"
Write-Host "  cl:   $((Get-Command cl.exe).Source)"
Write-Host "  link: $((Get-Command link.exe).Source)"
Write-Host "  perl: $((Get-Command perl.exe).Source)"
