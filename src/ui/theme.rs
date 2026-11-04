//! Colors derived from one background color, as in WinPick: luminance < 140
//! means dark; surfaces are mixed from the background towards white or
//! black; the accent is the Windows accent color.

use super::util::*;
use windows::core::BOOL;
use windows::Win32::Foundation::COLORREF;
use windows::Win32::Graphics::Dwm::DwmGetColorizationColor;

#[derive(Default, Clone, Copy)]
pub struct Theme {
    pub dark: bool,
    pub bg: COLORREF,
    pub panel: COLORREF,
    pub field: COLORREF,
    pub button: COLORREF,
    pub button_hot: COLORREF,
    pub button_down: COLORREF,
    pub text: COLORREF,
    pub text_dim: COLORREF,
    pub accent: COLORREF,
    pub select: COLORREF,
    pub line: COLORREF,
    pub blue: COLORREF,
    pub green: COLORREF,
    pub red: COLORREF,
    pub warn: COLORREF,
}

pub fn is_dark(c: COLORREF) -> bool {
    0.299 * cr_r(c) as f64 + 0.587 * cr_g(c) as f64 + 0.114 * (cr_b(c) as f64) < 140.0
}

pub unsafe fn build(bg: COLORREF) -> Theme {
    let dark = is_dark(bg);
    let up = if dark { rgb(255, 255, 255) } else { rgb(0, 0, 0) };
    let text = if dark { rgb(238, 238, 240) } else { rgb(24, 24, 28) };
    let mut accent = rgb(0, 120, 215);
    let mut argb = 0u32;
    let mut opaque = BOOL(0);
    if DwmGetColorizationColor(&mut argb, &mut opaque).is_ok() {
        accent = rgb((argb >> 16) as u8, (argb >> 8) as u8, argb as u8);
    }
    Theme {
        dark,
        bg,
        panel: mix(bg, up, if dark { 0.04 } else { 0.03 }),
        field: if dark {
            mix(bg, up, 0.10)
        } else {
            mix(bg, rgb(255, 255, 255), 0.85)
        },
        button: mix(bg, up, if dark { 0.12 } else { 0.08 }),
        button_hot: mix(bg, up, if dark { 0.18 } else { 0.13 }),
        button_down: mix(bg, up, if dark { 0.24 } else { 0.18 }),
        text,
        text_dim: mix(text, bg, 0.45),
        accent,
        select: mix(bg, accent, if dark { 0.45 } else { 0.30 }),
        line: mix(bg, text, 0.25),
        blue: if dark { rgb(130, 180, 255) } else { rgb(0, 84, 190) },
        green: if dark { rgb(120, 220, 140) } else { rgb(0, 120, 40) },
        red: if dark { rgb(255, 130, 130) } else { rgb(190, 20, 20) },
        warn: if dark { rgb(240, 200, 100) } else { rgb(150, 100, 0) },
    }
}
