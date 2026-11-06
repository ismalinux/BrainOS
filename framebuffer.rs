use core::ptr::{write_volatile, NonNull};

pub const COLUMNS: usize = 80;
pub const ROWS: usize = 25;
pub const WIDTH: usize = COLUMNS;
pub const HEIGHT: usize = ROWS;
pub const BLUE: u32 = 0x2458D3;
pub const RED: u32 = 0xD52B3E;
pub const DESKTOP: u32 = 0x25272B;
pub const STATUS_BAR: u32 = 0xD2D4D8;
pub const TERMINAL_BACKGROUND: u32 = 0x171C24;
pub const TERMINAL_FOREGROUND: u32 = 0xF1F2F4;
pub const RECOVERY_FOREGROUND: u32 = 0xFFFFFF;

const MAX_FRAMEBUFFER_WIDTH: u32 = 4096;
const MAX_FRAMEBUFFER_HEIGHT: u32 = 2160;
const IDENTITY_MAP_LIMIT: u64 = 1u64 << 32;
const MULTIBOOT2_FRAMEBUFFER_TAG: u32 = 8;

#[derive(Clone, Copy)]
pub struct FramebufferInfo {
    address: u64,
    pitch: u32,
    width: u32,
    height: u32,
    red_position: u8,
    green_position: u8,
    blue_position: u8,
}

impl FramebufferInfo {
    pub unsafe fn from_multiboot2(info_address: u32) -> Option<Self> {
        if info_address == 0 || info_address % 8 != 0 {
            return None;
        }
        let info = info_address as usize as *const u8;
        let total_size = unsafe { info.cast::<u32>().read_volatile() } as usize;
        if !(16..=1_048_576).contains(&total_size) {
            return None;
        }
        let info_end = u64::from(info_address).checked_add(total_size as u64)?;
        if info_end > IDENTITY_MAP_LIMIT {
            return None;
        }

        let mut offset = 8usize;
        while offset.checked_add(8)? <= total_size {
            let tag = unsafe { info.add(offset) };
            let tag_type = unsafe { tag.cast::<u32>().read_volatile() };
            let tag_size = unsafe { tag.add(4).cast::<u32>().read_volatile() } as usize;
            let tag_end = offset.checked_add(tag_size)?;
            if tag_size < 8 || tag_end > total_size {
                return None;
            }
            if tag_type == MULTIBOOT2_FRAMEBUFFER_TAG {
                if tag_size < 38 {
                    return None;
                }
                let address = unsafe { tag.add(8).cast::<u64>().read_volatile() };
                let pitch = unsafe { tag.add(16).cast::<u32>().read_volatile() };
                let width = unsafe { tag.add(20).cast::<u32>().read_volatile() };
                let height = unsafe { tag.add(24).cast::<u32>().read_volatile() };
                let bpp = unsafe { tag.add(28).read_volatile() };
                let framebuffer_type = unsafe { tag.add(29).read_volatile() };
                let red_position = unsafe { tag.add(32).read_volatile() };
                let red_size = unsafe { tag.add(33).read_volatile() };
                let green_position = unsafe { tag.add(34).read_volatile() };
                let green_size = unsafe { tag.add(35).read_volatile() };
                let blue_position = unsafe { tag.add(36).read_volatile() };
                let blue_size = unsafe { tag.add(37).read_volatile() };

                if bpp != 32
                    || framebuffer_type != 1
                    || red_size != 8
                    || green_size != 8
                    || blue_size != 8
                    || !valid_component_positions(red_position, green_position, blue_position)
                    || width == 0
                    || height == 0
                    || width > MAX_FRAMEBUFFER_WIDTH
                    || height > MAX_FRAMEBUFFER_HEIGHT
                    || pitch < width.checked_mul(4)?
                    || pitch % 4 != 0
                    || address == 0
                    || address % 4 != 0
                {
                    return None;
                }

                let byte_length = u64::from(pitch).checked_mul(u64::from(height))?;
                let end = address.checked_add(byte_length)?;
                if end > IDENTITY_MAP_LIMIT {
                    return None;
                }

                return Some(Self {
                    address,
                    pitch,
                    width,
                    height,
                    red_position,
                    green_position,
                    blue_position,
                });
            }
            if tag_type == 0 {
                break;
            }
            offset = tag_end.checked_add(7)? & !7;
        }
        None
    }
}

fn valid_component_positions(red: u8, green: u8, blue: u8) -> bool {
    let distinct_fields = |left: u8, right: u8| left.abs_diff(right) >= 8;
    red <= 24
        && green <= 24
        && blue <= 24
        && distinct_fields(red, green)
        && distinct_fields(red, blue)
        && distinct_fields(green, blue)
}

#[derive(Clone, Copy)]
pub struct FramebufferConsole {
    pixels: NonNull<u32>,
    width: usize,
    height: usize,
    pitch_pixels: usize,
    red_position: u8,
    green_position: u8,
    blue_position: u8,
    cursor_row: usize,
    cursor_column: usize,
    background: u32,
}

impl FramebufferConsole {
    pub fn new(info: FramebufferInfo) -> Option<Self> {
        let address = usize::try_from(info.address).ok()?;
        if address == 0 || address % core::mem::align_of::<u32>() != 0 {
            return None;
        }
        Some(Self {
            pixels: NonNull::new(address as *mut u32)?,
            width: info.width as usize,
            height: info.height as usize,
            pitch_pixels: (info.pitch / 4) as usize,
            red_position: info.red_position,
            green_position: info.green_position,
            blue_position: info.blue_position,
            cursor_row: 0,
            cursor_column: 0,
            background: DESKTOP,
        })
    }

    fn cell_width(&self) -> usize {
        (self.width / COLUMNS).max(1)
    }

    fn cell_height(&self) -> usize {
        (self.height / ROWS).max(1)
    }

    fn pack_color(&self, rgb: u32) -> u32 {
        let red = (rgb >> 16) & 0xFF;
        let green = (rgb >> 8) & 0xFF;
        let blue = rgb & 0xFF;
        (red << self.red_position) | (green << self.green_position) | (blue << self.blue_position)
    }

    fn pixel(&mut self, x: usize, y: usize, color: u32) {
        if x >= self.width || y >= self.height {
            return;
        }
        let Some(index) = y
            .checked_mul(self.pitch_pixels)
            .and_then(|row| row.checked_add(x))
        else {
            return;
        };
        unsafe {
            write_volatile(self.pixels.as_ptr().add(index), self.pack_color(color));
        }
    }

    pub fn clear(&mut self, color: u32) {
        self.background = color;
        self.cursor_row = 0;
        self.cursor_column = 0;
        let packed = self.pack_color(color);
        for y in 0..self.height {
            let Some(row_start) = y.checked_mul(self.pitch_pixels) else {
                return;
            };
            for x in 0..self.width {
                unsafe {
                    write_volatile(self.pixels.as_ptr().add(row_start + x), packed);
                }
            }
        }
    }

    pub fn fill_cells(&mut self, row: usize, column: usize, count: usize, color: u32) {
        if row >= ROWS || column >= COLUMNS {
            return;
        }
        let cell_width = self.cell_width();
        let cell_height = self.cell_height();
        let x_start = column.saturating_mul(cell_width).min(self.width);
        let x_end = column
            .saturating_add(count)
            .min(COLUMNS)
            .saturating_mul(cell_width)
            .min(self.width);
        let y_start = row.saturating_mul(cell_height).min(self.height);
        let y_end = row
            .saturating_add(1)
            .saturating_mul(cell_height)
            .min(self.height);
        self.fill_rect(
            x_start,
            y_start,
            x_end.saturating_sub(x_start),
            y_end.saturating_sub(y_start),
            color,
        );
    }

    fn fill_rect(&mut self, x: usize, y: usize, width: usize, height: usize, color: u32) {
        let x_end = x.saturating_add(width).min(self.width);
        let y_end = y.saturating_add(height).min(self.height);
        for py in y.min(self.height)..y_end {
            for px in x.min(self.width)..x_end {
                self.pixel(px, py, color);
            }
        }
    }

    pub fn write_at(&mut self, row: usize, column: usize, text: &[u8], color: u32) {
        if row >= ROWS || column >= COLUMNS {
            return;
        }
        for (offset, &character) in text.iter().enumerate() {
            let Some(current_column) = column.checked_add(offset) else {
                break;
            };
            if current_column >= COLUMNS {
                break;
            }
            self.draw_glyph(row, current_column, character, color);
        }
    }

    fn draw_glyph(&mut self, row: usize, column: usize, character: u8, color: u32) {
        let cell_width = self.cell_width();
        let cell_height = self.cell_height();
        let x_origin = column.saturating_mul(cell_width);
        let y_origin = row.saturating_mul(cell_height);
        let scale_x = (cell_width / 6).max(1);
        let scale_y = (cell_height / 8).max(1);
        let glyph = glyph_rows(character);
        for glyph_y in 0..7 {
            for glyph_x in 0..5 {
                if glyph[glyph_y] & (1 << (4 - glyph_x)) != 0 {
                    self.fill_rect(
                        x_origin + glyph_x * scale_x,
                        y_origin + glyph_y * scale_y,
                        scale_x,
                        scale_y,
                        color,
                    );
                }
            }
        }
    }

    pub fn set_cursor(&mut self, row: usize, column: usize) {
        self.cursor_row = row.min(ROWS - 1);
        self.cursor_column = column.min(COLUMNS - 1);
    }

    pub fn put_char(&mut self, character: u8, color: u32) {
        match character {
            b'\n' => {
                self.cursor_row += 1;
                self.cursor_column = 0;
                if self.cursor_row >= ROWS {
                    self.scroll();
                }
            }
            b'\r' => self.cursor_column = 0,
            b'\t' => {
                let spaces = 4 - (self.cursor_column % 4);
                for _ in 0..spaces {
                    self.put_char(b' ', color);
                }
            }
            b'\x08' => {
                if self.cursor_column > 0 {
                    self.cursor_column -= 1;
                } else if self.cursor_row > 0 {
                    self.cursor_row -= 1;
                    self.cursor_column = COLUMNS - 1;
                }
                self.fill_cell(self.cursor_row, self.cursor_column, self.background);
            }
            _ => {
                self.draw_glyph(self.cursor_row, self.cursor_column, character, color);
                self.cursor_column += 1;
                if self.cursor_column >= COLUMNS {
                    self.cursor_column = 0;
                    self.cursor_row += 1;
                    if self.cursor_row >= ROWS {
                        self.scroll();
                    }
                }
            }
        }
    }

    fn fill_cell(&mut self, row: usize, column: usize, color: u32) {
        self.fill_rect(
            column * self.cell_width(),
            row * self.cell_height(),
            self.cell_width(),
            self.cell_height(),
            color,
        );
    }

    fn scroll(&mut self) {
        let cell_height = self.cell_height().min(self.height);
        if cell_height == 0 {
            return;
        }
        let copy_rows = self.height - cell_height;
        for y in 0..copy_rows {
            for x in 0..self.width {
                let source_index = (y + cell_height) * self.pitch_pixels + x;
                let destination_index = y * self.pitch_pixels + x;
                let pixel = unsafe { self.pixels.as_ptr().add(source_index).read_volatile() };
                unsafe {
                    write_volatile(self.pixels.as_ptr().add(destination_index), pixel);
                }
            }
        }
        self.fill_rect(0, copy_rows, self.width, cell_height, self.background);
        self.cursor_row = ROWS - 1;
    }

    pub fn write(&mut self, text: &[u8], color: u32) {
        for &character in text {
            self.put_char(character, color);
        }
    }
}

fn glyph_rows(character: u8) -> [u8; 7] {
    match character.to_ascii_uppercase() {
        b'A' => [14, 17, 17, 31, 17, 17, 17],
        b'B' => [30, 17, 17, 30, 17, 17, 30],
        b'C' => [14, 17, 16, 16, 16, 17, 14],
        b'D' => [30, 17, 17, 17, 17, 17, 30],
        b'E' => [31, 16, 16, 30, 16, 16, 31],
        b'F' => [31, 16, 16, 30, 16, 16, 16],
        b'G' => [14, 17, 16, 23, 17, 17, 15],
        b'H' => [17, 17, 17, 31, 17, 17, 17],
        b'I' => [14, 4, 4, 4, 4, 4, 14],
        b'J' => [7, 2, 2, 2, 18, 18, 12],
        b'K' => [17, 18, 20, 24, 20, 18, 17],
        b'L' => [16, 16, 16, 16, 16, 16, 31],
        b'M' => [17, 27, 21, 21, 17, 17, 17],
        b'N' => [17, 25, 21, 19, 17, 17, 17],
        b'O' => [14, 17, 17, 17, 17, 17, 14],
        b'P' => [30, 17, 17, 30, 16, 16, 16],
        b'Q' => [14, 17, 17, 17, 21, 18, 13],
        b'R' => [30, 17, 17, 30, 20, 18, 17],
        b'S' => [15, 16, 16, 14, 1, 1, 30],
        b'T' => [31, 4, 4, 4, 4, 4, 4],
        b'U' => [17, 17, 17, 17, 17, 17, 14],
        b'V' => [17, 17, 17, 17, 17, 10, 4],
        b'W' => [17, 17, 17, 21, 21, 21, 10],
        b'X' => [17, 17, 10, 4, 10, 17, 17],
        b'Y' => [17, 17, 10, 4, 4, 4, 4],
        b'Z' => [31, 1, 2, 4, 8, 16, 31],
        b'0' => [14, 17, 19, 21, 25, 17, 14],
        b'1' => [4, 12, 4, 4, 4, 4, 14],
        b'2' => [14, 17, 1, 2, 4, 8, 31],
        b'3' => [30, 1, 1, 14, 1, 1, 30],
        b'4' => [2, 6, 10, 18, 31, 2, 2],
        b'5' => [31, 16, 16, 30, 1, 1, 30],
        b'6' => [14, 16, 16, 30, 17, 17, 14],
        b'7' => [31, 1, 2, 4, 8, 8, 8],
        b'8' => [14, 17, 17, 14, 17, 17, 14],
        b'9' => [14, 17, 17, 15, 1, 1, 14],
        b'.' => [0, 0, 0, 0, 0, 12, 12],
        b',' => [0, 0, 0, 0, 4, 4, 8],
        b':' => [0, 12, 12, 0, 12, 12, 0],
        b';' => [0, 12, 12, 0, 4, 4, 8],
        b'!' => [4, 4, 4, 4, 4, 0, 4],
        b'?' => [14, 17, 1, 2, 4, 0, 4],
        b'-' => [0, 0, 0, 31, 0, 0, 0],
        b'_' => [0, 0, 0, 0, 0, 0, 31],
        b'/' => [1, 2, 2, 4, 8, 8, 16],
        b'\\' => [16, 8, 8, 4, 2, 2, 1],
        b'(' => [2, 4, 8, 8, 8, 4, 2],
        b')' => [8, 4, 2, 2, 2, 4, 8],
        b'[' => [14, 8, 8, 8, 8, 8, 14],
        b']' => [14, 2, 2, 2, 2, 2, 14],
        b'<' => [2, 4, 8, 16, 8, 4, 2],
        b'>' => [8, 4, 2, 1, 2, 4, 8],
        b'=' => [0, 0, 31, 0, 31, 0, 0],
        b'+' => [0, 4, 4, 31, 4, 4, 0],
        b'%' => [17, 2, 4, 8, 16, 17, 0],
        b'|' | 186 => [4, 4, 4, 4, 4, 4, 4],
        205 => [0, 0, 31, 0, 0, 31, 0],
        201 | 187 | 200 | 188 => [0, 0, 31, 16, 16, 16, 16],
        b' ' => [0; 7],
        _ => [14, 17, 1, 2, 4, 0, 4],
    }
}
