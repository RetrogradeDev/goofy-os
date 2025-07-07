use x86_64::VirtAddr;

use crate::{memory::BootInfoFrameAllocator, process::run_process_async, serial_println};

pub async fn run_example_program(
    frame_allocator: &mut BootInfoFrameAllocator,
    physical_memory_offset: VirtAddr,
) -> u8 {
    serial_println!("Starting example program asynchronously...");

    let program = include_bytes!("../hello.elf");
    let exit_code = run_process_async(program, frame_allocator, physical_memory_offset).await;

    serial_println!("Example program completed with exit code: {}", exit_code);
    exit_code
}
