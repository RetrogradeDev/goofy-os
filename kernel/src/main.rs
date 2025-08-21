#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

extern crate alloc;

use bootloader_api::{BootInfo, entry_point};
use kernel::sysinfo::{STACK_BASE, get_stack_pointer};
use kernel::{desktop::main::run_desktop, memory::BootInfoFrameAllocator, println, serial_println};

use bootloader_api::config::{BootloaderConfig, Mapping};
use kernel::{allocator, memory};
use x86_64::VirtAddr;
use x86_64::instructions::interrupts;

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    unsafe { STACK_BASE = get_stack_pointer() as usize };

    serial_println!("Booting goofy OS...");

    let frame = boot_info.framebuffer.as_mut().unwrap();
    kernel::framebuffer::init(frame);

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset.into_option().unwrap());

    // Initialize the OS
    kernel::init(phys_mem_offset);

    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_regions) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    // Some tests for the heap allocator
    let heap_value = alloc::boxed::Box::new(41);
    println!("heap_value at {:p}", heap_value);

    let heap_vector = alloc::vec![1, 2, 3, 4, 5];
    println!("heap_vector at {:p}", heap_vector.as_ptr());
    let heap_string = alloc::string::String::from("Hello from the heap!");
    println!("heap_string at {:p}", heap_string.as_ptr());

    #[cfg(test)]
    test_main();

    interrupts::enable();

    match kernel::fs::manager::init_filesystem() {
        Ok(_) => {
            serial_println!("Filesystem initialized successfully!");
            println!("Filesystem ready!");
        }
        Err(e) => {
            serial_println!("Failed to initialize filesystem: {}", e);
            println!("Filesystem initialization failed: {}", e);
        }
    }

    #[cfg(test)]
    test_main();

    run_desktop();
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("Panic occurred: {}", info);
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    kernel::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    kernel::test_panic_handler(info)
}
