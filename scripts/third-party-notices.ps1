# Writes THIRD-PARTY-NOTICES.md at the repository root: every Rust crate linked into the release
# executable for this host (normal dependencies reached from the repose binary without passing
# through a proc-macro crate; build-only, dev-only and proc-macro crates are compile-time inputs,
# not shipped code), with its licence expression, repository and the licence texts from its own
# package, each distinct text printed once; and every vendored file with its licence text. A crate
# whose package carries no licence file fails the run. qc.ps1 runs -Check and copies the file
# beside the executable. Usage: pwsh scripts/third-party-notices.ps1 [-Check]
param(
    # Exit 1 instead of writing when the file on disk differs, byte for byte, from what would be written.
    [switch] $Check
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$hostLine = rustc -vV | Select-String '^host: (.+)$'
if ($LASTEXITCODE -ne 0 -or -not $hostLine) { Write-Host 'FAILED: rustc -vV'; exit 1 }
$hostTriple = $hostLine.Matches[0].Groups[1].Value.Trim()
$metaJson = cargo metadata --format-version 1 --locked --filter-platform $hostTriple
if ($LASTEXITCODE -ne 0) { Write-Host 'FAILED: cargo metadata --locked'; exit 1 }
$meta = $metaJson | ConvertFrom-Json
$byId = @{}
foreach ($p in $meta.packages) { $byId[$p.id] = $p }
$nodes = @{}
foreach ($n in $meta.resolve.nodes) { $nodes[$n.id] = $n }
$rootId = ($meta.packages | Where-Object { $_.name -eq 'repose' }).id

function Is-ProcMacro($pkg) {
    foreach ($t in $pkg.targets) { if ($t.kind -contains 'proc-macro') { return $true } }
    return $false
}

# Walk normal dependencies from the binary, never through a proc-macro crate.
$seen = @{}
$queue = New-Object System.Collections.Generic.Queue[string]
$queue.Enqueue($rootId)
while ($queue.Count -gt 0) {
    $id = $queue.Dequeue()
    if ($seen.ContainsKey($id)) { continue }
    $seen[$id] = $true
    if (Is-ProcMacro $byId[$id]) { continue }
    foreach ($d in $nodes[$id].deps) {
        $normal = $false
        foreach ($k in $d.dep_kinds) { if ($null -eq $k.kind) { $normal = $true } }
        if ($normal -and -not $seen.ContainsKey($d.pkg)) { $queue.Enqueue($d.pkg) }
    }
}
$workspace = @($meta.workspace_members)
$crates = $seen.Keys |
    Where-Object { $workspace -notcontains $_ -and -not (Is-ProcMacro $byId[$_]) } |
    ForEach-Object { $byId[$_] } |
    Sort-Object name, version

# Crates whose packages carry no licence file: the texts come from licenses/<folder>, fetched
# from their repositories (licenses/SOURCE.md says where and when).
$fetched = @{
    'ecolor' = 'egui'; 'eframe' = 'egui'; 'egui' = 'egui'; 'egui_glow' = 'egui'
    'egui_system_font_provider' = 'egui'; 'egui-wgpu' = 'egui'; 'egui-winit' = 'egui'
    'emath' = 'egui'; 'epaint' = 'egui'; 'epaint_default_fonts' = 'egui'
    'accesskit' = 'accesskit'; 'accesskit_consumer' = 'accesskit'
    'accesskit_windows' = 'accesskit'; 'accesskit_winit' = 'accesskit'
    'clipboard-win' = 'clipboard-win'; 'profiling' = 'profiling'; 'spirv' = 'rspirv'
}
$licencePattern = '^(LICEN[CS]E|COPYING|COPYRIGHT|NOTICE|UNLICENSE|OFL|UFL)'
# Licence files whose names do not say so, named per crate and required to exist, so a renamed
# or dropped file fails the run instead of silently leaving a text out (Morticia L-4 on 1a4888a):
# the fonts compiled into epaint_default_fonts carry their licences as fonts/<name>.txt.
$explicit = @{
    'epaint_default_fonts' = @('fonts/egui-icons.txt', 'fonts/emoji-icon-font-mit-license.txt', 'fonts/Hack-Regular.txt')
}

# Licence texts, each distinct text once, remembering which crate files carry it. A package is
# searched two folders deep, since font licences sit beside the fonts.
$texts = [ordered]@{}
$missing = New-Object System.Collections.Generic.List[string]
$filesOf = @{}
foreach ($c in $crates) {
    $dir = Split-Path -Parent $c.manifest_path
    $files = @(Get-ChildItem -Path $dir -File -Recurse -Depth 2 |
        Where-Object { $_.Name -match $licencePattern } |
        Sort-Object FullName)
    if ($explicit.ContainsKey($c.name)) {
        foreach ($rel in $explicit[$c.name]) {
            $path = Join-Path $dir $rel
            if (-not (Test-Path $path)) { Write-Host "FAILED: $($c.name) $($c.version) has no $rel"; exit 1 }
            $files = @($files) + @(Get-Item $path)
        }
    }
    if ($fetched.ContainsKey($c.name)) {
        $extra = @(Get-ChildItem -Path (Join-Path $root "licenses/$($fetched[$c.name])") -File | Sort-Object Name)
        $files = @($files) + $extra
    }
    if ($files.Count -eq 0) { $missing.Add("$($c.name) $($c.version)"); continue }
    $names = New-Object System.Collections.Generic.List[string]
    foreach ($f in $files) {
        $bytes = [System.IO.File]::ReadAllBytes($f.FullName)
        $hash = [System.BitConverter]::ToString([System.Security.Cryptography.SHA256]::HashData($bytes)).Replace('-', '').Substring(0, 16)
        if (-not $texts.Contains($hash)) {
            $texts[$hash] = @{ text = [System.Text.Encoding]::UTF8.GetString($bytes); users = New-Object System.Collections.Generic.List[string] }
        }
        $shown = if ($f.FullName.StartsWith($dir)) { $f.FullName.Substring($dir.Length + 1).Replace('\', '/') } else { "licenses/$($fetched[$c.name])/$($f.Name)" }
        $texts[$hash].users.Add("$($c.name) $($c.version): $shown")
        $names.Add("$shown ($hash)")
    }
    $filesOf[$c.id] = $names -join ', '
}
if ($missing.Count -gt 0) {
    Write-Host "FAILED: no licence file in the package or under licenses/ for: $($missing -join ', ')"
    exit 1
}

# two-face compiles syntax and theme dumps into the executable. Their collected notices are
# separate from the crate's own MIT/Apache licence files and must travel with the release pair.
$twoFace = @($crates | Where-Object { $_.name -eq 'two-face' })
if ($twoFace.Count -ne 1 -or $twoFace[0].version -ne '0.4.5') {
    Write-Host 'FAILED: update the embedded two-face notices for the locked two-face version'
    exit 1
}
$twoFacePath = Join-Path $root 'licenses/two-face/acknowledgements_full.md'
if (-not (Test-Path -LiteralPath $twoFacePath)) {
    Write-Host 'FAILED: missing two-face syntax and theme acknowledgements'
    exit 1
}
$twoFaceBytes = [System.IO.File]::ReadAllBytes($twoFacePath)
$twoFaceHash = [Convert]::ToHexString([System.Security.Cryptography.SHA256]::HashData($twoFaceBytes))
if ($twoFaceHash -ne 'AFCFB536E62C5F78AF45AAD96498F658E8494F151AE0835440F3BE5A91C2B00B') {
    Write-Host 'FAILED: two-face syntax and theme acknowledgements differ from the pinned upstream text'
    exit 1
}
$twoFaceText = [System.Text.UTF8Encoding]::new($false, $true).GetString($twoFaceBytes)

$lines = New-Object System.Collections.Generic.List[string]
$lines.Add('# Third-party notices')
$lines.Add('')
$lines.Add('repose is MIT. It ships one executable that links the Rust crates below and compiles in the')
$lines.Add('vendored files after them. Each crate is under the licence its authors chose, given as its')
$lines.Add('SPDX expression from the crate metadata, and every licence, copying and notice file found in')
$lines.Add('its package (or, for a crate that publishes none in its package, fetched from its repository')
$lines.Add('into licenses/, see licenses/SOURCE.md) is reproduced in the Licence texts section, each')
$lines.Add('distinct text once. The list is')
$lines.Add('the closure of normal dependencies from the repose binary, not entered through proc-macro')
$lines.Add("crates; build-only, dev-only and proc-macro crates are compile-time inputs and are not listed.")
$lines.Add("Generated by scripts/third-party-notices.ps1 from ``cargo metadata --locked`` for $hostTriple;")
$lines.Add('run it after a dependency change (the gate fails when this file is stale).')
$lines.Add('')
$lines.Add("## Rust crates ($($crates.Count))")
$lines.Add('')
$lines.Add('| Crate | Version | Licence | Source | Licence files (text id) |')
$lines.Add('|---|---|---|---|---|')
foreach ($c in $crates) {
    $licence = if ($c.license) { $c.license } else { 'not declared' }
    $source = if ($c.repository) { $c.repository } else { '' }
    $lines.Add("| $($c.name) | $($c.version) | $licence | $source | $($filesOf[$c.id]) |")
}
$lines.Add('')
$lines.Add('### Licences in use')
$lines.Add('')
$crates | Group-Object { if ($_.license) { $_.license } else { 'not declared' } } | Sort-Object Name | ForEach-Object {
    $lines.Add("- $($_.Name): $($_.Count)")
}
$lines.Add('')
$lines.Add('## Vendored files')
$lines.Add('')
$lines.Add('### PowerShell grammar (crates/app/syntaxes/PowerShell)')
$lines.Add('')
$lines.Add('`PowerShell.sublime-syntax` from <https://github.com/SublimeText/PowerShell>, MIT, compiled')
$lines.Add('into the executable; provenance and the one local edit are in `SOURCE.md` beside it. Its')
$lines.Add('licence:')
$lines.Add('')
$lines.Add('```')
foreach ($l in (Get-Content (Join-Path $root 'crates/app/syntaxes/PowerShell/LICENSE.txt'))) { $lines.Add($l) }
$lines.Add('```')
$lines.Add('')
$lines.Add('### two-face 0.4.5 embedded syntax and theme dumps')
$lines.Add('')
$lines.Add('`two_face::syntax::extra_newlines` and `two_face::theme::extra` compile in the')
$lines.Add('`syntaxes-fancy-newlines.bin` and `themes.bin` dumps. The full upstream syntax and')
$lines.Add('theme acknowledgements below are pinned in `licenses/two-face/`; see `licenses/SOURCE.md`.')
$lines.Add('')
foreach ($l in ($twoFaceText -split "`r?`n")) { $lines.Add($l.TrimEnd()) }
$lines.Add('')
$lines.Add('## Not shipped')
$lines.Add('')
$lines.Add('Git is a prerequisite the user installs; repose runs the git on PATH and bundles none. The')
$lines.Add('editor and terminal it opens (VS Code, Windows Terminal, pwsh) are likewise the user''s own.')
$lines.Add('Segoe UI and Cascadia are read from the Windows font folder at run time, never copied; the')
$lines.Add('fallback fonts are those the egui crates above bundle, under the licences listed for them.')
$lines.Add('')
$lines.Add("## Licence texts ($($texts.Count) distinct)")
$lines.Add('')
$lines.Add('Each text once, headed by the crate files that carry it (text id in the table above).')
foreach ($hash in $texts.Keys) {
    $lines.Add('')
    $lines.Add("### $hash")
    $lines.Add('')
    foreach ($u in $texts[$hash].users) { $lines.Add("- $u") }
    $lines.Add('')
    $lines.Add('```')
    foreach ($l in ($texts[$hash].text -split "`r?`n")) { $lines.Add($l) }
    # A text that ended in a newline split into a trailing empty line; drop it.
    while ($lines[$lines.Count - 1] -eq '') { $lines.RemoveAt($lines.Count - 1) }
    $lines.Add('```')
}

# Reproduced texts keep their trailing spaces: the licence is copied as its authors wrote it.
$text = ($lines -join "`n") + "`n"
$bytes = (New-Object System.Text.UTF8Encoding $false).GetBytes($text)
$out = Join-Path $root 'THIRD-PARTY-NOTICES.md'
if ($Check) {
    # Bytes, not decoded text, so a BOM or an encoding difference counts too (Roger 5Q-2 on 1a4888a).
    $current = if (Test-Path $out) { [System.IO.File]::ReadAllBytes($out) } else { [byte[]]@() }
    if ($current.Length -ne $bytes.Length -or -not [System.Linq.Enumerable]::SequenceEqual([byte[]]$current, [byte[]]$bytes)) {
        Write-Host 'FAILED: THIRD-PARTY-NOTICES.md is out of date; run scripts/third-party-notices.ps1'
        exit 1
    }
    Write-Host "THIRD-PARTY-NOTICES.md is current ($($crates.Count) crates, $($texts.Count) licence texts)"
    exit 0
}
[System.IO.File]::WriteAllBytes($out, $bytes)
Write-Host "wrote $out ($($crates.Count) crates, $($texts.Count) licence texts)"
