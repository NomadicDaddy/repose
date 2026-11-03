# Licence texts fetched from upstream repositories

Some crates publish their licence files at their repository root and not in the crates.io
package, so `scripts/third-party-notices.ps1` finds nothing in the package and reads these
copies instead (Morticia M-1 on b991042: the texts must ship with the executable). Each folder
is keyed by the repository and mapped to its crates in the script. Fetched 2026-11-01 with curl,
unchanged.

| Folder | Crates | Source |
|---|---|---|
| egui | ecolor, eframe, egui, egui_glow, egui_system_font_provider, egui-wgpu, egui-winit, emath, epaint, epaint_default_fonts | <https://github.com/emilk/egui> tag 0.37.0: LICENSE-MIT, LICENSE-APACHE |
| accesskit | accesskit, accesskit_consumer, accesskit_windows, accesskit_winit | <https://github.com/AccessKit/accesskit> main at f3026222559c7f5f46d8f25cb7404de68eb149f0: LICENSE-MIT, LICENSE-APACHE, LICENSE.chromium |
| clipboard-win | clipboard-win | <https://github.com/DoumanAsh/clipboard-win> master at ed27008ae5d1f10cfbb266de039f0fb35fd05a56: LICENSE (BSL-1.0) |
| profiling | profiling | <https://github.com/aclysma/profiling> master at da44bd1c5497c665c0f00e5d70594fc14d390b4b: LICENSE-MIT, LICENSE-APACHE |
| rspirv | spirv | <https://github.com/gfx-rs/rspirv> master at 7e6f38e1cf4b787a4d38409272c8c9f30541f0fd: LICENSE (Apache-2.0, the expression in spirv 0.4.0+sdk-1.4.341.0's Cargo.toml; fetched because the package publishes no licence file) |
| two-face | embedded syntax and theme dumps | <https://github.com/CosmicHorrorDev/two-face/blob/v0.4.5/generated/acknowledgements_full.md> at tag v0.4.5: source SHA256 `55BE6F98F911DF5CECC95275D95B75A916C3F4EFB5F0DD322C7F835A60AEDEAB`; vendored copy normalizes line endings to LF, removes trailing whitespace and terminal blank lines, SHA256 `AFCFB536E62C5F78AF45AAD96498F658E8494F151AE0835440F3BE5A91C2B00B` |

A crate that is neither in its package nor here fails the generator, so a new dependency of
this kind is noticed at the gate.
