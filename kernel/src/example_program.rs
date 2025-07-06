use x86_64::VirtAddr;

use crate::{
    memory::BootInfoFrameAllocator,
    process::{PROCESS_MANAGER, switch_to_user_mode},
    serial_println,
};

pub fn run_example_program(
    frame_allocator: &mut BootInfoFrameAllocator,
    physical_memory_offset: VirtAddr,
) {
    let pid = {
        let mut process_manager = PROCESS_MANAGER.lock();
        let program = include_bytes!("../hello.elf"); // Assuming the binary is included in the build

        match process_manager.create_process(program, frame_allocator, physical_memory_offset) {
            Ok(pid) => {
                serial_println!("Created process with PID: {}", pid);
                pid
            }
            Err(e) => {
                serial_println!("Failed to create process: {:?}", e);
                return;
            }
        }
    }; // Lock is released here

    // Now get the process and switch to user mode
    let process = {
        let process_manager = PROCESS_MANAGER.lock();
        process_manager.get_process(pid).unwrap().clone()
    };

    switch_to_user_mode(&process, physical_memory_offset);
}
