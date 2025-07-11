#![no_std]
#![cfg_attr(test, no_main)]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

#[cfg(test)]
use bootloader_api::{BootInfo, entry_point};
use conquer_once::spin::OnceCell;

use core::panic::PanicInfo;
use exit::{QemuExitCode, exit_qemu};

extern crate alloc;

pub mod allocator;
pub mod exit;
pub mod framebuffer;
pub mod gdt;
pub mod graphics;
pub mod interrupts;
pub mod memory;
pub mod process;
pub mod serial;
pub mod task;

use bootloader_api::config::{BootloaderConfig, Mapping};

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

pub static PHYSICAL_MEMORY_OFFSET: OnceCell<x86_64::VirtAddr> = OnceCell::uninit();

pub fn init(physical_memory_offset: x86_64::VirtAddr) {
    // Initialize the physical memory offset
    PHYSICAL_MEMORY_OFFSET.init_once(|| physical_memory_offset);

    serial_println!("Initializing interrupts...");
    interrupts::init_idt();
    serial_println!("Initializing GDT...");
    gdt::init();
    serial_println!("Initializing PICs...");
    unsafe { interrupts::PICS.lock().initialize() };
    serial_println!("Enabling interrupts...");

    // Disable interrupts to prevent switching to processes before they are initialized
    x86_64::instructions::interrupts::disable();
    serial_println!("Done!");
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    hlt_loop();
}

#[cfg(test)]
entry_point!(test_kernel_main, config = &BOOTLOADER_CONFIG);

/// Entry point for `cargo test`
#[cfg(test)]
fn test_kernel_main(boot_info: &'static mut BootInfo) -> ! {
    use x86_64::VirtAddr;

    init(VirtAddr::new(
        boot_info.physical_memory_offset.into_option().unwrap(),
    ));
    test_main();
    hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

async fn async_number() -> u32 {
    42
}

pub async fn example_task() {
    let number = async_number().await;
    crate::println!("async number: {}", number);
}
