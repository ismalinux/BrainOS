use crate::cosmic_settings;
use crate::framebuffer::{FramebufferConsole, HEIGHT, WIDTH};
use crate::input;

const BUFFER_CAPACITY: usize = 1024;

pub fn run(screen: &mut FramebufferConsole) {
    let mut buffer = [0u8; BUFFER_CAPACITY];
    let mut length = 0usize;
    let mut message: &[u8] = b"Session buffer only; saving requires a VFS.";

    loop {
        let colors = cosmic_settings::palette();
        screen.clear(colors.background);
        screen.fill_cells(0, 0, WIDTH, colors.panel);
        screen.write_at(0, 2, b"TEXT EDITOR", colors.foreground);
        screen.write_at(2, 4, b"Untitled document", colors.accent);
        screen.write_at(3, 4, b"Plain text  |  fixed 1024-byte buffer", colors.muted);
        screen.set_cursor(5, 4);
        screen.write(&buffer[..length], colors.foreground);
        screen.write_at(HEIGHT - 3, 4, message, colors.muted);
        screen.write_at(
            HEIGHT - 2,
            4,
            b"Type to edit  Enter New Line  Backspace Delete  Esc Back",
            colors.muted,
        );

        match input::read_char() {
            b'\x1B' => return,
            b'\n' if length < BUFFER_CAPACITY => {
                buffer[length] = b'\n';
                length += 1;
                message = b"Changes are in the temporary session buffer.";
            }
            b'\x08' if length > 0 => {
                length -= 1;
                buffer[length] = 0;
            }
            character @ 0x20..=0x7E if length < BUFFER_CAPACITY => {
                buffer[length] = character;
                length += 1;
            }
            _ => {}
        }
    }
}
