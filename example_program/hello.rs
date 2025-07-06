#![no_std]
#![no_main]

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // Simple exit syscall - just exit with code 42
    unsafe {
        core::arch::asm!(
            "mov rax, 60", // syscall number for exit
            "mov rdi, 42", // exit code
            "int 0x80",    // use int 0x80 instead of syscall instruction
            options(noreturn)
        );
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
