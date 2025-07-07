#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

extern crate alloc;

use bootloader_api::{BootInfo, entry_point};
use kernel::{
    // framebuffer::clear_screen,
    framebuffer::{FRAMEBUFFER, FrameBufferWriter},
    graphics::{
        draw_circle, draw_circle_outline, draw_line, draw_rect, draw_rect_outline, set_pixel,
    },
    memory::BootInfoFrameAllocator,
    println,
    serial_println,
    task::{Task, executor::Executor},
};

use bootloader_api::config::{BootloaderConfig, Mapping};

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

async fn async_number() -> u32 {
    42
}

async fn example_task() {
    let number = async_number().await;
    println!("async number: {}", number);
}

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    use kernel::{allocator, memory};
    use x86_64::VirtAddr;

    serial_println!("Booting goofy OS...");
    serial_println!("Bootloader information: {:#?}", boot_info);

    FRAMEBUFFER.init_once(|| {
        let frame = boot_info.framebuffer.as_mut();
        let info = match frame {
            Some(ref v) => v.info(),
            None => panic!("BOOTLOADER NOT CONFIGURED TO SUPPORT FRAMEBUFFER"),
        };
        let buffer = match frame {
            Some(v) => v.buffer_mut(),
            None => panic!("BOOTLOADER NOT CONFIGURED TO SUPPORT FRAMEBUFFER"),
        };
        spinning_top::Spinlock::new(FrameBufferWriter::new(buffer, info))
    });

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

    // Queue the example program instead of running it directly
    // This allows the kernel to continue while the process runs via timer scheduling
    match kernel::process::queue_example_program(&mut frame_allocator, phys_mem_offset) {
        Ok(pid) => serial_println!("Successfully queued process with PID: {}", pid),
        Err(e) => serial_println!("Failed to queue process: {:?}", e),
    }

    // Continue with the rest of kernel initialization - this will now run concurrently with processes

    // Some tests for the heap allocator
    let heap_value = alloc::boxed::Box::new(41);
    println!("heap_value at {:p}", heap_value);

    let heap_vector = alloc::vec![1, 2, 3, 4, 5];
    println!("heap_vector at {:p}", heap_vector.as_ptr());
    let heap_string = alloc::string::String::from("Hello from the heap!");
    println!("heap_string at {:p}", heap_string.as_ptr());

    #[cfg(test)]
    test_main();

    let mut executor = Executor::new();
    executor.spawn(Task::new(example_task()));
    executor.spawn(Task::new(kernel::task::keyboard::print_keypresses()));
    executor.run();
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
