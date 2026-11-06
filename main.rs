#![no_std]

mod ata_pio;
mod cmos;
mod cosmic_desktop;
mod cosmic_editor;
mod cosmic_files;
mod cosmic_settings;
mod cosmic_store;
mod framebuffer;
mod input;
mod memory_map;
mod recovery_ui;
#[path = "../kernel/runtime.rs"]
#[allow(dead_code)]
mod runtime;
mod shell;
mod trmnl;

use core::arch::asm;
use core::ptr::{addr_of, addr_of_mut, read_volatile, write_volatile};
use core::sync::atomic::{AtomicBool, Ordering};
use framebuffer::{FramebufferConsole, FramebufferInfo};

const MULTIBOOT2_BOOTLOADER_MAGIC: u32 = 0x36D76289;

static SYSTEM_CRASH_FLAG: AtomicBool = AtomicBool::new(false);
static SECURITY_BREACH_FLAG: AtomicBool = AtomicBool::new(false);
static mut ACTIVE_FRAMEBUFFER: Option<FramebufferInfo> = None;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootStatus {
    Normal,
    SystemCrash,
    SecurityBreach,
}

#[no_mangle]
pub extern "C" fn brainos_report_system_crash() {
    SYSTEM_CRASH_FLAG.store(true, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn brainos_report_hardware_error() {
    SYSTEM_CRASH_FLAG.store(true, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn brainos_report_security_breach() {
    SECURITY_BREACH_FLAG.store(true, Ordering::SeqCst);
}

fn detect_boot_status(multiboot_magic: u32, boot_info: u32) -> BootStatus {
    if SECURITY_BREACH_FLAG.load(Ordering::SeqCst) {
        BootStatus::SecurityBreach
    } else if SYSTEM_CRASH_FLAG.load(Ordering::SeqCst)
        || multiboot_magic != MULTIBOOT2_BOOTLOADER_MAGIC
        || boot_info == 0
        || boot_info % 8 != 0
    {
        BootStatus::SystemCrash
    } else {
        BootStatus::Normal
    }
}

#[no_mangle]
pub extern "C" fn kernel_main(multiboot_magic: u32, boot_info: u32) -> ! {
    let Some(framebuffer_info) = (unsafe { FramebufferInfo::from_multiboot2(boot_info) }) else {
        halt();
    };
    let memory_summary = unsafe { memory_map::MemorySummary::from_multiboot2(boot_info) };
    memory_map::publish(memory_summary);
    unsafe {
        write_volatile(addr_of_mut!(ACTIVE_FRAMEBUFFER), Some(framebuffer_info));
    }
    let Some(screen) = FramebufferConsole::new(framebuffer_info) else {
        halt();
    };

    match detect_boot_status(multiboot_magic, boot_info) {
        BootStatus::Normal => cosmic_desktop::run(screen),
        status @ (BootStatus::SystemCrash | BootStatus::SecurityBreach) => {
            recovery_ui::run(screen, status)
        }
    }
}

fn halt() -> ! {
    loop {
        unsafe {
            asm!("cli; hlt", options(nomem, nostack));
        }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    let framebuffer_info = unsafe { read_volatile(addr_of!(ACTIVE_FRAMEBUFFER)) };
    match framebuffer_info.and_then(FramebufferConsole::new) {
        Some(screen) => recovery_ui::run(screen, BootStatus::SystemCrash),
        None => halt(),
    }
}
