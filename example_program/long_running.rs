#![no_std]
#![no_main]

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // Long-running process that prints messages periodically
    let mut counter = 0u32;

    loop {
        // Simple delay loop - count to a large number
        for i in 0..1000000 {
            // Prevent optimization from removing this loop
            unsafe {
                core::arch::asm!("nop", options(nomem, nostack));
            }
        }

        // Make a write syscall to log a message
        // For simplicity, we'll just use the syscall number and some dummy data
        unsafe {
            core::arch::asm!(
                "mov rax, 1",       // syscall number for write
                "mov rdi, 1",       // stdout
                "mov rsi, {}",      // buffer pointer (we'll use counter as dummy data)
                "mov rdx, 8",       // count
                "int 0x80",         // system call
                in(reg) &counter as *const u32 as u64,
                out("rax") _,
                options(nostack)
            );
        }

        counter = counter.wrapping_add(1);

        // Every 10 iterations, yield to other processes
        if counter % 10 == 0 {
            // Small delay to allow scheduling
            for i in 0..100000 {
                unsafe {
                    core::arch::asm!("nop", options(nomem, nostack));
                }
            }
        }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
