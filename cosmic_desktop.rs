use crate::framebuffer::{FramebufferConsole, HEIGHT, WIDTH};
use crate::input;
use crate::trmnl;
use crate::{cosmic_editor, cosmic_files, cosmic_settings, cosmic_store};

const CLOCK: &[u8] = b"5:22 PM";

pub fn run(mut screen: FramebufferConsole) -> ! {
    loop {
        draw_desktop(&mut screen);
        match input::read_scancode() {
            0x1E => show_applications(&mut screen),    // A
            0x14 => trmnl::run(&mut screen),           // T
            0x12 => cosmic_editor::run(&mut screen),   // E
            0x21 => cosmic_files::run(&mut screen),    // F
            0x1F => cosmic_settings::run(&mut screen), // S
            0x13 => cosmic_store::run(&mut screen),    // R
            _ => {}
        }
    }
}

fn draw_desktop(screen: &mut FramebufferConsole) {
    let colors = cosmic_settings::palette();
    screen.clear(colors.background);
    draw_panel(screen);
    screen.write_at(3, 4, b"COSMIC Desktop", colors.foreground);
    screen.write_at(4, 4, b"Personal workspace", colors.muted);
    draw_shortcut(screen, 7, 8, b"[trmnl]", colors.accent);
    draw_shortcut(screen, 7, 26, b"[Apps]", colors.foreground);
    screen.write_at(12, 8, b"Terminal", colors.foreground);
    screen.write_at(12, 26, b"Applications", colors.foreground);
    screen.write_at(21, 4, b"A  Applications", colors.muted);
    screen.write_at(21, 24, b"T  TRMNL Console", colors.muted);
    screen.write_at(
        22,
        4,
        b"E Editor  F Files  S Settings  R Store",
        colors.muted,
    );
    screen.write_at(
        HEIGHT - 2,
        4,
        b"BrainOS | Framebuffer workspace",
        colors.muted,
    );
}

fn show_applications(screen: &mut FramebufferConsole) {
    loop {
        draw_applications(screen);
        match input::read_scancode() {
            0x14 => trmnl::run(screen),
            0x12 => cosmic_editor::run(screen),
            0x21 => cosmic_files::run(screen),
            0x1F => cosmic_settings::run(screen),
            0x13 => cosmic_store::run(screen),
            0x01 => return,
            _ => {}
        }
    }
}

fn draw_applications(screen: &mut FramebufferConsole) {
    let colors = cosmic_settings::palette();
    screen.clear(colors.background);
    draw_panel(screen);
    screen.write_at(3, 4, b"Applications", colors.foreground);
    screen.write_at(4, 4, b"Native apps", colors.muted);
    draw_shortcut(screen, 7, 7, b"[trmnl]", colors.accent);
    draw_shortcut(screen, 7, 23, b"[Files]", colors.foreground);
    draw_shortcut(screen, 7, 39, b"[Settings]", colors.foreground);
    draw_shortcut(screen, 7, 55, b"[Store]", colors.foreground);
    draw_shortcut(screen, 14, 7, b"[Editor]", colors.foreground);
    screen.write_at(12, 7, b"T Terminal", colors.foreground);
    screen.write_at(12, 23, b"F Files", colors.foreground);
    screen.write_at(12, 39, b"S Settings", colors.foreground);
    screen.write_at(12, 55, b"R Store", colors.foreground);
    screen.write_at(19, 7, b"E Text Editor", colors.foreground);
    screen.write_at(
        HEIGHT - 2,
        4,
        b"Select with T/E/F/S/R  |  Esc Back",
        colors.muted,
    );
}

fn draw_panel(screen: &mut FramebufferConsole) {
    let colors = cosmic_settings::palette();
    screen.fill_cells(0, 0, WIDTH, colors.panel);
    screen.write_at(0, 0, b"Workspaces  Applications", colors.foreground);
    let clock_column = WIDTH.saturating_sub(CLOCK.len());
    screen.write_at(0, clock_column, CLOCK, colors.foreground);
    screen.write_at(1, 0, &[196; WIDTH], colors.muted);
}

fn draw_shortcut(
    screen: &mut FramebufferConsole,
    row: usize,
    column: usize,
    label: &[u8],
    color: u32,
) {
    const ICON_WIDTH: usize = 13;
    let mut top = [b' '; ICON_WIDTH];
    let mut middle = [b' '; ICON_WIDTH];
    let mut bottom = [b' '; ICON_WIDTH];
    top[0] = b'+';
    top[ICON_WIDTH - 1] = b'+';
    bottom[0] = b'+';
    bottom[ICON_WIDTH - 1] = b'+';
    middle[0] = b'|';
    middle[ICON_WIDTH - 1] = b'|';
    for cell in 1..ICON_WIDTH - 1 {
        top[cell] = b'-';
        bottom[cell] = b'-';
    }
    let label_column = (ICON_WIDTH.saturating_sub(label.len())) / 2;
    if label_column + label.len() < ICON_WIDTH {
        middle[label_column..label_column + label.len()].copy_from_slice(label);
    }
    screen.write_at(row, column, &top, color);
    screen.write_at(row + 1, column, &middle, color);
    screen.write_at(row + 2, column, &bottom, color);
}
