# Refuses an executable that names a Visual C++ runtime DLL, the ones a clean Windows install
# lacks: vcruntime*.dll and msvcp*.dll, and the api-ms-win-crt-*.dll set that only stands in for
# them when they are not linked in. repose links the C runtime statically (.cargo/config.toml,
# Phillip #43404), so a build that names any of these would fail to start on a clean host, as every
# release to 1.4.0 did (winlab, Carl #43391). The DLL names of a PE's import table are plain ASCII,
# so a byte search finds them; the 1.4.0 exe is the known positive (Morticia on dc1d662).
# Usage: pwsh scripts/check-no-vc-runtime.ps1 <path-to-exe>. Exits 1 and names each DLL found.
param([Parameter(Mandatory)] [string] $Exe)
$ErrorActionPreference = 'Stop'
if (-not (Test-Path -LiteralPath $Exe)) { Write-Host "FAILED: missing $Exe"; exit 1 }
$text = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes((Resolve-Path -LiteralPath $Exe)))
$pattern = '(?i)\b(vcruntime\d+[a-z0-9_]*\.dll|msvcp\d+[a-z0-9_]*\.dll|api-ms-win-crt-[a-z0-9-]+\.dll)'
$found = @([regex]::Matches($text, $pattern) | ForEach-Object { $_.Value.ToLowerInvariant() } | Sort-Object -Unique)
if ($found.Count -gt 0) {
    Write-Host "FAILED: $(Split-Path -Leaf $Exe) needs the Visual C++ runtime: $($found -join ', ')"
    exit 1
}
Write-Host "NO-VC-RUNTIME $(Split-Path -Leaf $Exe) names no vcruntime, msvcp or api-ms-win-crt DLL"
exit 0
