use crate::cosmic_settings;
use crate::framebuffer::{FramebufferConsole, HEIGHT, WIDTH};
use crate::input;

const ENTRY_COUNT: usize = 3;
const ENTRIES: [&[u8]; ENTRY_COUNT] = [b"System volume", b"User files", b"Applications catalog"];

pub fn run(screen: &mut FramebufferConsole) {
    let mut selected = 0usize;
    let mut message: &[u8] = b"VFS is not mounted; file operations are disabled.";

    loop {
        let colors = cosmic_settings::palette();
        screen.clear(colors.background);
        screen.write_at(0, 0, &[b' '; WIDTH], colors.panel);
        screen.write_at(0, 2, b"COSMIC FILES", colors.panel);
        screen.write_at(2, 4, b"Location", colors.foreground);
        screen.write_at(3, 4, b"/", colors.accent);
        screen.write_at(5, 4, b"Name", colors.muted);
        screen.write_at(5, 48, b"Status", colors.muted);

        for (index, name) in ENTRIES.iter().enumerate() {
            let row = 8 + index * 2;
            let color = if index == selected {
                colors.accent
            } else {
                colors.foreground
            };
            screen.write_at(row, 4, if index == selected { b">" } else { b" " }, color);
            screen.write_at(row, 7, name, color);
            screen.write_at(
                row,
                48,
                match index {
                    0 | 1 => b"Unavailable",
                    _ => b"Built-in",
                },
                colors.muted,
            );
        }

        screen.write_at(17, 4, message, colors.muted);
        screen.write_at(
            HEIGHT - 2,
            4,
            b"W/S Select  Enter Details  Esc Back",
            colors.muted,
        );

        match input::read_scancode() {
            0x11 => selected = selected.saturating_sub(1),
            0x1F => selected = (selected + 1).min(ENTRY_COUNT - 1),
            0x1C => {
                message = match selected {
                    0 => b"No block device or mounted root filesystem is available.",
                    1 => b"User directories require a VFS and storage driver.",
                    _ => b"Apps are registered in the built-in desktop catalog.",
                };
            }
            0x01 => return,
            _ => {}
        }
    }
}
