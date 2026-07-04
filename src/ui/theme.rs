//! Shared color palette and chrome. A deliberate dark background (rather than whatever the
//! user's terminal profile happens to default to) plus a small, disciplined color hierarchy:
//! near-white for primary text, a clearly-legible (not washed-out) gray for secondary text,
//! and a calm blue accent reserved for titles, key numbers, and the selection highlight.

use ratatui::style::Color;
use ratatui::widgets::BorderType;

/// App background, painted explicitly behind every screen so the look is consistent
/// regardless of the terminal's own default background/theme.
pub const BG: Color = Color::Rgb(0x14, 0x16, 0x1A);
/// Slightly lifted background for the selected row, used instead of a saturated accent
/// fill so per-row text/bar colors stay legible on top of it (see `ui::tree_view`'s
/// `highlight_style` comment for why fg is never set here).
pub const BG_SELECTED: Color = Color::Rgb(0x27, 0x3A, 0x4D);

pub const ACCENT: Color = Color::Rgb(0x5A, 0xA8, 0xE6); // calm, clearly-visible blue
pub const TEXT: Color = Color::Rgb(0xF2, 0xF3, 0xF5); // primary text: near-white, high contrast
pub const SUBTEXT: Color = Color::Rgb(0xB8, 0xBE, 0xC6); // secondary text: legible, not washed out
pub const BORDER: Color = Color::Rgb(0x3A, 0x3F, 0x47);

pub const SUCCESS: Color = Color::Rgb(0x6F, 0xC9, 0x8E);
pub const WARNING: Color = Color::Rgb(0xE0, 0xAF, 0x68);
pub const DANGER: Color = Color::Rgb(0xE0, 0x6C, 0x6C);

pub const PANEL_BORDER: BorderType = BorderType::Rounded;

/// Size-relative color for a usage bar/percentage: calm green when small, amber in the
/// middle, red when an entry dominates its parent.
pub fn size_color(frac: f64) -> Color {
    if frac > 0.5 {
        DANGER
    } else if frac > 0.2 {
        WARNING
    } else {
        SUCCESS
    }
}

pub const ICON_DIR: &str = "📁";
pub const ICON_FILE: &str = "📄";
pub const ICON_REPARSE: &str = "🔗";
pub const ICON_DRIVE: &str = "💾";
