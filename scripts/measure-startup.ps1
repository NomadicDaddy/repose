# Three starts to the first frame with repository rows, median reported; then the private
# working set after 60 seconds. Targets, not gates: about 300 ms and about 150 MB.
param(
    [Parameter(Mandatory)] [string] $Exe,
    [int] $IdleSeconds = 60
)
$ErrorActionPreference = 'Continue'
$starts = @()
$file = Join-Path ([System.IO.Path]::GetTempPath()) "repose-startup-$PID.txt"
for ($i = 1; $i -le 3; $i++) {
    if (Test-Path $file) { Remove-Item $file }
    $p = Start-Process -FilePath $Exe -ArgumentList @('--measure-startup', $file) -PassThru
    $p.WaitForExit(60000) | Out-Null
    if (-not $p.HasExited) { $p.Kill() }
    if (Test-Path $file) {
        $line = Get-Content $file | Select-Object -First 1
        if ($line) { $starts += [int]($line -split ' ')[1] }
    }
    Start-Sleep -Seconds 1
}
if (Test-Path $file) { Remove-Item $file }
if ($starts.Count -gt 0) {
    $median = ($starts | Sort-Object)[[int][math]::Floor(($starts.Count - 1) / 2)]
    Write-Host "MEASURE startup-ms $median (runs: $($starts -join ', '); target about 300)"
} else {
    Write-Host 'MEASURE startup-ms unavailable'
}
$p = Start-Process -FilePath $Exe -PassThru
Start-Sleep -Seconds $IdleSeconds
$counter = Get-Counter "\Process($($p.ProcessName))\Working Set - Private" -ErrorAction SilentlyContinue
$private = if ($counter) { [math]::Round($counter.CounterSamples[0].CookedValue / 1MB, 1) } else { 'unavailable' }
$ws = [math]::Round((Get-Process -Id $p.Id).WorkingSet64 / 1MB, 1)
Stop-Process -Id $p.Id -Force
Write-Host "MEASURE idle-private-working-set-mb $private (working set $ws; target about 150)"
