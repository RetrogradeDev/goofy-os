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
    example_program::run_example_program,
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

    // Store the kernel page table address for interrupt handlers
    let kernel_cr3 = x86_64::registers::control::Cr3::read().0.start_address();
    kernel::interrupts::set_kernel_page_table(kernel_cr3);
    serial_println!("Kernel CR3: 0x{:x}", kernel_cr3.as_u64());

    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_regions) };

    serial_println!("Frame allocator initialized successfully!");

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    serial_println!("Heap initialized successfully!");

    println!("Hello World{}", "!");

    // run_example_program(&mut frame_allocator, phys_mem_offset);

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

    // Spawn the keyboard task first with high priority
    serial_println!("Spawning keyboard task...");
    executor.spawn(Task::new(kernel::task::keyboard::print_keypresses()));

    executor.spawn(Task::new(example_task()));

    // Add a persistent heartbeat task that never completes
    serial_println!("Spawning heartbeat task...");
    executor.spawn(Task::new(async {
        let mut counter = 0;
        loop {
            counter += 1;
            serial_println!(
                "Heartbeat #{} - system is alive, checking for keyboard input",
                counter
            );

            // Yield to other tasks
            kernel::task::yield_now().await;

            // Wait a bit before next heartbeat
            for _ in 0..500000 {
                core::hint::spin_loop();
            }
        }
    }));

    // Spawn the background scheduler to keep the process manager alive
    serial_println!("Spawning background scheduler...");
    executor.spawn(Task::new(kernel::process::process_scheduler_background())); // Spawn the example program task
    serial_println!("Spawning example program task...");
    executor.spawn(Task::new(async move {
        serial_println!("Example program task: Starting real process execution...");
        let exit_code = run_example_program(&mut frame_allocator, phys_mem_offset).await;
        serial_println!("Example program finished with exit code: {}", exit_code);

        // After the example program finishes, keep the system alive with an infinite task
        serial_println!("Example program completed, entering maintenance mode...");
        let mut maintenance_counter = 0;
        loop {
            maintenance_counter += 1;
            serial_println!(
                "Maintenance task #{} - system ready for input",
                maintenance_counter
            );
            kernel::task::yield_now().await;

            // Shorter wait to be more responsive
            for _ in 0..300000 {
                core::hint::spin_loop();
            }
        }
    }));

    serial_println!("Starting executor main loop...");
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
