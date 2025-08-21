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
    serial_println!("Bootloader information: {:#?}", boot_info);

    serial_println!("Initializing framebuffer");
    let frame = boot_info.framebuffer.as_mut().unwrap();
    kernel::framebuffer::init(frame);

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset.into_option().unwrap());

    // Initialize the OS
    kernel::init(phys_mem_offset);

    serial_println!("Kernel initialized, setting up memory...");
    println!("Kernel initialized successfully!");

    serial_println!("Physical memory offset: {:?}", phys_mem_offset);

    let mut mapper = unsafe { memory::init(phys_mem_offset) };

    serial_println!("Memory mapper initialized successfully!");

    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_regions) };

    serial_println!("Frame allocator initialized successfully!");

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    serial_println!("Heap initialized successfully!");

    println!("Hello World{}", "!");

    // Some tests for the heap allocator
    let heap_value = alloc::boxed::Box::new(41);
    println!("heap_value at {:p}", heap_value);

    let heap_vector = alloc::vec![1, 2, 3, 4, 5];
    println!("heap_vector at {:p}", heap_vector.as_ptr());
    let heap_string = alloc::string::String::from("Hello from the heap!");
    println!("heap_string at {:p}", heap_string.as_ptr());

    serial_println!("Heap tests completed successfully!");

    #[cfg(test)]
    test_main();

    serial_println!("Current UTC time: {:#?}", kernel::time::get_utc_time());
    serial_println!(
        "Milliseconds since epoch: {}",
        kernel::time::get_ms_since_epoch()
    );

    // Initialize the filesystem after interrupts are enabled
    serial_println!("Enabling interrupts...");
    interrupts::enable();
    serial_println!("Interrupts enabled successfully!");

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

    match kernel::fs::manager::list_root_files() {
        Ok(entries) => {
            serial_println!("Root directory files:");
            for entry in entries {
                serial_println!(" - {}", entry.name);
            }
        }
        Err(e) => {
            serial_println!("Failed to list root directory files: {}", e);
        }
    }

    // Test filesystem write operations
    serial_println!("Testing filesystem write operations...");

    // Create a test file
    let test_content =
        "Hello from the FAT32 filesystem!\nThis is a test file created by goofy OS.\n";
    match kernel::fs::manager::create_text_file_in_root("TEST.TXT", test_content) {
        Ok(_) => {
            serial_println!("Successfully created TEST.TXT");
            println!("Created test file: TEST.TXT");

            // Try to read it back
            match kernel::fs::manager::find_file_in_root("TEST.TXT") {
                Ok(Some(file)) => {
                    match kernel::fs::manager::read_text_file(file.first_cluster, file.size) {
                        Ok(content) => {
                            serial_println!("File content read back: {}", content);
                        }
                        Err(e) => {
                            serial_println!("Failed to read back file content: {}", e);
                        }
                    }
                }
                Ok(None) => {
                    serial_println!("File not found after creation");
                }
                Err(e) => {
                    serial_println!("Error finding created file: {}", e);
                }
            }
        }
        Err(e) => {
            serial_println!("Failed to create test file: {}", e);
        }
    }

    #[cfg(test)]
    test_main();

    serial_println!("Current UTC time: {:#?}", kernel::time::get_utc_time());
    serial_println!(
        "Milliseconds since epoch: {}",
        kernel::time::get_ms_since_epoch()
    );

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
