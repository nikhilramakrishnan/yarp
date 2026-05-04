use pathfinder_color::ColorU;
use yarp_core::ui::theme::{
    AnsiColor, AnsiColors, Details, Fill, TerminalColors, YarpTheme,
};

const DARK_MODE_NORMAL_COLORS: AnsiColors = AnsiColors::new(
    AnsiColor::from_u32(0x616161FF),
    AnsiColor::from_u32(0xFF8272FF),
    AnsiColor::from_u32(0xB4FA72FF),
    AnsiColor::from_u32(0xFEFDC2FF),
    AnsiColor::from_u32(0xA5D5FEFF),
    AnsiColor::from_u32(0xFF8FFDFF),
    AnsiColor::from_u32(0xD0D1FEFF),
    AnsiColor::from_u32(0xF1F1F1FF),
);
const DARK_MODE_BRIGHT_COLORS: AnsiColors = AnsiColors::new(
    AnsiColor::from_u32(0x8E8E8EFF),
    AnsiColor::from_u32(0xFFC4BDFF),
    AnsiColor::from_u32(0xD6FCB9FF),
    AnsiColor::from_u32(0xFEFDD5FF),
    AnsiColor::from_u32(0xC1E3FEFF),
    AnsiColor::from_u32(0xFFB1FEFF),
    AnsiColor::from_u32(0xE5E6FEFF),
    AnsiColor::from_u32(0xFEFFFFFF),
);

const LIGHT_MODE_NORMAL_COLORS: AnsiColors = AnsiColors::new(
    AnsiColor::from_u32(0x212121FF),
    AnsiColor::from_u32(0xC30771FF),
    AnsiColor::from_u32(0x10A778FF),
    AnsiColor::from_u32(0xA89C14FF),
    AnsiColor::from_u32(0x008EC4FF),
    AnsiColor::from_u32(0x523C79FF),
    AnsiColor::from_u32(0x20A5BAFF),
    AnsiColor::from_u32(0xE0E0E0FF),
);
const LIGHT_MODE_BRIGHT_COLORS: AnsiColors = AnsiColors::new(
    AnsiColor::from_u32(0x212121FF),
    AnsiColor::from_u32(0xFB007AFF),
    AnsiColor::from_u32(0x5FD7AFFF),
    AnsiColor::from_u32(0xF3E430FF),
    AnsiColor::from_u32(0x20BBFCFF),
    AnsiColor::from_u32(0x6855DEFF),
    AnsiColor::from_u32(0x4FB8CCFF),
    AnsiColor::from_u32(0xF1F1F1FF),
);

pub(super) fn light_mode_colors() -> TerminalColors {
    TerminalColors::new(LIGHT_MODE_NORMAL_COLORS, LIGHT_MODE_BRIGHT_COLORS)
}

pub(super) fn dark_mode_colors() -> TerminalColors {
    TerminalColors::new(DARK_MODE_NORMAL_COLORS, DARK_MODE_BRIGHT_COLORS)
}

const SANDFORD_NORMAL_COLORS: AnsiColors = AnsiColors::new(
    AnsiColor::from_u32(0x1A2438FF),
    AnsiColor::from_u32(0x9F2A2AFF),
    AnsiColor::from_u32(0x6B8E5CFF),
    AnsiColor::from_u32(0xD4A24CFF),
    AnsiColor::from_u32(0x5C7A99FF),
    AnsiColor::from_u32(0xA66B8AFF),
    AnsiColor::from_u32(0x7FA89EFF),
    AnsiColor::from_u32(0xF5EFD8FF),
);
const SANDFORD_BRIGHT_COLORS: AnsiColors = AnsiColors::new(
    AnsiColor::from_u32(0x4A5468FF),
    AnsiColor::from_u32(0xC44545FF),
    AnsiColor::from_u32(0x8AAB7AFF),
    AnsiColor::from_u32(0xE8B85FFF),
    AnsiColor::from_u32(0x7E97B7FF),
    AnsiColor::from_u32(0xC18BAAFF),
    AnsiColor::from_u32(0x9CC1B6FF),
    AnsiColor::from_u32(0xFFFAECFF),
);

const GREATER_GOOD_NORMAL_COLORS: AnsiColors = AnsiColors::new(
    AnsiColor::from_u32(0x2A1F14FF),
    AnsiColor::from_u32(0x8C2424FF),
    AnsiColor::from_u32(0x4A6B3AFF),
    AnsiColor::from_u32(0xB07B1AFF),
    AnsiColor::from_u32(0x3A5878FF),
    AnsiColor::from_u32(0x804A6BFF),
    AnsiColor::from_u32(0x4A766BFF),
    AnsiColor::from_u32(0xE8D9B8FF),
);
const GREATER_GOOD_BRIGHT_COLORS: AnsiColors = AnsiColors::new(
    AnsiColor::from_u32(0x4D3D2CFF),
    AnsiColor::from_u32(0xB23030FF),
    AnsiColor::from_u32(0x6E8C5CFF),
    AnsiColor::from_u32(0xD49830FF),
    AnsiColor::from_u32(0x5A789CFF),
    AnsiColor::from_u32(0xA06A89FF),
    AnsiColor::from_u32(0x6E9489FF),
    AnsiColor::from_u32(0xF5E6C8FF),
);

pub(super) fn sandford_colors() -> TerminalColors {
    TerminalColors::new(SANDFORD_NORMAL_COLORS, SANDFORD_BRIGHT_COLORS)
}

pub(super) fn greater_good_colors() -> TerminalColors {
    TerminalColors::new(GREATER_GOOD_NORMAL_COLORS, GREATER_GOOD_BRIGHT_COLORS)
}

/// Default bundled dark theme — Sandford after dusk: police-uniform navy
/// background, parchment foreground, constabulary red cursor.
pub fn dark_theme() -> YarpTheme {
    YarpTheme::new(
        Fill::Solid(ColorU::from_u32(0x0E1A2EFF)),
        ColorU::from_u32(0xF5EFD8FF),
        Fill::Solid(ColorU::from_u32(0x9F2A2AFF)),
        None,
        Some(Details::Darker),
        sandford_colors(),
        None,
        Some("Sandford".to_string()),
    )
}

/// Default bundled light theme — pub interior: amber cream background,
/// dark wood foreground, constabulary red accent. The Greater Good.
pub fn light_theme() -> YarpTheme {
    YarpTheme::new(
        Fill::Solid(ColorU::from_u32(0xF5E6C8FF)),
        ColorU::from_u32(0x2A1F14FF),
        Fill::Solid(ColorU::from_u32(0x9F2A2AFF)),
        None,
        Some(Details::Lighter),
        greater_good_colors(),
        None,
        Some("The Greater Good".to_string()),
    )
}
