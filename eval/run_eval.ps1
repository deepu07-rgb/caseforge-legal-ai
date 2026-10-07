# CaseForge eval harness launcher (B-S0-05)
# Usage:
#   .\eval\run_eval.ps1
#   .\eval\run_eval.ps1 -Ac AC-16.1
#   .\eval\run_eval.ps1 -FailOnSkip

param(
    [string]$Ac = "",
    [string]$Json = "",
    [switch]$FailOnSkip
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Runner = Join-Path $PSScriptRoot "run_eval.py"

$python = $null
foreach ($name in @("python", "python3", "py")) {
    $cmd = Get-Command $name -ErrorAction SilentlyContinue
    if ($cmd) {
        $python = $cmd.Source
        break
    }
}

if (-not $python) {
    Write-Error "Python 3 is required to run the CaseForge eval harness (eval/run_eval.py). Install Python 3 and re-run."
    exit 2
}

$argsList = @($Runner)
if ($Ac) { $argsList += @("--ac", $Ac) }
if ($Json) { $argsList += @("--json", $Json) }
if ($FailOnSkip) { $argsList += "--fail-on-skip" }

& $python @argsList
exit $LASTEXITCODE
