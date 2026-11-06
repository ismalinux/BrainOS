use crate::framebuffer::{FramebufferConsole, DESKTOP, HEIGHT, STATUS_BAR, WIDTH};
use crate::input;
use core::sync::atomic::{AtomicBool, Ordering};

static DARK_THEME: AtomicBool = AtomicBool::new(true);
static HIGH_CONTRAST: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
pub struct Palette {
    pub background: u32,
    pub panel: u32,
    pub foreground: u32,
    pub muted: u32,
    pub accent: u32,
}

pub fn palette() -> Palette {
    let dark = DARK_THEME.load(Ordering::Relaxed);
    let high_contrast = HIGH_CONTRAST.load(Ordering::Relaxed);
    match (dark, high_contrast) {
        (true, true) => Palette {
            background: 0x101114,
            panel: 0x000000,
            foreground: 0xFFFFFF,
            muted: 0xC8CBD0,
            accent: 0xFFE080,
        },
        (true, false) => Palette {
            background: DESKTOP,
            panel: STATUS_BAR,
            foreground: 0xF1F2F4,
            muted: 0xB4B8C0,
            accent: 0x79B8F2,
        },
        (false, true) => Palette {
            background: 0xFFFFFF,
            panel: 0xE2E5E9,
            foreground: 0x111318,
            muted: 0x3D424A,
            accent: 0x004C9A,
        },
        (false, false) => Palette {
            background: 0xE9EBEF,
            panel: 0xD8DBE0,
            foreground: 0x202329,
            muted: 0x555B65,
            accent: 0x145D9F,
        },
    }
}

pub fn run(screen: &mut FramebufferConsole) {
    loop {
        let colors = palette();
        screen.clear(colors.background);
        screen.fill_cells(0, 0, WIDTH, colors.panel);
        screen.write_at(0, 2, b"COSMIC SETTINGS", colors.foreground);
        screen.write_at(2, 4, b"Appearance", colors.foreground);
        screen.write_at(5, 6, b"[D] Theme", colors.foreground);
        screen.write_at(
            5,
            30,
            if DARK_THEME.load(Ordering::Relaxed) {
                b"Dark"
            } else {
                b"Light"
            },
            colors.accent,
        );
        screen.write_at(8, 6, b"[C] High contrast", colors.foreground);
        screen.write_at(
            8,
            30,
            if HIGH_CONTRAST.load(Ordering::Relaxed) {
                b"On"
            } else {
                b"Off"
            },
            colors.accent,
        );
        screen.write_at(
            13,
            4,
            b"Changes apply to desktop and native app screens.",
            colors.muted,
        );
        screen.write_at(
            15,
            4,
            b"Settings are session-only; persistent storage is unavailable.",
            colors.muted,
        );
        screen.write_at(
            HEIGHT - 2,
            4,
            b"D Theme  C Contrast  Esc Back",
            colors.muted,
        );

        match input::read_scancode() {
            0x20 => DARK_THEME.store(!DARK_THEME.load(Ordering::Relaxed), Ordering::Relaxed),
            0x2E => HIGH_CONTRAST.store(!HIGH_CONTRAST.load(Ordering::Relaxed), Ordering::Relaxed),
            0x01 => return,
            _ => {}
        }
    }
}
