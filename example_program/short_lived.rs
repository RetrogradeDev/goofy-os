#![no_std]
#![no_main]

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // Short-lived process that does some work and exits

    // Do some work for a limited time
    for iteration in 0..5u32 {
        // Make a write syscall to show we're working
        unsafe {
            core::arch::asm!(
                "mov rax, 1",       // syscall number for write
                "mov rdi, 1",       // stdout
                "mov rsi, {}",      // buffer pointer (iteration number)
                "mov rdx, 4",       // count
                "int 0x80",         // system call
                in(reg) &iteration as *const u32 as u64,
                out("rax") _,
                options(nostack)
            );
        }

        // Do some computation work
        let mut work_result = 0u32;
        for i in 0..500000 {
            work_result = work_result.wrapping_add(i as u32);
            // Prevent optimization
            unsafe {
                core::arch::asm!("nop", options(nomem, nostack));
            }
        }

        // Small delay between iterations
        for i in 0..200000 {
            unsafe {
                core::arch::asm!("nop", options(nomem, nostack));
            }
        }
    }

    // Exit after finishing work
    unsafe {
        core::arch::asm!(
            "mov rax, 60", // syscall number for exit
            "mov rdi, 99", // exit code (different from the other process)
            "int 0x80",    // system call
            options(noreturn)
        );
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
