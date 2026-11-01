//! Colour and size tokens. Values from docs/specs/2026-11-01-repose-v1/technical-spec.md, Theme.

use eframe::egui::Color32;

pub const BG_APP: Color32 = Color32::from_rgb(0x10, 0x15, 0x18);
pub const BG_PANEL: Color32 = Color32::from_rgb(0x16, 0x1e, 0x22);
pub const BG_RAISED: Color32 = Color32::from_rgb(0x18, 0x23, 0x28);
pub const BG_HOVER: Color32 = Color32::from_rgb(0x23, 0x31, 0x37);
pub const LINE_SOFT: Color32 = Color32::from_rgb(0x26, 0x30, 0x36);
pub const LINE: Color32 = Color32::from_rgb(0x2b, 0x35, 0x39);
pub const LINE_STRONG: Color32 = Color32::from_rgb(0x39, 0x48, 0x4e);
pub const TEXT: Color32 = Color32::from_rgb(0xdc, 0xe4, 0xe4);
pub const TEXT_BRIGHT: Color32 = Color32::from_rgb(0xe0, 0xeb, 0xe8);
pub const MUTED: Color32 = Color32::from_rgb(0x97, 0xa8, 0xad);
pub const ACCENT: Color32 = Color32::from_rgb(0x87, 0xd6, 0xb2);
pub const ACCENT_BG: Color32 = Color32::from_rgb(0x28, 0x3c, 0x3a);
pub const ACCENT_LINE: Color32 = Color32::from_rgb(0x60, 0x8b, 0x7b);
pub const INFO: Color32 = Color32::from_rgb(0x80, 0x9f, 0xce);
pub const WARN: Color32 = Color32::from_rgb(0xe3, 0xbe, 0x7b);
pub const DANGER: Color32 = Color32::from_rgb(0xf0, 0x78, 0x78);
/// Diff line backgrounds, and the stronger fill behind the changed words within a line.
pub const ADDED_BG: Color32 = Color32::from_rgb(0x17, 0x30, 0x27);
pub const ADDED_MARK: Color32 = Color32::from_rgb(0x25, 0x52, 0x3d);
pub const REMOVED_BG: Color32 = Color32::from_rgb(0x38, 0x1f, 0x25);
pub const REMOVED_MARK: Color32 = Color32::from_rgb(0x5e, 0x2c, 0x35);

/// Base interface text size in points at 100% scaling.
pub const TEXT_BASE: f32 = 13.0;
/// The smallest text drawn anywhere: section labels and badges.
pub const TEXT_SMALL: f32 = 11.0;
pub const TEXT_TITLE: f32 = 17.0;
pub const ROW_HEIGHT: f32 = 46.0;
pub const SIDEBAR_WIDTH: f32 = 300.0;
pub const RADIUS: f32 = 4.0;
