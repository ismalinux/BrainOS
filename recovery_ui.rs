use crate::framebuffer::{FramebufferConsole, BLUE, HEIGHT, RECOVERY_FOREGROUND, RED, WIDTH};
use crate::input;
use crate::trmnl;
use crate::BootStatus;

const BOX_COLOR_BLUE: u32 = 0xF4C84A;
const BOX_COLOR_RED: u32 = 0xFFFFFF;
const BOX_WIDTH: usize = 24;
const BOX_HEIGHT: usize = 5;
const BOX_GAP: usize = 4;

pub fn run(mut screen: FramebufferConsole, status: BootStatus) -> ! {
    loop {
        draw_recovery(&mut screen, status);
        match input::read_scancode() {
            0x02 | 0x1C => trmnl::run(&mut screen), // 1 or Enter
            0x03 => show_diagnostics(&mut screen, status),
            _ => {}
        }
    }
}

fn draw_recovery(screen: &mut FramebufferConsole, status: BootStatus) {
    let background = if status == BootStatus::SecurityBreach {
        RED
    } else {
        BLUE
    };
    screen.clear(background);
    screen.write_at(2, 0, b"BRAIN OS // EMERGENCY RECOVERY", RECOVERY_FOREGROUND);
    match status {
        BootStatus::Normal => {
            screen.write_at(4, 0, b"Unexpected recovery request.", RECOVERY_FOREGROUND)
        }
        BootStatus::SystemCrash => screen.write_at(
            4,
            0,
            b"System crash or hardware error detected.",
            RECOVERY_FOREGROUND,
        ),
        BootStatus::SecurityBreach => screen.write_at(
            4,
            0,
            b"Security alert. System halted for safety.",
            RECOVERY_FOREGROUND,
        ),
    }
    let box_color = if status == BootStatus::SecurityBreach {
        BOX_COLOR_RED
    } else {
        BOX_COLOR_BLUE
    };
    let group_width = BOX_WIDTH * 2 + BOX_GAP;
    let first_column = (WIDTH - group_width) / 2;
    let row = (HEIGHT - BOX_HEIGHT) / 2;
    draw_box(screen, row, first_column, BOX_WIDTH, BOX_HEIGHT, box_color);
    draw_box(
        screen,
        row,
        first_column + BOX_WIDTH + BOX_GAP,
        BOX_WIDTH,
        BOX_HEIGHT,
        box_color,
    );
    centered_text(
        screen,
        row + 2,
        first_column,
        BOX_WIDTH,
        b"[1] TRMNL",
        box_color,
    );
    centered_text(
        screen,
        row + 2,
        first_column + BOX_WIDTH + BOX_GAP,
        BOX_WIDTH,
        b"[2] DIAGNOSTICS",
        box_color,
    );
    screen.write_at(22, 0, b"Select with 1/Enter or 2", RECOVERY_FOREGROUND);
}

fn show_diagnostics(screen: &mut FramebufferConsole, status: BootStatus) {
    let background = if status == BootStatus::SecurityBreach {
        RED
    } else {
        BLUE
    };
    screen.clear(background);
    screen.write_at(2, 0, b"BRAIN OS // DIAGNOSTICS", RECOVERY_FOREGROUND);
    let box_color = if status == BootStatus::SecurityBreach {
        BOX_COLOR_RED
    } else {
        BOX_COLOR_BLUE
    };
    let column = (WIDTH - BOX_WIDTH) / 2;
    let row = (HEIGHT - BOX_HEIGHT) / 2;
    draw_box(screen, row, column, BOX_WIDTH, BOX_HEIGHT, box_color);
    screen.write_at(row + 1, column + 2, b"Boot status:", box_color);
    let label: &[u8] = match status {
        BootStatus::Normal => b"NORMAL",
        BootStatus::SystemCrash => b"SYSTEM CRASH",
        BootStatus::SecurityBreach => b"SECURITY BREACH",
    };
    centered_text(screen, row + 2, column + 1, BOX_WIDTH - 2, label, box_color);
    screen.write_at(22, 0, b"Press Escape to return", RECOVERY_FOREGROUND);
    while input::read_scancode() != 0x01 {}
}

fn draw_box(
    screen: &mut FramebufferConsole,
    row: usize,
    column: usize,
    width: usize,
    height: usize,
    color: u32,
) {
    if width < 2 || height < 2 || row >= HEIGHT || column >= WIDTH {
        return;
    }
    let width = width.min(WIDTH - column);
    let height = height.min(HEIGHT - row);
    if width < 2 || height < 2 {
        return;
    }

    let mut border = [b' '; WIDTH];
    border[0] = 201;
    border[width - 1] = 187;
    border[1..width - 1].fill(205);
    screen.write_at(row, column, &border[..width], color);

    border[0] = 200;
    border[width - 1] = 188;
    screen.write_at(row + height - 1, column, &border[..width], color);

    for inner_row in row + 1..row + height - 1 {
        border[..width].fill(b' ');
        border[0] = 186;
        border[width - 1] = 186;
        screen.write_at(inner_row, column, &border[..width], color);
    }
}

fn centered_text(
    screen: &mut FramebufferConsole,
    row: usize,
    column: usize,
    width: usize,
    text: &[u8],
    color: u32,
) {
    if text.len() > width || row >= HEIGHT || column >= WIDTH || width > WIDTH - column {
        return;
    }
    let text_column = column + (width - text.len()) / 2;
    screen.write_at(row, text_column, text, color);
}
