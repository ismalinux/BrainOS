use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

const MULTIBOOT2_MEMORY_MAP_TAG: u32 = 6;
const MAX_BOOT_INFO_SIZE: usize = 1_048_576;
const IDENTITY_MAP_LIMIT: u64 = 1u64 << 32;

static USABLE_BYTES: AtomicU64 = AtomicU64::new(0);
static HIGHEST_USABLE_ADDRESS: AtomicU64 = AtomicU64::new(0);
static USABLE_REGIONS: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Copy, Debug, Default)]
pub struct MemorySummary {
    pub usable_bytes: u64,
    pub highest_usable_address: u64,
    pub usable_regions: u32,
}

impl MemorySummary {
    pub unsafe fn from_multiboot2(info_address: u32) -> Option<Self> {
        if info_address == 0 || info_address % 8 != 0 {
            return None;
        }
        let info = info_address as usize as *const u8;
        let total_size = unsafe { info.cast::<u32>().read_volatile() } as usize;
        if !(16..=MAX_BOOT_INFO_SIZE).contains(&total_size)
            || u64::from(info_address).checked_add(total_size as u64)? > IDENTITY_MAP_LIMIT
        {
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
            if tag_type == MULTIBOOT2_MEMORY_MAP_TAG {
                if tag_size < 16 {
                    return None;
                }
                let entry_size = unsafe { tag.add(8).cast::<u32>().read_volatile() } as usize;
                let entry_version = unsafe { tag.add(12).cast::<u32>().read_volatile() };
                let entry_bytes = tag_size.checked_sub(16)?;
                if entry_version != 0 || entry_size < 24 || entry_bytes % entry_size != 0 {
                    return None;
                }

                let mut summary = Self::default();
                let entries = entry_bytes / entry_size;
                for entry_index in 0..entries {
                    let entry_offset = 16usize.checked_add(entry_index.checked_mul(entry_size)?)?;
                    let entry = unsafe { tag.add(entry_offset) };
                    let base = unsafe { entry.cast::<u64>().read_volatile() };
                    let length = unsafe { entry.add(8).cast::<u64>().read_volatile() };
                    let kind = unsafe { entry.add(16).cast::<u32>().read_volatile() };
                    if kind != 1 || length == 0 {
                        continue;
                    }
                    let end = base.checked_add(length)?;
                    summary.usable_bytes = summary.usable_bytes.checked_add(length)?;
                    summary.highest_usable_address = summary.highest_usable_address.max(end);
                    summary.usable_regions = summary.usable_regions.saturating_add(1);
                }
                return Some(summary);
            }
            if tag_type == 0 {
                break;
            }
            offset = tag_end.checked_add(7)? & !7;
        }
        None
    }
}

pub fn publish(summary: Option<MemorySummary>) {
    let summary = summary.unwrap_or_default();
    USABLE_BYTES.store(summary.usable_bytes, Ordering::Release);
    HIGHEST_USABLE_ADDRESS.store(summary.highest_usable_address, Ordering::Release);
    USABLE_REGIONS.store(summary.usable_regions, Ordering::Release);
}

pub fn current() -> MemorySummary {
    MemorySummary {
        usable_bytes: USABLE_BYTES.load(Ordering::Acquire),
        highest_usable_address: HIGHEST_USABLE_ADDRESS.load(Ordering::Acquire),
        usable_regions: USABLE_REGIONS.load(Ordering::Acquire),
    }
}
