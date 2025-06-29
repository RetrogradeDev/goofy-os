#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(goofy_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

use goofy_os::{hlt_loop, println};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    // Initialize the OS
    goofy_os::init();

    #[cfg(test)]
    test_main();

    println!("It did not crash!");

    hlt_loop();
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
