use core::arch::asm;

const CMOS_INDEX_PORT: u16 = 0x70;
const CMOS_DATA_PORT: u16 = 0x71;
const RTC_STATUS_A: u8 = 0x0A;
const RTC_STATUS_B: u8 = 0x0B;
const RTC_SECONDS: u8 = 0x00;
const RTC_MINUTES: u8 = 0x02;
const RTC_HOURS: u8 = 0x04;
const RTC_DAY: u8 = 0x07;
const RTC_MONTH: u8 = 0x08;
const RTC_YEAR: u8 = 0x09;
const RTC_CENTURY: u8 = 0x32;
const CMOS_POLL_LIMIT: usize = 100_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CmosError {
    UpdateTimeout,
    InvalidBcd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

unsafe fn read_register(register: u8) -> u8 {
    unsafe {
        asm!(
            "out dx, al",
            in("dx") CMOS_INDEX_PORT,
            in("al") register & 0x7F,
            options(nomem, nostack, preserves_flags)
        );
    }
    let value: u8;
    unsafe {
        asm!(
            "in al, dx",
            in("dx") CMOS_DATA_PORT,
            out("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

fn wait_for_update() -> Result<(), CmosError> {
    for _ in 0..CMOS_POLL_LIMIT {
        if unsafe { read_register(RTC_STATUS_A) } & 0x80 == 0 {
            return Ok(());
        }
    }
    Err(CmosError::UpdateTimeout)
}

fn decode(value: u8, binary_mode: bool) -> Result<u8, CmosError> {
    if binary_mode {
        return Ok(value);
    }
    let low = value & 0x0F;
    let high = value >> 4;
    if low > 9 || high > 9 {
        return Err(CmosError::InvalidBcd);
    }
    Ok(high * 10 + low)
}

#[derive(Clone, Copy)]
struct Sample {
    second: u8,
    minute: u8,
    hour: u8,
    day: u8,
    month: u8,
    year: u8,
    century: u8,
    status_b: u8,
}

fn sample() -> Result<Sample, CmosError> {
    wait_for_update()?;
    Ok(unsafe {
        Sample {
            second: read_register(RTC_SECONDS),
            minute: read_register(RTC_MINUTES),
            hour: read_register(RTC_HOURS),
            day: read_register(RTC_DAY),
            month: read_register(RTC_MONTH),
            year: read_register(RTC_YEAR),
            century: read_register(RTC_CENTURY),
            status_b: read_register(RTC_STATUS_B),
        }
    })
}

pub fn read_datetime() -> Result<DateTime, CmosError> {
    let first = sample()?;
    let second = sample()?;
    if first.second != second.second
        || first.minute != second.minute
        || first.hour != second.hour
        || first.day != second.day
        || first.month != second.month
        || first.year != second.year
    {
        return Err(CmosError::UpdateTimeout);
    }

    let binary_mode = second.status_b & 0x04 != 0;
    let is_24_hour = second.status_b & 0x02 != 0;
    let pm = second.hour & 0x80 != 0;
    let raw_hour = second.hour & 0x7F;
    let mut hour = decode(raw_hour, binary_mode)?;
    if !is_24_hour {
        hour %= 12;
        if pm {
            hour += 12;
        }
    }

    let short_year = decode(second.year, binary_mode)?;
    let century = decode(second.century, binary_mode).unwrap_or(0);
    let year = if century != 0 {
        u16::from(century) * 100 + u16::from(short_year)
    } else if short_year >= 70 {
        1900 + u16::from(short_year)
    } else {
        2000 + u16::from(short_year)
    };

    Ok(DateTime {
        year,
        month: decode(second.month, binary_mode)?,
        day: decode(second.day, binary_mode)?,
        hour,
        minute: decode(second.minute, binary_mode)?,
        second: decode(second.second, binary_mode)?,
    })
}

pub fn read_legacy_config(index: u8) -> Option<u8> {
    if !(0x10..=0x3F).contains(&index) {
        return None;
    }
    Some(unsafe { read_register(index) })
}
