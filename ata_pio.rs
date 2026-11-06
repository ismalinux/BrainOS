use core::arch::asm;

pub const SECTOR_SIZE: usize = 512;
const PRIMARY_IO: u16 = 0x1F0;
const PRIMARY_CONTROL: u16 = 0x3F6;
const POLL_LIMIT: usize = 1_000_000;
const LBA28_LIMIT: u32 = 1 << 28;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtaError {
    NoDevice,
    Timeout,
    DeviceFault,
    DeviceError(u8),
    LbaOutOfRange,
}

unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    unsafe {
        asm!(
            "in al, dx",
            in("dx") port,
            out("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

unsafe fn inw(port: u16) -> u16 {
    let value: u16;
    unsafe {
        asm!(
            "in ax, dx",
            in("dx") port,
            out("ax") value,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

unsafe fn outb(port: u16, value: u8) {
    unsafe {
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

fn delay_400ns() {
    for _ in 0..4 {
        let _ = unsafe { inb(PRIMARY_CONTROL) };
    }
}

fn wait_data_request() -> Result<(), AtaError> {
    for _ in 0..POLL_LIMIT {
        let status = unsafe { inb(PRIMARY_IO + 7) };
        if status == 0 || status == 0xFF {
            return Err(AtaError::NoDevice);
        }
        if status & 0x01 != 0 {
            return Err(AtaError::DeviceError(unsafe { inb(PRIMARY_IO + 1) }));
        }
        if status & 0x20 != 0 {
            return Err(AtaError::DeviceFault);
        }
        if status & 0x80 == 0 && status & 0x08 != 0 {
            return Ok(());
        }
    }
    Err(AtaError::Timeout)
}

pub fn read_sector(lba: u32) -> Result<[u8; SECTOR_SIZE], AtaError> {
    if lba >= LBA28_LIMIT {
        return Err(AtaError::LbaOutOfRange);
    }

    unsafe {
        outb(PRIMARY_IO + 6, 0xE0 | ((lba >> 24) as u8 & 0x0F));
    }
    delay_400ns();
    if unsafe { inb(PRIMARY_IO + 7) } == 0xFF {
        return Err(AtaError::NoDevice);
    }

    unsafe {
        outb(PRIMARY_IO + 1, 0);
        outb(PRIMARY_IO + 2, 1);
        outb(PRIMARY_IO + 3, lba as u8);
        outb(PRIMARY_IO + 4, (lba >> 8) as u8);
        outb(PRIMARY_IO + 5, (lba >> 16) as u8);
        outb(PRIMARY_IO + 7, 0x20);
    }

    wait_data_request()?;
    let mut sector = [0u8; SECTOR_SIZE];
    for pair in sector.chunks_exact_mut(2) {
        let word = unsafe { inw(PRIMARY_IO) }.to_le_bytes();
        pair.copy_from_slice(&word);
    }
    Ok(sector)
}
