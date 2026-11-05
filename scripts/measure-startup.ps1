# Three starts to the first frame with repository rows, median reported; then the private
# working set after 60 seconds with a diff open (A9). Targets, not gates: about 300 ms and about
# 150 MB. The idle run uses a disposable fixture repository with a known modified file and its own
# config under the temp folder, opened with --config, --select and --file, and samples only after
# --measure-diff reports the diff loaded. It removes only that fixture and never touches a user
# checkout.
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
# The idle fixture: one repository with a committed 400-line file, then edited in three places so
# its working-tree diff has three hunks. Its own empty hooks folder keeps user hooks out.
# The folder name is new for every run, and a folder already there is refused rather than
# reused, so neither an old marker nor files this run did not create can be read or removed.
$fixture = Join-Path ([System.IO.Path]::GetTempPath()) "repose-idle-$([guid]::NewGuid().ToString('N'))"
if (Test-Path -LiteralPath $fixture) {
    Write-Host "MEASURE idle-private-working-set-mb unavailable (fixture folder $fixture already exists)"
    exit 0
}
[System.IO.Directory]::CreateDirectory($fixture) | Out-Null
$repo = Join-Path $fixture 'repo'
$marker = Join-Path $fixture 'diff-loaded.txt'
New-Item -ItemType Directory -Path (Join-Path $repo 'hooks-none') -Force | Out-Null
git -C $repo init -q 2>&1 | Out-Null
git -C $repo config core.hooksPath hooks-none
git -C $repo config core.autocrlf false
$lines = 1..400 | ForEach-Object { "fn line_$_() -> u32 { $_ }" }
Set-Content -Path (Join-Path $repo 'sample.rs') -Value $lines
git -C $repo add sample.rs
git -C $repo -c user.name=repose-measure -c user.email=measure@example.invalid commit -q -m fixture
foreach ($i in 49, 199, 349) { $lines[$i] = "fn line_$($i + 1)() -> u32 { $($i + 1) * 2 }" }
Set-Content -Path (Join-Path $repo 'sample.rs') -Value $lines
$config = Join-Path $fixture 'config.toml'
Set-Content -Path $config -Value "roots = ['$fixture']"
$p = Start-Process -FilePath $Exe -PassThru -ArgumentList @(
    '--config', $config, '--select', $repo, '--file', 'sample.rs', '--measure-diff', $marker)
$deadline = (Get-Date).AddSeconds(60)
while (-not (Test-Path $marker) -and -not $p.HasExited -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 250 }
$loaded = if (Test-Path $marker) { @((Get-Content $marker | Select-Object -First 1) -split "`t") } else { $null }
# The marker counts only when it names this run's repository and file with a loaded hunk.
$hunks = 0
$valid = $loaded -and $loaded.Count -eq 5 -and $loaded[0] -eq 'diff-loaded' -and
    [string]::Equals([System.IO.Path]::GetFullPath($loaded[1]), [System.IO.Path]::GetFullPath($repo), 'OrdinalIgnoreCase') -and
    $loaded[2] -ceq 'sample.rs' -and [int]::TryParse($loaded[3], [ref]$hunks) -and $hunks -gt 0 -and $loaded[4] -match '^\d+$'
if ($loaded -and -not $valid) {
    Write-Host "MEASURE idle-private-working-set-mb unavailable (the diff marker does not name this fixture: $($loaded -join ' | '))"
} elseif ($valid -and -not $p.HasExited) {
    Start-Sleep -Seconds $IdleSeconds
    # The counter instance is matched by its ID Process, not its name: with another repose open,
    # the instance named repose may be that one (repose#1 is then ours).
    $ids = Get-Counter "\Process($($p.ProcessName)*)\ID Process" -ErrorAction SilentlyContinue
    $mine = $ids.CounterSamples | Where-Object { [int]$_.CookedValue -eq $p.Id } | Select-Object -First 1
    $counter = if ($mine) { Get-Counter ($mine.Path -replace '\\id process$', '\working set - private') -ErrorAction SilentlyContinue }
    $private = if ($counter) { [math]::Round($counter.CounterSamples[0].CookedValue / 1MB, 1) } else { 'unavailable' }
    $ws = [math]::Round((Get-Process -Id $p.Id).WorkingSet64 / 1MB, 1)
    Write-Host "MEASURE idle-private-working-set-mb $private (working set $ws; target about 150; diff open: $($loaded[2]) in the fixture repository, $($loaded[3]) hunks, loaded at $($loaded[4]) ms)"
} else {
    Write-Host 'MEASURE idle-private-working-set-mb unavailable (the fixture diff did not load within 60 s)'
}
if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force; $p.WaitForExit(10000) | Out-Null }
Remove-Item -LiteralPath $fixture -Recurse -Force -ErrorAction SilentlyContinue
