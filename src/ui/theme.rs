//! Shared color palette and chrome, styled after Claude Code's own CLI: bordered panels with
//! a warm coral accent used sparingly for the one thing that matters per screen — titles, key
//! numbers, the selected row. Color choices lean on basic color psychology on purpose:
//!   - the coral accent reads as "this is the app talking to you" — reserved for chrome and
//!     status, never used decoratively, so it stays meaningful.
//!   - green/amber/red for the usage bar map directly to "safe / caution / critical", the same
//!     association most people already have from traffic lights, so it lands without a legend.
//! Text itself is a single white — hierarchy is communicated by *weight* (bold vs. regular)
//! rather than by shading text through a gray scale: the thing that matters most on a line is
//! bold, everything else is regular. This keeps every word at full, unambiguous contrast
//! (no washed-out grays to squint at) while still making it obvious what to look at first.
//! Note: we deliberately avoid `Modifier::DIM` anywhere — on several terminal color profiles
//! it renders as illegible/near-invisible rather than a subtle fade.

use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::BorderType;

/// App background, painted explicitly behind every screen so the look is consistent
/// regardless of the terminal's own default background/theme.
pub const BG: Color = Color::Rgb(0x0F, 0x0F, 0x0E);
/// Lifted surface for the selected row — a visible highlight band. No `fg` is ever paired
/// with this at the call site: `List` patches `highlight_style` onto already-rendered cells,
/// and setting `fg` there would stomp the bar's own per-cell gradient color on that row.
pub const BG_SELECTED: Color = Color::Rgb(0x3A, 0x30, 0x28);

/// The one accent color in the whole UI — Claude Code's own warm coral. Used sparingly and
/// consistently: panel titles, the headline size number, the selected row.
pub const ACCENT: Color = Color::Rgb(0xD9, 0x77, 0x57);

/// The one text color. Pair with `Modifier::BOLD` at the call site for anything that should
/// stand out (titles, key numbers, the current selection); leave it regular for everything
/// else. See module docs for why this is weight-based rather than a gray scale.
pub const TEXT: Color = Color::Rgb(0xFA, 0xFA, 0xFA);
pub const BORDER: Color = Color::Rgb(0x3C, 0x3A, 0x35);

pub const SAFE: Color = Color::Rgb(0x6F, 0xB9, 0x71);
pub const WARNING: Color = Color::Rgb(0xD2, 0x99, 0x22);
pub const DANGER: Color = Color::Rgb(0xD9, 0x6B, 0x5A);

pub const PANEL_BORDER: BorderType = BorderType::Rounded;

/// Continuous green→amber→red interpolation for a value in `0.0..=1.0` — a real gradient
/// rather than a 3-band step function, so color shifts smoothly instead of jumping.
pub fn gradient(frac: f64) -> Color {
    let f = frac.clamp(0.0, 1.0);
    if f <= 0.5 {
        lerp(SAFE, WARNING, f / 0.5)
    } else {
        lerp(WARNING, DANGER, (f - 0.5) / 0.5)
    }
}

fn lerp(a: Color, b: Color, t: f64) -> Color {
    let (ar, ag, ab) = rgb_parts(a);
    let (br, bg, bb) = rgb_parts(b);
    let t = t.clamp(0.0, 1.0);
    Color::Rgb(
        (ar as f64 + (br as f64 - ar as f64) * t).round() as u8,
        (ag as f64 + (bg as f64 - ag as f64) * t).round() as u8,
        (ab as f64 + (bb as f64 - ab as f64) * t).round() as u8,
    )
}

fn rgb_parts(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (0, 0, 0),
    }
}

/// Builds a fixed-width usage bar as a heat-map "ruler": each filled cell is colored by its
/// own position along the bar (green near the start, red near the end) rather than one flat
/// color for the whole bar. This way the fill line landing in the red zone communicates
/// "this one's big relative to its parent" the same way a fuel gauge's red zone does, and a
/// bar that's mostly-red vs. mostly-green is legible even out of the corner of your eye.
pub fn gradient_bar(filled: usize, width: usize) -> Vec<Span<'static>> {
    (0..width)
        .map(|i| {
            if i < filled {
                let pos = if width <= 1 { 0.0 } else { i as f64 / (width - 1) as f64 };
                Span::styled("█", Style::default().fg(gradient(pos)))
            } else {
                Span::styled("░", Style::default().fg(BORDER))
            }
        })
        .collect()
}

pub const ICON_DIR: &str = "📁";
pub const ICON_FILE: &str = "📄";
pub const ICON_REPARSE: &str = "🔗";
// No drive icon: a terminal can't render Windows' actual per-drive shell icons (those are
// bitmap/vector resources, not Unicode glyphs), and every plausible Unicode stand-in (floppy
// disk, generic "hard disk" glyph) is a worse metaphor than just showing the drive letter.
