#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

extern crate alloc;

use bootloader_api::{BootInfo, entry_point};
use kernel::{
    graphics::{
        draw_circle, draw_circle_outline, draw_line, draw_rect, draw_rect_outline, set_pixel,
    },
    memory::BootInfoFrameAllocator,
    println, serial_println,
};

use bootloader_api::config::{BootloaderConfig, Mapping};
use x86_64::instructions::interrupts;

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    use kernel::{allocator, memory};
    use x86_64::VirtAddr;

    serial_println!("Booting goofy OS...");
    serial_println!("Bootloader information: {:#?}", boot_info);

    serial_println!("Initializing framebuffer");
    let frame = boot_info.framebuffer.as_mut().unwrap();
    kernel::framebuffer::init(frame);

    set_pixel(10, 10, kernel::framebuffer::Color::new(255, 0, 0));
    draw_line(
        (100, 100),
        (50, 50),
        kernel::framebuffer::Color::new(0, 255, 0),
    );
    draw_rect(
        (200, 200),
        (300, 300),
        kernel::framebuffer::Color::new(0, 0, 255),
    );
    draw_rect_outline(
        (400, 400),
        (500, 500),
        kernel::framebuffer::Color::new(255, 255, 0),
    );
    draw_circle((600, 600), 50, kernel::framebuffer::Color::new(255, 0, 255));
    draw_circle_outline((700, 600), 75, kernel::framebuffer::Color::new(0, 255, 255));

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

    // Initialize the global executor
    kernel::task::executor::init_global_executor();

    // Create the executor as a kernel process FIRST
    {
        use kernel::process::PROCESS_MANAGER;
        use x86_64::VirtAddr;

        let mut pm = PROCESS_MANAGER.lock();
        let executor_entry = kernel::task::executor::get_executor_entry_point();
        let executor_entry_addr = VirtAddr::new(executor_entry as *const () as u64);

        // Allocate a proper kernel stack
        const KERNEL_STACK_SIZE: usize = 4096 * 4; // 16KB stack
        static mut KERNEL_STACK: [u8; KERNEL_STACK_SIZE] = [0; KERNEL_STACK_SIZE];

        let kernel_stack = VirtAddr::from_ptr(&raw const KERNEL_STACK) + KERNEL_STACK_SIZE as u64;

        match pm.create_kernel_process(executor_entry_addr, kernel_stack) {
            Ok(pid) => {
                serial_println!("Created executor kernel process with PID: {}", pid);
            }
            Err(e) => serial_println!("Failed to create executor kernel process: {:?}", e),
        }

        // Now release the lock before proceeding to the next step
    }

    // Queue the example program instead of running it directly
    // This allows the kernel to continue while the process runs via timer scheduling
    match kernel::process::queue_example_program(&mut frame_allocator, phys_mem_offset) {
        Ok(pid) => serial_println!("Successfully queued process with PID: {}", pid),
        Err(e) => serial_println!("Failed to queue process: {:?}", e),
    }

    // Some tests for the heap allocator
    let heap_value = alloc::boxed::Box::new(41);
    println!("heap_value at {:p}", heap_value);

    let heap_vector = alloc::vec![1, 2, 3, 4, 5];
    println!("heap_vector at {:p}", heap_vector.as_ptr());
    let heap_string = alloc::string::String::from("Hello from the heap!");
    println!("heap_string at {:p}", heap_string.as_ptr());

    #[cfg(test)]
    test_main();

    {
        use kernel::process::PROCESS_MANAGER;

        let mut pm = PROCESS_MANAGER.lock();
        let pid = 1;

        pm.set_current_pid(pid);
    }

    // The kernel should continue running and let the scheduler handle task execution
    interrupts::enable();
    kernel::hlt_loop();
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
