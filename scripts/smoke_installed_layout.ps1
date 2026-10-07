<#
.SYNOPSIS
  Prove a CaseForge resource tree works with the BUNDLED Python only (no system Python involved).

.DESCRIPTION
  -Root must contain runtime-python/, sidecars/ and contracts/ side by side:
    * repo root after scripts/stage_runtime_python.ps1          (local/dev check)
    * <install dir>/_up_ after silent installation of the NSIS installer   (CI check of the real installed layout)
  Every check calls the real bundled files; any failure exits non-zero.
#>
param([Parameter(Mandatory = $true)][string]$Root)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path -LiteralPath $Root).Path
$py   = Join-Path $Root 'runtime-python/python.exe'
$failures = New-Object System.Collections.Generic.List[string]

function Check([string]$name, [scriptblock]$body) {
  try {
    $detail = & $body
    Write-Host ("PASS  {0}  {1}" -f $name, $detail)
  } catch {
    $failures.Add($name) | Out-Null
    Write-Host ("FAIL  {0}  {1}" -f $name, $_.Exception.Message)
  }
}
function Rpc([string]$script, [string]$method) {
  # The BUNDLED interpreter drives the sidecar through scripts/sidecar_rpc.py (stdlib only), exactly like the Rust host
  # (one JSON-RPC line on stdin). PowerShell's own process plumbing is avoided: Windows PowerShell 5.1 strips quotes from
  # native arguments and adds a BOM to piped text.
  $env:PYTHONPATH = $null; $env:PYTHONHOME = $null   # a developer environment must not mask a packaging problem
  $driver = Join-Path $PSScriptRoot 'sidecar_rpc.py'
  $out = & $py $driver (Join-Path $Root $script) $method 2>&1 | Out-String
  if ($LASTEXITCODE -ne 0) { throw $out.Trim() }
  return ($out.Trim() | ConvertFrom-Json)
}

Check 'bundled python exists'        { if (-not (Test-Path $py)) { throw "missing $py" }; $py.Substring($Root.Length) }
Check 'python is 3.12 and isolated'  { $v = & $py -c "import sys;print(sys.version.split()[0], sys.flags.isolated)"; if ($v -notmatch '^3\.12\.') { throw $v }; $v }
Check 'stdlib + wheels import'       { & $py -c "import json,hashlib,ctypes,zipfile,urllib.request,pypdfium2,PIL;print('pypdfium2',pypdfium2.version.PYPDFIUM_INFO.version if hasattr(pypdfium2,'version') else '?','Pillow',PIL.__version__)" }
Check 'contracts present'            { foreach ($f in 'contracts/constants.json','contracts/ocr_config.json','contracts/supported_models.json','contracts/prompts/legal_reasoner_v1.txt') { if (-not (Test-Path (Join-Path $Root $f))) { throw "missing $f" } }; '4 files' }
Check 'docpipe.ping'                 { $r = Rpc 'sidecars/docpipe/docpipe.py' 'docpipe.ping'; ($r | ConvertTo-Json -Compress) }
Check 'infer_llm.ping'               { $r = Rpc 'sidecars/infer_llm/infer_llm.py' 'llm.ping'; ($r | ConvertTo-Json -Compress) }
Check 'infer_llm.tiers'              { $r = Rpc 'sidecars/infer_llm/infer_llm.py' 'llm.tiers'; if ($r.auto_download -ne $false) { throw 'auto_download must be false' }; ($r.tiers | ForEach-Object { $_.tier + '=' + $_.model }) -join ', ' }
Check 'tesseract runs'               { $t = Join-Path $Root 'sidecars/docpipe/runtime/tesseract/tesseract.exe'; $v = (& $t --version 2>&1 | Select-Object -First 1); if ($LASTEXITCODE -ne 0) { throw "exit $LASTEXITCODE" }; "$v" }
Check 'tesseract English data'       { if (-not (Test-Path (Join-Path $Root 'sidecars/docpipe/runtime/tesseract/tessdata/eng.traineddata'))) { throw 'missing eng.traineddata' }; 'eng.traineddata' }
Check 'official India pack bundled'  { if (-not (Test-Path (Join-Path $Root 'packs/official/caseforge.india.partition.highcourt.civil-appeal/manifest.json'))) { throw 'missing pack' }; 'ok' }

if ($failures.Count -gt 0) { Write-Host ("`n{0} check(s) failed: {1}" -f $failures.Count, ($failures -join ', ')); exit 1 }
Write-Host "`nAll layout checks passed."
