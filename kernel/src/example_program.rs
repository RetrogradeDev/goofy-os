use x86_64::VirtAddr;

use crate::{memory::BootInfoFrameAllocator, process::ProcessManager, serial_println};

pub fn run_example_program(
    frame_allocator: &mut BootInfoFrameAllocator,
    physical_memory_offset: VirtAddr,
) {
    let mut process_manager = ProcessManager::new();

    let program = include_bytes!("../hello.elf"); // Assuming the binary is included in the build

    match process_manager.create_process(program, frame_allocator, physical_memory_offset) {
        Ok(pid) => {
            serial_println!("Created process with PID: {}", pid);

            // Get the process and examine it, but don't switch to user mode yet
            if let Some(process) = process_manager.get_process(pid) {
                serial_println!("Process {} ready to run", pid);
                serial_println!("Entry point: {:?}", process.instruction_pointer);
                serial_println!("Stack pointer: {:?}", process.stack_pointer);

                // Let's examine the program data at the entry point
                serial_println!("Examining program code...");

                process_manager.switch_to_user_mode(process, physical_memory_offset);
            } else {
                serial_println!("Failed to find process with PID: {}", pid);
            }
        }
        Err(e) => {
            serial_println!("Failed to create process: {:?}", e);
        }
    }
}
