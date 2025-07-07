#![no_std]
#![no_main]

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // Very simple process that just exits immediately

    // Exit with status 0
    unsafe {
        core::arch::asm!(
            "mov rax, 60", // syscall number for exit
            "mov rdi, 0",  // exit status
            "int 0x80",    // system call
            options(noreturn)
        );
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
