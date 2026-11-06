use core::arch::asm;
use core::sync::atomic::{AtomicBool, Ordering};

static LEFT_SHIFT: AtomicBool = AtomicBool::new(false);
static RIGHT_SHIFT: AtomicBool = AtomicBool::new(false);
static CAPS_LOCK: AtomicBool = AtomicBool::new(false);
static CAPS_LOCK_DOWN: AtomicBool = AtomicBool::new(false);
static EXTENDED: AtomicBool = AtomicBool::new(false);

unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    asm!(
        "in al, dx",
        in("dx") port,
        out("al") value,
        options(nomem, nostack, preserves_flags)
    );
    value
}

pub fn read_scancode() -> u8 {
    loop {
        if unsafe { inb(0x64) } & 1 == 0 {
            continue;
        }
        let raw = unsafe { inb(0x60) };
        if raw == 0xE0 {
            EXTENDED.store(true, Ordering::Relaxed);
            continue;
        }
        if EXTENDED.swap(false, Ordering::Relaxed) {
            continue;
        }

        let released = raw & 0x80 != 0;
        let code = raw & 0x7F;
        match code {
            0x2A => {
                LEFT_SHIFT.store(!released, Ordering::Relaxed);
                continue;
            }
            0x36 => {
                RIGHT_SHIFT.store(!released, Ordering::Relaxed);
                continue;
            }
            0x3A => {
                if !released && !CAPS_LOCK_DOWN.swap(true, Ordering::Relaxed) {
                    CAPS_LOCK.fetch_xor(true, Ordering::Relaxed);
                } else if released {
                    CAPS_LOCK_DOWN.store(false, Ordering::Relaxed);
                }
                continue;
            }
            _ if released => continue,
            _ => return code,
        }
    }
}

pub fn read_char() -> u8 {
    loop {
        let code = read_scancode();
        let character = match code {
            0x02 => shifted(b'1', b'!'),
            0x03 => shifted(b'2', b'@'),
            0x04 => shifted(b'3', b'#'),
            0x05 => shifted(b'4', b'$'),
            0x06 => shifted(b'5', b'%'),
            0x07 => shifted(b'6', b'^'),
            0x08 => shifted(b'7', b'&'),
            0x09 => shifted(b'8', b'*'),
            0x0A => shifted(b'9', b'('),
            0x0B => shifted(b'0', b')'),
            0x0C => shifted(b'-', b'_'),
            0x0D => shifted(b'=', b'+'),
            0x10 => letter(b'q'),
            0x11 => letter(b'w'),
            0x12 => letter(b'e'),
            0x13 => letter(b'r'),
            0x14 => letter(b't'),
            0x15 => letter(b'y'),
            0x16 => letter(b'u'),
            0x17 => letter(b'i'),
            0x18 => letter(b'o'),
            0x19 => letter(b'p'),
            0x1A => shifted(b'[', b'{'),
            0x1B => shifted(b']', b'}'),
            0x1E => letter(b'a'),
            0x1F => letter(b's'),
            0x20 => letter(b'd'),
            0x21 => letter(b'f'),
            0x22 => letter(b'g'),
            0x23 => letter(b'h'),
            0x24 => letter(b'j'),
            0x25 => letter(b'k'),
            0x26 => letter(b'l'),
            0x27 => shifted(b';', b':'),
            0x28 => shifted(b'\'', b'"'),
            0x29 => shifted(b'`', b'~'),
            0x2B => shifted(b'\\', b'|'),
            0x2C => letter(b'z'),
            0x2D => letter(b'x'),
            0x2E => letter(b'c'),
            0x2F => letter(b'v'),
            0x30 => letter(b'b'),
            0x31 => letter(b'n'),
            0x32 => letter(b'm'),
            0x33 => shifted(b',', b'<'),
            0x34 => shifted(b'.', b'>'),
            0x35 => shifted(b'/', b'?'),
            0x39 => b' ',
            0x1C => b'\n',
            0x0E => b'\x08',
            0x01 => b'\x1B',
            _ => 0,
        };
        if character != 0 {
            return character;
        }
    }
}

fn is_shifted() -> bool {
    LEFT_SHIFT.load(Ordering::Relaxed) || RIGHT_SHIFT.load(Ordering::Relaxed)
}

fn letter(lower: u8) -> u8 {
    if is_shifted() ^ CAPS_LOCK.load(Ordering::Relaxed) {
        lower.to_ascii_uppercase()
    } else {
        lower
    }
}

fn shifted(normal: u8, with_shift: u8) -> u8 {
    if is_shifted() {
        with_shift
    } else {
        normal
    }
}
