# The gate's pass-only step cache (quality-gate-cache). Dot-sourced by qc.ps1 and by
# gate-cache-test.ps1. A step that passed records the key of its complete inputs in
# target\gate-cache\<step>.json; the next gate skips that step only when the key is the same, the
# record parses, says pass, and any artifact it names is still there with the same SHA-256.
# Everything else reruns: a failure is never recorded (and removes an earlier pass), and a missing,
# unreadable or mismatched record is a miss. The key covers each input file's relative path and
# SHA-256 (so an added, removed or changed file is a miss) plus facts such as tool versions.
# Set $GateRoot before dot-sourcing; paths given to these functions are relative to it.

function Get-GateCacheDir { Join-Path $GateRoot 'target\gate-cache' }

function Get-GateCacheFile([string] $Name) {
    Join-Path (Get-GateCacheDir) (($Name -replace '[^A-Za-z0-9]+', '-').Trim('-') + '.json')
}

# SHA-256 over sorted "file <path> <hash>" lines for every file under the given paths (a folder
# counts recursively, hidden files included), "missing <path>" for a path not there, and
# "fact <text>" lines. Paths compare ordinally with forward slashes, so the order is stable.
function Get-GateInputKey([string[]] $Paths, [string[]] $Facts = @()) {
    $lines = [System.Collections.Generic.List[string]]::new()
    foreach ($fact in $Facts) { $lines.Add("fact $fact") }
    $files = [System.Collections.Generic.List[string]]::new()
    foreach ($p in $Paths) {
        $full = Join-Path $GateRoot $p
        if (Test-Path -LiteralPath $full -PathType Container) {
            Get-ChildItem -LiteralPath $full -Recurse -File -Force | ForEach-Object { $files.Add($_.FullName) }
        } elseif (Test-Path -LiteralPath $full -PathType Leaf) {
            $files.Add((Get-Item -LiteralPath $full).FullName)
        } else {
            $lines.Add("missing $($p.Replace('\', '/'))")
        }
    }
    $base = [System.IO.Path]::GetFullPath($GateRoot).TrimEnd('\') + '\'
    $entries = foreach ($f in $files) {
        $rel = if ($f.StartsWith($base, [System.StringComparison]::OrdinalIgnoreCase)) { $f.Substring($base.Length) } else { $f }
        "file $($rel.Replace('\', '/')) $((Get-FileHash -LiteralPath $f -Algorithm SHA256).Hash)"
    }
    $sorted = [string[]] @($entries | Select-Object -Unique)
    [System.Array]::Sort($sorted, [System.StringComparer]::Ordinal)
    foreach ($e in $sorted) { $lines.Add($e) }
    $bytes = [System.Text.Encoding]::UTF8.GetBytes(($lines -join "`n"))
    [System.Convert]::ToHexString([System.Security.Cryptography.SHA256]::HashData($bytes))
}

# The recorded pass for this step and key, or $null with the reason printed as a CACHE line.
function Get-GatePass([string] $Name, [string] $Key) {
    $file = Get-GateCacheFile $Name
    if (-not (Test-Path -LiteralPath $file)) { return $null }
    try {
        # -DateKind String keeps the recorded time as written, with its UTC offset, rather than
        # turning it into a local DateTime that prints without one.
        $r = Get-Content -LiteralPath $file -Raw | ConvertFrom-Json -DateKind String -ErrorAction Stop
    } catch {
        Write-Host "CACHE miss: $Name (the record does not parse)"
        return $null
    }
    if ($null -eq $r -or $r.step -cne $Name -or $r.result -cne 'pass' -or $r.key -cne $Key) {
        Write-Host "CACHE miss: $Name (inputs changed)"
        return $null
    }
    if ($r.artifact) {
        $art = Join-Path $GateRoot $r.artifact.path
        if (-not (Test-Path -LiteralPath $art -PathType Leaf)) {
            Write-Host "CACHE miss: $Name ($($r.artifact.path) is missing)"
            return $null
        }
        if ((Get-FileHash -LiteralPath $art -Algorithm SHA256).Hash -cne $r.artifact.sha256) {
            Write-Host "CACHE miss: $Name ($($r.artifact.path) changed since the pass)"
            return $null
        }
    }
    $r
}

function Save-GatePass([string] $Name, [string] $Key, [string] $Artifact = '', [string[]] $Lines = @()) {
    New-Item -ItemType Directory -Force -Path (Get-GateCacheDir) | Out-Null
    $record = [ordered] @{ step = $Name; result = 'pass'; key = $Key; at = (Get-Date).ToString('o'); lines = @($Lines) }
    if ($Artifact) {
        $hash = (Get-FileHash -LiteralPath (Join-Path $GateRoot $Artifact) -Algorithm SHA256).Hash
        $record.artifact = [ordered] @{ path = $Artifact; sha256 = $hash }
    }
    $record | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Get-GateCacheFile $Name) -Encoding utf8
}

function Clear-GatePass([string] $Name) {
    $file = Get-GateCacheFile $Name
    if (Test-Path -LiteralPath $file) { Remove-Item -LiteralPath $file -Force }
}

# Runs one gate step through the cache and returns $true when it passed (live or cached). The key
# is taken before the body runs, so a pass is recorded against the inputs it actually saw. With
# $NoCache the step always runs, and still records a pass for the next gate.
function Invoke-GateStep([string] $Name, [string[]] $Paths, [string[]] $Facts, [scriptblock] $Body, [string] $Artifact = '', [switch] $NoCache) {
    Write-Host "== $Name"
    # The artifact a step must leave is part of what the step is, so it is part of the key too.
    $key = Get-GateInputKey $Paths ($Facts + "artifact $Artifact")
    if (-not $NoCache) {
        $hit = Get-GatePass $Name $key
        if ($hit) {
            Write-Host "CACHED: $Name passed at $($hit.at) with these same inputs (key $($key.Substring(0, 12)))"
            return $true
        }
    }
    $global:LASTEXITCODE = 0
    # A command that cannot start (cargo off PATH, say) sets no exit code. Its error only ends the
    # body's own statement, so $? after the body still reads True; the try catches it instead.
    $started = $true
    try {
        # Out-Host keeps the step's output on screen and out of this function's return value.
        & $Body | Out-Host
    } catch {
        $started = $false
        Write-Host "ERROR: $Name could not run: $($_.Exception.Message)"
    }
    if ($LASTEXITCODE -ne 0 -or -not $started) {
        Clear-GatePass $Name
        Write-Host "FAILED: $Name"
        return $false
    }
    if ($Artifact -and -not (Test-Path -LiteralPath (Join-Path $GateRoot $Artifact) -PathType Leaf)) {
        Clear-GatePass $Name
        Write-Host "FAILED: $Name ($Artifact was not produced)"
        return $false
    }
    Save-GatePass $Name $key $Artifact
    $true
}
