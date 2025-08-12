use core::arch::asm;

use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::{
    VirtAddr,
    instructions::interrupts::without_interrupts,
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
    Terminated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessType {
    User,
    Kernel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessError {
    OutOfMemory,
    InvalidProgram,
    InvalidStateTransition,
    InvalidInstructionPointer,
    InvalidStackPointer,
}

#[derive(Clone, Copy)]
pub struct Process {
    pub pid: u32,
    pub state: ProcessState,
    pub process_type: ProcessType,
    pub address_space: ProcessAddressSpace,
    pub stack_pointer: VirtAddr,
    pub instruction_pointer: VirtAddr,
    // Saved register state
    pub registers: RegisterState,
    // Flag to track if this process has valid saved register state
    pub has_saved_state: bool,
}

impl Process {
    pub fn cleanup_resources(&mut self) {
        // Clean up any resources associated with the process
        self.state = ProcessState::Terminated;

        self.address_space.cleanup();

        serial_println!("Cleaning up resources for process with PID {}", self.pid);

        // TODO: Clean up any other resources
    }
}

#[repr(C, align(8))]
#[derive(Clone, Copy, Debug)]
pub struct RegisterState {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub rip: u64,
    pub rflags: u64,
    pub rsp: u64,
}

impl RegisterState {
    /// Create a new register state with default values for a new process
    pub fn new() -> Self {
        Self {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            r11: 0,
            r10: 0,
            r9: 0,
            r8: 0,
            rsi: 0,
            rdi: 0,
            rbp: 0,
            rdx: 0,
            rcx: 0,
            rbx: 0,
            rax: 0,
            rip: 0,
            rflags: 0x202, // Default RFLAGS with interrupts enabled
            rsp: 0,
        }
    }

    /// Save the current CPU state into this register state
    pub fn save_current_state(&mut self) {
        unsafe {
            // Save registers in chunks to avoid register pressure
            asm!(
                "mov {}, rax",
                "mov {}, rbx",
                "mov {}, rcx",
                "mov {}, rdx",
                out(reg) self.rax,
                out(reg) self.rbx,
                out(reg) self.rcx,
                out(reg) self.rdx,
            );

            asm!(
                "mov {}, rsi",
                "mov {}, rdi",
                "mov {}, rbp",
                "mov {}, r8",
                out(reg) self.rsi,
                out(reg) self.rdi,
                out(reg) self.rbp,
                out(reg) self.r8,
            );

            asm!(
                "mov {}, r9",
                "mov {}, r10",
                "mov {}, r11",
                "mov {}, r12",
                out(reg) self.r9,
                out(reg) self.r10,
                out(reg) self.r11,
                out(reg) self.r12,
            );

            asm!(
                "mov {}, r13",
                "mov {}, r14",
                "mov {}, r15",
                "mov {}, rsp",
                out(reg) self.r13,
                out(reg) self.r14,
                out(reg) self.r15,
                out(reg) self.rsp,
            );

            asm!(
                "pushfq",
                "pop {}",
                out(reg) self.rflags,
            );
        }
    }
}

/// Save CPU state from interrupt context
/// This function captures the complete CPU state during an interrupt
pub fn save_process_state_from_interrupt() -> RegisterState {
    let mut state = RegisterState::new();
    unsafe {
        // Save registers in chunks to avoid register pressure
        asm!(
            "mov {}, rax",
            "mov {}, rbx",
            "mov {}, rcx",
            "mov {}, rdx",
            out(reg) state.rax,
            out(reg) state.rbx,
            out(reg) state.rcx,
            out(reg) state.rdx,
        );

        asm!(
            "mov {}, rsi",
            "mov {}, rdi",
            "mov {}, rbp",
            "mov {}, r8",
            out(reg) state.rsi,
            out(reg) state.rdi,
            out(reg) state.rbp,
            out(reg) state.r8,
        );

        asm!(
            "mov {}, r9",
            "mov {}, r10",
            "mov {}, r11",
            "mov {}, r12",
            out(reg) state.r9,
            out(reg) state.r10,
            out(reg) state.r11,
            out(reg) state.r12,
        );

        asm!(
            "mov {}, r13",
            "mov {}, r14",
            "mov {}, r15",
            out(reg) state.r13,
            out(reg) state.r14,
            out(reg) state.r15,
        );

        // Get flags
        asm!(
            "pushfq",
            "pop {}",
            out(reg) state.rflags,
        );

        // Get current stack pointer
        asm!(
            "mov {}, rsp",
            out(reg) state.rsp,
        );
    }
    state
}
pub struct ProcessManager {
    processes: Vec<Process>,
    current_pid: u32,
    next_pid: u32,
    kernel_cr3: u64,
}

impl ProcessManager {
    pub fn new() -> Self {
        let kernel_cr3: u64;
        unsafe {
            asm!("mov {}, cr3", out(reg) kernel_cr3);
        }
        serial_println!("Kernel CR3: 0x{:x}", kernel_cr3);

        Self {
            processes: Vec::new(),
            current_pid: 0,
            next_pid: 1,
            kernel_cr3,
        }
    }

    pub fn create_process(
        &mut self,
        binary: &[u8],
        frame_allocator: &mut BootInfoFrameAllocator,
        physical_memory_offset: VirtAddr,
    ) -> Result<u32, ProcessError> {
        serial_println!(
            "Creating process with binary data of {} bytes",
            binary.len()
        );

        // Parse the ELF binary
        let elf = goblin::elf::Elf::parse(binary).expect("Failed to parse ELF");
        serial_println!("ELF entry point: 0x{:x}", elf.entry);
        serial_println!("ELF has {} program headers", elf.program_headers.len());

        // Create the address space first
        serial_println!("Creating address space...");
        let mut address_space = ProcessAddressSpace::new(frame_allocator, physical_memory_offset)
            .map_err(|e| {
            serial_println!("Failed to create address space: {:?}", e);
            ProcessError::OutOfMemory
        })?;

        // Allocate a frame for the stack
        serial_println!("Allocating stack frame...");
        let stack_frame = frame_allocator.allocate_frame().ok_or_else(|| {
            serial_println!("Failed to allocate stack frame");
            ProcessError::OutOfMemory
        })?;

        // Map stack at 0x800000 (8MB mark)
        serial_println!("Mapping stack...");
        let stack_virtual_addr = VirtAddr::new(0x800000);
        address_space
            .map_user_memory(
                stack_virtual_addr,
                stack_frame.start_address(),
                0x1000, // 4KB stack
                PageTableFlags::PRESENT
                    | PageTableFlags::WRITABLE
                    | PageTableFlags::USER_ACCESSIBLE
                    | PageTableFlags::NO_EXECUTE,
                frame_allocator,
            )
            .map_err(|e| {
                serial_println!("Failed to map stack: {:?}", e);
                ProcessError::OutOfMemory
            })?;

        // Copy program data to the mapped memory through virtual memory
        serial_println!("Loading ELF segments...");
        for (i, ph) in elf.program_headers.iter().enumerate() {
            if ph.p_type != goblin::elf::program_header::PT_LOAD {
                serial_println!("Skipping non-loadable segment {}", i);
                continue;
            }

            serial_println!(
                "Loading segment {} at vaddr 0x{:x}, size {} bytes",
                i,
                ph.p_vaddr,
                ph.p_filesz
            );

            let mem_start = ph.p_vaddr;
            let file_start = ph.p_offset as usize;
            let file_end = file_start + ph.p_filesz as usize;

            if file_end > binary.len() {
                serial_println!("Segment {} extends beyond binary data", i);
                return Err(ProcessError::InvalidProgram);
            }

            let segment_data = &binary[file_start..file_end];

            // Calculate how many pages we need for this segment
            let segment_virtual_addr = VirtAddr::new(mem_start & !0xfff); // Page-align the start address
            let segment_end_addr = mem_start + ph.p_memsz;
            let aligned_size = (segment_end_addr + 4095) & !0xfff - (mem_start & !0xfff); // Calculate aligned size
            let pages_needed = aligned_size / 4096;

            serial_println!(
                "Segment {} needs {} pages ({} bytes)",
                i,
                pages_needed,
                aligned_size
            );
            serial_println!(
                "Original segment virtual address: 0x{:x}, aligned: {:?}",
                mem_start,
                segment_virtual_addr
            );

            // Set appropriate flags based on ELF segment permissions
            let mut segment_flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
            if ph.p_flags & goblin::elf::program_header::PF_W != 0 {
                segment_flags |= PageTableFlags::WRITABLE;
            }
            if (ph.p_flags & goblin::elf::program_header::PF_X) == 0 {
                segment_flags |= PageTableFlags::NO_EXECUTE;
            }

            serial_println!(
                "Segment {} ELF flags: readable={}, writable={}, executable={}",
                i,
                (ph.p_flags & goblin::elf::program_header::PF_R) != 0,
                (ph.p_flags & goblin::elf::program_header::PF_W) != 0,
                (ph.p_flags & goblin::elf::program_header::PF_X) != 0
            );
            serial_println!("Segment {} page flags: {:?}", i, segment_flags);

            // Map each page for this segment
            for page_idx in 0..pages_needed {
                let page_virtual_addr = segment_virtual_addr + (page_idx * 4096);

                // Allocate frame for this page
                let page_frame = frame_allocator.allocate_frame().ok_or_else(|| {
                    serial_println!(
                        "Failed to allocate frame for segment {} page {}",
                        i,
                        page_idx
                    );
                    ProcessError::OutOfMemory
                })?;

                serial_println!(
                    "Mapping page {} of segment {} at virtual address {:?}",
                    page_idx,
                    i,
                    page_virtual_addr
                );

                address_space
                    .map_user_memory(
                        page_virtual_addr,
                        page_frame.start_address(),
                        4096,
                        segment_flags,
                        frame_allocator,
                    )
                    .map_err(|e| {
                        serial_println!("Failed to map segment {} page {}: {:?}", i, page_idx, e);
                        ProcessError::OutOfMemory
                    })?;

                // Copy segment data to this page if needed
                let page_offset = page_idx * 4096;
                let page_start_addr = segment_virtual_addr.as_u64() + page_offset;
                let original_segment_start = mem_start;
                let original_segment_end = original_segment_start + ph.p_filesz;

                // Calculate what part of this page should contain data
                let data_start_in_page = if page_start_addr < original_segment_start {
                    (original_segment_start - page_start_addr) as usize
                } else {
                    0
                };

                let data_end_in_page = if page_start_addr + 4096 > original_segment_end {
                    if original_segment_end > page_start_addr {
                        (original_segment_end - page_start_addr) as usize
                    } else {
                        0
                    }
                } else {
                    4096
                };

                if data_start_in_page < data_end_in_page {
                    let page_virtual_ptr = (physical_memory_offset
                        + page_frame.start_address().as_u64())
                    .as_mut_ptr::<u8>();

                    // Calculate offset in the source data
                    let src_offset = if page_start_addr >= original_segment_start {
                        (page_start_addr - original_segment_start) as usize
                    } else {
                        0
                    };

                    let copy_size = data_end_in_page - data_start_in_page;

                    if src_offset < segment_data.len() && copy_size > 0 {
                        let actual_copy_size =
                            core::cmp::min(copy_size, segment_data.len() - src_offset);
                        let data_to_copy = &segment_data[src_offset..src_offset + actual_copy_size];

                        unsafe {
                            // Zero out the entire page first
                            core::ptr::write_bytes(page_virtual_ptr, 0, 4096);

                            // Copy the actual data for this page
                            core::ptr::copy_nonoverlapping(
                                data_to_copy.as_ptr(),
                                page_virtual_ptr.add(data_start_in_page),
                                data_to_copy.len(),
                            );
                        }

                        serial_println!(
                            "Copied {} bytes to page {} of segment {} (src_offset: {}, page_offset: {})",
                            data_to_copy.len(),
                            page_idx,
                            i,
                            src_offset,
                            data_start_in_page
                        );
                    } else {
                        // Zero the page if no data to copy
                        let page_virtual_ptr = (physical_memory_offset
                            + page_frame.start_address().as_u64())
                        .as_mut_ptr::<u8>();
                        unsafe {
                            core::ptr::write_bytes(page_virtual_ptr, 0, 4096);
                        }
                        serial_println!(
                            "Zeroed page {} of segment {} (no data to copy)",
                            page_idx,
                            i
                        );
                    }
                } else {
                    // This page is beyond the file data, just zero it
                    let page_virtual_ptr = (physical_memory_offset
                        + page_frame.start_address().as_u64())
                    .as_mut_ptr::<u8>();
                    unsafe {
                        core::ptr::write_bytes(page_virtual_ptr, 0, 4096);
                    }
                    serial_println!(
                        "Zeroed page {} of segment {} (beyond file data)",
                        page_idx,
                        i
                    );
                }
            }

            serial_println!(
                "Successfully loaded segment {} at {:?} with {} bytes",
                i,
                segment_virtual_addr,
                segment_data.len()
            );
        }

        let stack_pointer = stack_virtual_addr + 0x1000 - 8; // Stack grows downward, point to top of stack minus 8 bytes for alignment
        let instruction_pointer = VirtAddr::new(elf.entry); // Start at ELF entry point

        serial_println!("Setting up process with PID {}", self.next_pid);
        serial_println!("Stack pointer will be at: {:?}", stack_pointer);
        serial_println!("Instruction pointer will be at: {:?}", instruction_pointer);

        let process = Process {
            pid: self.next_pid,
            state: ProcessState::Ready,
            process_type: ProcessType::User,
            address_space,
            stack_pointer,
            instruction_pointer,
            registers: {
                let mut regs = RegisterState::new();
                regs.rsp = stack_pointer.as_u64();
                regs.rip = instruction_pointer.as_u64();
                regs
            },
            has_saved_state: false,
        };

        let pid = self.next_pid;
        // self.current_pid = pid;
        self.processes.push(process);
        self.next_pid += 1;
        Ok(pid)
    }

    pub fn create_kernel_process(
        &mut self,
        entry_point: VirtAddr,
        stack_ptr: VirtAddr,
    ) -> Result<u32, ProcessError> {
        serial_println!(
            "Creating kernel process with entry point: {:?}",
            entry_point
        );

        // Create a dummy address space for kernel process (it won't be used for page table switching)
        // For kernel processes, we'll use the kernel's page table frame (stored in kernel_cr3)
        let kernel_frame = x86_64::structures::paging::PhysFrame::from_start_address(
            x86_64::PhysAddr::new(self.kernel_cr3 & !0xfff), // Remove flags from CR3
        )
        .map_err(|_| ProcessError::OutOfMemory)?;

        let dummy_address_space = crate::memory::ProcessAddressSpace::dummy(kernel_frame);

        let process = Process {
            pid: self.next_pid,
            state: ProcessState::Ready,
            process_type: ProcessType::Kernel,
            address_space: dummy_address_space,
            stack_pointer: stack_ptr,
            instruction_pointer: entry_point,
            registers: {
                let mut regs = RegisterState::new();
                regs.rsp = stack_ptr.as_u64();
                regs.rip = entry_point.as_u64();
                regs
            },
            has_saved_state: false,
        };

        let pid = self.next_pid;
        self.processes.push(process);
        self.next_pid += 1;
        serial_println!("Created kernel process with PID: {}", pid);
        Ok(pid)
    }

    pub fn schedule_next(&mut self) -> Option<&Process> {
        // Find the next ready process
        self.processes
            .iter()
            .find(|p| p.state == ProcessState::Ready)
    }

    pub fn has_running_processes(&self) -> bool {
        self.processes
            .iter()
            .any(|p| p.state != ProcessState::Terminated)
    }

    pub fn set_current_pid(&mut self, pid: u32) {
        self.current_pid = pid;
    }

    pub fn get_current_pid(&self) -> u32 {
        self.current_pid
    }

    pub fn get_process(&self, pid: u32) -> Option<&Process> {
        self.processes.iter().find(|p| p.pid == pid)
    }

    pub fn get_process_mut(&mut self, pid: u32) -> Option<&mut Process> {
        self.processes.iter_mut().find(|p| p.pid == pid)
    }

    pub fn get_next_ready_process(&mut self) -> Option<u32> {
        // Simple round-robin scheduling: find next ready process
        let current_index = if self.current_pid == 0 {
            // No current process, start from beginning
            0
        } else {
            // Find current process index and start from next
            self.processes
                .iter()
                .position(|p| p.pid == self.current_pid)
                .map(|i| (i + 1) % self.processes.len())
                .unwrap_or(0)
        };

        // Look for a ready process starting from current_index
        for i in 0..self.processes.len() {
            let index = (current_index + i) % self.processes.len();
            if self.processes[index].state == ProcessState::Ready {
                return Some(self.processes[index].pid);
            }
        }

        // If we can't find a new process but we have a current process, return it
        if self.current_pid != 0 && self.get_process(self.current_pid).is_some() {
            return Some(self.current_pid);
        }

        None
    }

    pub fn get_current_process(&self) -> Option<&Process> {
        if self.current_pid == 0 {
            None
        } else {
            self.get_process(self.current_pid)
        }
    }

    /// Save the current CPU state into the current process
    pub fn save_current_process_state(
        &mut self,
        interrupt_frame: Option<&x86_64::structures::idt::InterruptStackFrame>,
    ) {
        if self.current_pid == 0 {
            return; // No current process to save
        }

        if let Some(process) = self.get_process_mut(self.current_pid) {
            // If we have an interrupt frame, use it as the primary source of state
            if let Some(frame) = interrupt_frame {
                // Use the interrupt frame for the critical state
                let mut saved_state = RegisterState::new();
                saved_state.rip = frame.instruction_pointer.as_u64();
                saved_state.rsp = frame.stack_pointer.as_u64();
                saved_state.rflags = frame.cpu_flags.bits();

                // For other general-purpose registers, keep them as zero (safe defaults)
                // Don't capture them from interrupt handler context as that would be garbage
                // The process will need to reinitialize any registers it cares about

                process.registers = saved_state;

                // Update the process's instruction pointer and stack pointer
                process.instruction_pointer = VirtAddr::new(saved_state.rip);
                process.stack_pointer = VirtAddr::new(saved_state.rsp);

                // Mark that this process now has valid saved state
                process.has_saved_state = true;

                serial_println!(
                    "Saved state for process {}: RIP=0x{:x}, RSP=0x{:x}, RFLAGS=0x{:x}",
                    self.current_pid,
                    saved_state.rip,
                    saved_state.rsp,
                    saved_state.rflags
                );
            } else {
                serial_println!(
                    "No interrupt frame available, skipping state save for process {}",
                    self.current_pid
                );
            }
        }
    }
}

lazy_static! {
    pub static ref PROCESS_MANAGER: Mutex<ProcessManager> = Mutex::new(ProcessManager::new());
}

// Main scheduling function called by timer interrupt
pub fn schedule() -> ! {
    schedule_with_frame(None)
}

// Enhanced scheduling function that can save state from interrupt context
pub fn schedule_with_frame(
    interrupt_frame: Option<&x86_64::structures::idt::InterruptStackFrame>,
) -> ! {
    // Only schedule if we're not already in a critical section
    if let Some(mut pm) = PROCESS_MANAGER.try_lock() {
        if let Some(next_pid) = pm.get_next_ready_process() {
            // Save the current process state before switching
            if pm.current_pid != 0 {
                // Check if the current process still exists before saving state
                if pm.get_process(pm.current_pid).is_some() {
                    serial_println!("Saving state for process {}", pm.current_pid);
                    pm.save_current_process_state(interrupt_frame);
                } else {
                    serial_println!(
                        "Current process {} no longer exists, skipping state save",
                        pm.current_pid
                    );
                }
            }

            // Clear the current process
            let current_pid = pm.current_pid;
            if let Some(current_process) = pm.get_process_mut(current_pid) {
                current_process.state = ProcessState::Ready;
            }

            // Get and update the next process
            let mut process = {
                let next_process = pm.get_process_mut(next_pid).unwrap();
                next_process.state = ProcessState::Running;

                next_process.clone()
            };

            pm.current_pid = next_pid;

            drop(pm);

            context_switch_to(&mut process);
        } else {
            // No ready processes, switch back to kernel
            if pm.current_pid != 0 {
                serial_println!("No ready processes, switching back to kernel");
                unsafe {
                    asm!("mov cr3, {}", in(reg) pm.kernel_cr3);
                }
                pm.current_pid = 0;
            }

            loop {}
        }
    } else {
        // If we can't get the lock, skip this scheduling round to avoid deadlock
        serial_println!("Failed to acquire PROCESS_MANAGER lock, skipping scheduling");

        loop {}
    }
}

// Function to queue a process without immediately running it
pub fn queue_user_program(
    program: &[u8],
    frame_allocator: &mut BootInfoFrameAllocator,
    physical_memory_offset: VirtAddr,
) -> Result<u32, ProcessError> {
    let mut process_manager = PROCESS_MANAGER.lock();

    match process_manager.create_process(program, frame_allocator, physical_memory_offset) {
        Ok(pid) => {
            serial_println!("Queued process with PID: {}", pid);
            Ok(pid)
        }
        Err(e) => {
            serial_println!("Failed to queue process: {:?}", e);
            Err(e)
        }
    }
}

pub fn context_switch_to(process: &mut Process) -> ! {
    serial_println!("Preparing to switch context to process");

    // Get the process and check if it's a kernel or user process
    // process.state = ProcessState::Running;

    match process.process_type {
        ProcessType::Kernel => {
            serial_println!("Context switching to kernel process ");
            perform_kernel_context_switch(process);
        }
        ProcessType::User => {
            serial_println!("Context switching to user process");
            let page_table_frame = process.address_space.page_table_frame;
            perform_context_switch(page_table_frame, process);
        }
    }
}

fn perform_kernel_context_switch(process: &mut Process) -> ! {
    serial_println!("Performing kernel context switch to process");

    serial_println!("Switching to kernel process {}", process.pid);
    serial_println!(
        "Entry point: {:?}, Stack: {:?}",
        process.instruction_pointer,
        process.stack_pointer
    );

    if !process.has_saved_state {
        serial_println!(
            "First run of kernel process {}, using simple setup",
            process.pid
        );
        // For first-time kernel processes, use simple setup
        perform_kernel_first_run(process);
    } else {
        serial_println!(
            "Resuming kernel process {}, restoring full state",
            process.pid
        );
        // For resumed processes, restore full register state
        perform_kernel_resume(process);
    }
}

fn perform_kernel_first_run(process: &mut Process) -> ! {
    serial_println!("Setting up first run for kernel process {}", process.pid);

    unsafe {
        // Ensure we're using the kernel's page table
        let kernel_cr3 = x86_64::registers::control::Cr3::read()
            .0
            .start_address()
            .as_u64();
        asm!("mov cr3, {}", in(reg) kernel_cr3);
        x86_64::instructions::tlb::flush_all();

        // Get kernel selectors
        let kernel_code_sel = crate::gdt::GDT.1.code.0 as u64;
        let kernel_data_sel = crate::gdt::GDT.1.data.0 as u64;

        // Use iretq setup to ensure interrupts are enabled properly
        let temp_stack = process.stack_pointer.as_u64() - 128;

        asm!(
            "mov rsp, {temp_stack}",
            "push {ss}",      // SS
            "push {krsp}",    // RSP
            "push 0x202",     // RFLAGS (interrupts enabled)
            "push {cs}",      // CS
            "push {rip}",     // RIP
            temp_stack = in(reg) temp_stack,
            ss = in(reg) kernel_data_sel,
            krsp = in(reg) process.stack_pointer.as_u64(),
            cs = in(reg) kernel_code_sel,
            rip = in(reg) process.instruction_pointer.as_u64(),
        );

        // Set up kernel segments
        asm!(
            "mov ax, {data_sel:x}",
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            data_sel = in(reg) kernel_data_sel as u16,
        );

        // Clear registers for clean start
        asm!(
            "xor rax, rax",
            "xor rbx, rbx",
            "xor rcx, rcx",
            "xor rdx, rdx",
            "xor rsi, rsi",
            "xor rdi, rdi",
            "xor rbp, rbp",
            "xor r8, r8",
            "xor r9, r9",
            "xor r10, r10",
            "xor r11, r11",
            "xor r12, r12",
            "xor r13, r13",
            "xor r14, r14",
            "xor r15, r15",
        );

        // Use iretq to properly enable interrupts
        asm!("iretq", options(noreturn));
    }
}

fn perform_kernel_resume(process: &mut Process) -> ! {
    serial_println!("Restoring kernel register state: {:?}", process.registers);

    unsafe {
        // Ensure we're using the kernel's page table
        let kernel_cr3 = x86_64::registers::control::Cr3::read()
            .0
            .start_address()
            .as_u64();
        asm!("mov cr3, {}", in(reg) kernel_cr3);
        x86_64::instructions::tlb::flush_all();

        // Get kernel selectors
        let kernel_code_sel = crate::gdt::GDT.1.code.0 as u64;
        let kernel_data_sel = crate::gdt::GDT.1.data.0 as u64;

        // Switch to a temporary stack and set up iret frame
        let temp_stack = process.registers.rsp - 128;

        asm!(
            "mov rsp, {temp_stack}",
            "push {ss}",      // SS
            "push {krsp}",    // RSP
            "push {rflags}",  // RFLAGS
            "push {cs}",      // CS
            "push {rip}",     // RIP
            temp_stack = in(reg) temp_stack,
            ss = in(reg) kernel_data_sel,
            krsp = in(reg) process.registers.rsp,
            rflags = in(reg) process.registers.rflags,
            cs = in(reg) kernel_code_sel,
            rip = in(reg) process.registers.rip,
        );

        // Restore registers in chunks
        asm!(
            "mov rax, {rax}",
            "mov rbx, {rbx}",
            "mov rcx, {rcx}",
            "mov rdx, {rdx}",
            rax = in(reg) process.registers.rax,
            rbx = in(reg) process.registers.rbx,
            rcx = in(reg) process.registers.rcx,
            rdx = in(reg) process.registers.rdx,
        );

        asm!(
            "mov rsi, {rsi}",
            "mov rdi, {rdi}",
            "mov rbp, {rbp}",
            "mov r8, {r8}",
            rsi = in(reg) process.registers.rsi,
            rdi = in(reg) process.registers.rdi,
            rbp = in(reg) process.registers.rbp,
            r8 = in(reg) process.registers.r8,
        );

        asm!(
            "mov r9, {r9}",
            "mov r10, {r10}",
            "mov r11, {r11}",
            "mov r12, {r12}",
            r9 = in(reg) process.registers.r9,
            r10 = in(reg) process.registers.r10,
            r11 = in(reg) process.registers.r11,
            r12 = in(reg) process.registers.r12,
        );

        asm!(
            "mov r13, {r13}",
            "mov r14, {r14}",
            "mov r15, {r15}",
            r13 = in(reg) process.registers.r13,
            r14 = in(reg) process.registers.r14,
            r15 = in(reg) process.registers.r15,
        );

        // Switch to kernel process
        asm!("iretq", options(noreturn));
    }
}

fn perform_context_switch(
    page_table_frame: x86_64::structures::paging::PhysFrame,
    process: &Process,
) -> ! {
    // Get the process to switch to
    serial_println!("Performing full context switch to process {}", process.pid);

    // Switch to the process's page table
    unsafe {
        asm!("mov cr3, {}", in(reg) page_table_frame.start_address().as_u64());
    }

    serial_println!("Switched to process page table");

    // Now actually switch to user mode and start executing the process
    switch_to_user_mode_direct(process);
}

fn switch_to_user_mode_direct(process: &Process) -> ! {
    serial_println!("Switching to user mode for process {}", process.pid);
    serial_println!(
        "Entry point: {:?}, Stack: {:?}",
        process.instruction_pointer,
        process.stack_pointer
    );

    if !process.has_saved_state {
        serial_println!(
            "First run of user process {}, using simple setup",
            process.pid
        );
        switch_to_user_mode_first_run(process);
    } else {
        serial_println!(
            "Resuming user process {}, restoring full state",
            process.pid
        );
        switch_to_user_mode_resume(process);
    }
}

fn switch_to_user_mode_first_run(process: &Process) -> ! {
    serial_println!("Setting up first run for user process {}", process.pid);

    // Get user mode selectors from GDT - construct with RPL=3
    let user_code_sel = u64::from((crate::gdt::GDT.1.user_code.index() << 3) | 3);
    let user_data_sel = u64::from((crate::gdt::GDT.1.user_data.index() << 3) | 3);

    unsafe {
        // Set up segments
        asm!(
            "mov ax, {0:x}",
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            in(reg) user_data_sel as u16,
        );

        // Simple setup for first run - use the stack and entry point from the process
        asm!(
            // Push values for IRET (in reverse order)
            "push {user_data_sel}",    // SS
            "push {user_stack_ptr}",   // RSP
            "push 0x202",              // RFLAGS (interrupts enabled)
            "push {user_code_sel}",    // CS
            "push {user_ip}",          // RIP

            // Clear all registers for clean start
            "xor rax, rax",
            "xor rbx, rbx",
            "xor rcx, rcx",
            "xor rdx, rdx",
            "xor rsi, rsi",
            "xor rdi, rdi",
            "xor rbp, rbp",
            "xor r8, r8",
            "xor r9, r9",
            "xor r10, r10",
            "xor r11, r11",
            "xor r12, r12",
            "xor r13, r13",
            "xor r14, r14",
            "xor r15, r15",

            // Switch to user mode
            "iretq",
            user_data_sel = in(reg) user_data_sel,
            user_stack_ptr = in(reg) process.stack_pointer.as_u64(),
            user_code_sel = in(reg) user_code_sel,
            user_ip = in(reg) process.instruction_pointer.as_u64(),
            options(noreturn)
        );
    }
}

fn switch_to_user_mode_resume(process: &Process) -> ! {
    serial_println!("Restoring register state: {:?}", process.registers);

    // Get user mode selectors from GDT - construct with RPL=3
    let user_code_sel = u64::from((crate::gdt::GDT.1.user_code.index() << 3) | 3);
    let user_data_sel = u64::from((crate::gdt::GDT.1.user_data.index() << 3) | 3);

    unsafe {
        // First, set up segments
        asm!(
            "mov ax, {0:x}",
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            in(reg) user_data_sel as u16,
        );

        // Create space on stack for the iret frame
        let temp_stack = process.registers.rsp - 128; // Give ourselves some space

        // Switch to our temporary stack and push iret frame
        asm!(
            "mov rsp, {temp_stack}",
            "push {ss}",      // SS
            "push {user_rsp}", // RSP
            "push {rflags}",  // RFLAGS
            "push {cs}",      // CS
            "push {rip}",     // RIP
            temp_stack = in(reg) temp_stack,
            ss = in(reg) user_data_sel,
            user_rsp = in(reg) process.registers.rsp,
            rflags = in(reg) process.registers.rflags,
            cs = in(reg) user_code_sel,
            rip = in(reg) process.registers.rip,
        );

        // Now restore registers in chunks to avoid register pressure
        asm!(
            "mov rax, {rax}",
            "mov rbx, {rbx}",
            "mov rcx, {rcx}",
            "mov rdx, {rdx}",
            rax = in(reg) process.registers.rax,
            rbx = in(reg) process.registers.rbx,
            rcx = in(reg) process.registers.rcx,
            rdx = in(reg) process.registers.rdx,
        );

        asm!(
            "mov rsi, {rsi}",
            "mov rdi, {rdi}",
            "mov rbp, {rbp}",
            "mov r8, {r8}",
            rsi = in(reg) process.registers.rsi,
            rdi = in(reg) process.registers.rdi,
            rbp = in(reg) process.registers.rbp,
            r8 = in(reg) process.registers.r8,
        );

        asm!(
            "mov r9, {r9}",
            "mov r10, {r10}",
            "mov r11, {r11}",
            "mov r12, {r12}",
            r9 = in(reg) process.registers.r9,
            r10 = in(reg) process.registers.r10,
            r11 = in(reg) process.registers.r11,
            r12 = in(reg) process.registers.r12,
        );

        asm!(
            "mov r13, {r13}",
            "mov r14, {r14}",
            "mov r15, {r15}",
            r13 = in(reg) process.registers.r13,
            r14 = in(reg) process.registers.r14,
            r15 = in(reg) process.registers.r15,
        );

        // Finally, switch to user mode
        asm!("iretq", options(noreturn));
    }
}

pub fn queue_kernel_process(entry_point: fn() -> !) {
    let mut pm = PROCESS_MANAGER.lock();
    let entry_point_addr = VirtAddr::new(entry_point as *const () as u64);

    // Allocate a proper kernel stack
    const KERNEL_STACK_SIZE: usize = 4096 * 4; // 16KB stack
    static mut KERNEL_STACK: [u8; KERNEL_STACK_SIZE] = [0; KERNEL_STACK_SIZE];

    let kernel_stack = VirtAddr::from_ptr(&raw const KERNEL_STACK) + KERNEL_STACK_SIZE as u64;

    match pm.create_kernel_process(entry_point_addr, kernel_stack) {
        Ok(pid) => {
            serial_println!("Created executor kernel process with PID: {}", pid);
        }
        Err(e) => serial_println!("Failed to create executor kernel process: {:?}", e),
    }
}

pub fn kill_process(process: &mut Process) -> Result<(), ProcessError> {
    match process.process_type {
        ProcessType::Kernel => {
            // TODO: Figure out what to clean up
            process.stack_pointer = VirtAddr::zero();
            process.instruction_pointer = VirtAddr::zero();
            // process.address_space.cleanup();
        }
        ProcessType::User => {
            process.cleanup_resources();
        }
    }

    serial_println!("Process killed successfully");

    Ok(())
}

pub fn exit_current_process(exit_code: u8) {
    serial_println!("Exiting current process with exit code {}", exit_code);

    without_interrupts(|| {
        let mut pm = PROCESS_MANAGER
            .try_lock()
            .expect("Failed to acquire PROCESS_MANAGER lock");

        // Switch back to kernel page table BEFORE any cleanup
        unsafe {
            asm!("mov cr3, {}", in(reg) pm.kernel_cr3);
            serial_println!(
                "Switched back to kernel page table (CR3: 0x{:x})",
                pm.kernel_cr3
            );
        }

        let current_pid = pm.current_pid;
        pm.current_pid = 0;

        let index = pm
            .processes
            .iter()
            .position(|p| p.pid == current_pid)
            .expect("Current process not found");

        let mut process = pm
            .get_process_mut(current_pid)
            .expect("No current process to exit")
            .clone();

        pm.processes.remove(index);

        drop(pm); // Release the lock before calling cleanup

        kill_process(&mut process)
            .unwrap_or_else(|e| serial_println!("Failed to exit process: {:?}", e));

        serial_println!("Current process exited");
    });
}
