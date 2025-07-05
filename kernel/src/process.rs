use core::arch::asm;

use alloc::vec::Vec;
use x86_64::{
    VirtAddr,
    structures::paging::{FrameAllocator, PageTableFlags},
};

use crate::{
    memory::{BootInfoFrameAllocator, ProcessAddressSpace},
    serial_println,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Ready,
    Running,
    Blocked,
    Terminated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessError {
    OutOfMemory,
    InvalidProgram,
    InvalidStateTransition,
    InvalidInstructionPointer,
    InvalidStackPointer,
}

pub struct Process {
    pub pid: u32,
    pub state: ProcessState,
    pub address_space: ProcessAddressSpace,
    pub stack_pointer: VirtAddr,
    pub instruction_pointer: VirtAddr,
    // Saved register state
    pub registers: RegisterState,
}

#[repr(C)]
pub struct RegisterState {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rflags: u64,
    pub rip: u64,
}

pub struct ProcessManager {
    processes: Vec<Process>,
    #[allow(dead_code)] // Will be used for process scheduling
    current_pid: u32,
    next_pid: u32,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            processes: Vec::new(),
            current_pid: 0,
            next_pid: 1,
        }
    }

    pub fn create_process(
        &mut self,
        program_data: &[u8],
        frame_allocator: &mut BootInfoFrameAllocator,
        physical_memory_offset: VirtAddr,
    ) -> Result<u32, ProcessError> {
        serial_println!(
            "Creating process with program data of {} bytes",
            program_data.len()
        );

        // Create the address space first
        let mut address_space = ProcessAddressSpace::new(frame_allocator, physical_memory_offset)
            .map_err(|_| ProcessError::OutOfMemory)?;

        // Allocate a frame for the program code
        let program_frame = frame_allocator
            .allocate_frame()
            .ok_or(ProcessError::OutOfMemory)?;

        // Allocate a frame for the stack
        let stack_frame = frame_allocator
            .allocate_frame()
            .ok_or(ProcessError::OutOfMemory)?;

        // Map program code at 0x400000 (typical user program start)
        let program_virtual_addr = VirtAddr::new(0x400000);
        address_space
            .map_user_memory(
                program_virtual_addr,
                program_frame.start_address(),
                0x1000, // 4KB page
                PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE,
                frame_allocator,
            )
            .map_err(|_| ProcessError::OutOfMemory)?;

        // Map stack at 0x800000 (8MB mark)
        let stack_virtual_addr = VirtAddr::new(0x800000);
        address_space
            .map_user_memory(
                stack_virtual_addr,
                stack_frame.start_address(),
                0x1000, // 4KB stack
                PageTableFlags::PRESENT
                    | PageTableFlags::WRITABLE
                    | PageTableFlags::USER_ACCESSIBLE,
                frame_allocator,
            )
            .map_err(|_| ProcessError::OutOfMemory)?;

        // Copy program data to the mapped memory through virtual memory
        //TODO: ELF
        let program_virtual_ptr =
            (physical_memory_offset + program_frame.start_address().as_u64()).as_mut_ptr::<u8>();
        unsafe {
            core::ptr::copy_nonoverlapping(
                program_data.as_ptr(),
                program_virtual_ptr,
                program_data.len().min(4096),
            );
        }

        let stack_pointer = stack_virtual_addr + 0x1000; // Stack grows downward
        let instruction_pointer = program_virtual_addr; // Start at beginning of program

        serial_println!("Setting up process with PID {}", self.next_pid);

        let process = Process {
            pid: self.next_pid,
            state: ProcessState::Ready,
            address_space,
            stack_pointer,
            instruction_pointer,
            registers: RegisterState {
                rax: 0,
                rbx: 0,
                rcx: 0,
                rdx: 0,
                rsi: 0,
                rdi: 0,
                rbp: 0,
                rsp: stack_pointer.as_u64(),
                r8: 0,
                r9: 0,
                r10: 0,
                r11: 0,
                r12: 0,
                r13: 0,
                r14: 0,
                r15: 0,
                rflags: 0x202, // Default RFLAGS with interrupts enabled
                rip: instruction_pointer.as_u64(),
            },
        };

        let pid = self.next_pid;
        self.processes.push(process);
        self.next_pid += 1;
        Ok(pid)
    }

    pub fn switch_to_user_mode(&self, process: &Process) {
        serial_println!(
            "Preparing to switch to user mode for process {}",
            process.pid
        );
        serial_println!(
            "User IP: {:?}, User SP: {:?}",
            process.instruction_pointer,
            process.stack_pointer
        );

        // Get the page table frame for switching
        let page_table_frame = process.address_space.page_table_frame;

        serial_println!("About to switch to user mode...");

        unsafe {
            // Switch to user mode with page table switch in the assembly
            asm!(
                // First, switch to the process's page table
                "mov cr3, {page_table}",

                // Set up segment registers for user mode
                "mov ds, {user_data}",
                "mov es, {user_data}",
                "mov fs, {user_data}",
                "mov gs, {user_data}",

                // Set up the stack frame for iretq
                "push {user_data}",    // SS (user data segment)
                "push {user_stack}",   // RSP (user stack pointer)
                "push 0x202",          // RFLAGS (enable interrupts)
                "push {user_code}",    // CS (user code segment)
                "push {user_ip}",      // RIP (user instruction pointer)

                // Jump to user mode
                "iretq",

                page_table = in(reg) page_table_frame.start_address().as_u64(),
                user_data = in(reg) u64::from(crate::gdt::GDT.1.user_data.0),
                user_code = in(reg) u64::from(crate::gdt::GDT.1.user_code.0),
                user_stack = in(reg) process.stack_pointer.as_u64(),
                user_ip = in(reg) process.instruction_pointer.as_u64(),
                options(noreturn)
            );
        }
    }

    pub fn get_process(&self, pid: u32) -> Option<&Process> {
        self.processes.iter().find(|p| p.pid == pid)
    }
}
