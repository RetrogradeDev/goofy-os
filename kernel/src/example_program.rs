use x86_64::VirtAddr;

use crate::{memory::BootInfoFrameAllocator, process::ProcessManager, serial_println};

// Simple user program bytecode - makes a write syscall then exits
static USER_PROGRAM: &[u8] = &[
    // sys_write(1, "Hello from user!\n", 17)
    0x48, 0xc7, 0xc0, 0x01, 0x00, 0x00, 0x00, // mov rax, 1 (sys_write)
    0x48, 0xc7, 0xc7, 0x01, 0x00, 0x00, 0x00, // mov rdi, 1 (stdout)
    0x48, 0xc7, 0xc6, 0x10, 0x00, 0x40, 0x00, // mov rsi, 0x400010 (message address)
    0x48, 0xc7, 0xc2, 0x11, 0x00, 0x00, 0x00, // mov rdx, 17 (message length)
    0xcd, 0x80, // int 0x80 (syscall)
    // sys_exit(0)
    0x48, 0xc7, 0xc0, 0x3c, 0x00, 0x00, 0x00, // mov rax, 60 (sys_exit)
    0x48, 0xc7, 0xc7, 0x00, 0x00, 0x00, 0x00, // mov rdi, 0 (exit status)
    0xcd, 0x80, // int 0x80 (syscall)
    // Padding to reach offset 0x10
    0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90,
    // Message data at offset 0x10 (will be at virtual address 0x400010)
    b'H', b'e', b'l', b'l', b'o', b' ', b'f', b'r', b'o', b'm', b' ', b'u', b's', b'e', b'r', b'!',
    b'\n',
];

pub fn run_example_program(
    frame_allocator: &mut BootInfoFrameAllocator,
    physical_memory_offset: VirtAddr,
) {
    let mut process_manager = ProcessManager::new();

    match process_manager.create_process(USER_PROGRAM, frame_allocator, physical_memory_offset) {
        Ok(pid) => {
            serial_println!("Created process with PID: {}", pid);

            // Get the process and switch to user mode to execute it
            if let Some(process) = process_manager.get_process(pid) {
                serial_println!("Switching to user mode for process {}", pid);
                process_manager.switch_to_user_mode(process);
            } else {
                serial_println!("Failed to find process with PID: {}", pid);
            }
        }
        Err(e) => {
            serial_println!("Failed to create process: {:?}", e);
        }
    }
}
