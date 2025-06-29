#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(goofy_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

extern crate alloc;

use bootloader::{BootInfo, entry_point};
use goofy_os::{
    graphics::clear_screen,
    memory::BootInfoFrameAllocator,
    println, serial_println,
    task::{Task, executor::Executor},
};

async fn async_number() -> u32 {
    42
}

async fn example_task() {
    let number = async_number().await;
    println!("async number: {}", number);
}

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use goofy_os::{allocator, memory};
    use x86_64::VirtAddr;

    clear_screen(goofy_os::graphics::Color::Black);

    serial_println!("Booting goofy OS...");
    println!("Hello World{}", "!");

    // Initialize the OS
    goofy_os::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    #[cfg(test)]
    test_main();

    let mut executor = Executor::new();
    executor.spawn(Task::new(example_task()));
    executor.spawn(Task::new(goofy_os::task::keyboard::print_keypresses()));
    executor.run();
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("Panic occurred: {}", info);
    goofy_os::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    goofy_os::test_panic_handler(info)
}
