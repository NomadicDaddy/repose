# PowerShell grammar

Vendored third-party file, decided by Phillip in room #30926 (2026-11-01): two-face 0.4.5 ships no
PowerShell syntax, so this one is compiled into the binary (`include_str!`) and added to the
highlighter's syntax set when the set is first built: once per process, on the first colour call,
on a reader thread (`OnceLock` in `highlight.rs`), about 420 ms in a release build.

- Source: <https://github.com/SublimeText/PowerShell>, `PowerShell.sublime-syntax` at commit
  `2938700b586aeaa9c1d2dc0c04a1cc569ff3c24e` (fetched 2026-11-01).
- Licence: MIT, `LICENSE.txt` beside it, copied unchanged.
- Upstream file SHA-256: `0b3a72aa262a3998429952162818e2f68c433bb3228680a21e8d066cc7bfda62`.
- One local change, in two places: `set: scope:source.cs.embedded.powershell` reads
  `set: scope:source.cs`. The embedded C# variant upstream (`syntax/embed/C# (for
  PowerShell).sublime-syntax`) is built with `extends`, which syntect does not load, so an
  `Add-Type` here-string is coloured with two-face's plain C# grammar instead. Nothing else
  differs from upstream.
