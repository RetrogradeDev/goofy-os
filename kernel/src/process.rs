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
                    | PageTableFlags::USER_ACCESSIBLE,
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

            // Allocate frame for this segment
            serial_println!("Allocating frame for segment {}", i);
            let segment_frame = frame_allocator.allocate_frame().ok_or_else(|| {
                serial_println!("Failed to allocate frame for segment {}", i);
                ProcessError::OutOfMemory
            })?;

            // Map segment to memory
            let segment_virtual_addr = VirtAddr::new(mem_start);
            let mut segment_flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;

            // Set appropriate flags based on ELF segment permissions
            if ph.p_flags & goblin::elf::program_header::PF_W != 0 {
                segment_flags |= PageTableFlags::WRITABLE;
            }

            // For executable segments, allow execution by NOT setting NO_EXECUTE
            if ph.p_flags & goblin::elf::program_header::PF_X != 0 {
                // For now, keep both writable and executable for PIE executables
                // which may need to write to their own code segments during loading
                // TODO: Implement proper W^X after dynamic loading is complete
            } else {
                // For non-executable segments, set the NX bit for security
                segment_flags |= PageTableFlags::NO_EXECUTE;
            }

            // For executable segments, we don't need to set a special flag on x86_64
            // because pages are executable by default unless NX bit is set
            // But let's add some debug info about the segment flags
            serial_println!(
                "Segment {} ELF flags: readable={}, writable={}, executable={}",
                i,
                (ph.p_flags & goblin::elf::program_header::PF_R) != 0,
                (ph.p_flags & goblin::elf::program_header::PF_W) != 0,
                (ph.p_flags & goblin::elf::program_header::PF_X) != 0
            );
            serial_println!("Segment {} page flags: {:?}", i, segment_flags);

            serial_println!(
                "Mapping segment {} to virtual address {:?}",
                i,
                segment_virtual_addr
            );
            address_space
                .map_user_memory(
                    segment_virtual_addr,
                    segment_frame.start_address(),
                    ph.p_memsz,
                    segment_flags,
                    frame_allocator,
                )
                .map_err(|e| {
                    serial_println!("Failed to map segment {}: {:?}", i, e);
                    ProcessError::OutOfMemory
                })?;

            // Copy segment data to the mapped memory
            let segment_virtual_ptr = (physical_memory_offset
                + segment_frame.start_address().as_u64())
            .as_mut_ptr::<u8>();

            unsafe {
                // Zero out the entire segment first
                core::ptr::write_bytes(segment_virtual_ptr, 0, ph.p_memsz as usize);

                // Then copy the actual data
                core::ptr::copy_nonoverlapping(
                    segment_data.as_ptr(),
                    segment_virtual_ptr,
                    segment_data.len(),
                );
            }

            serial_println!(
                "Successfully loaded segment {} at {:?} with {} bytes",
                i,
                segment_virtual_addr,
                segment_data.len()
            );
        }

        let stack_pointer = stack_virtual_addr + 0x1000; // Stack grows downward
        let instruction_pointer = VirtAddr::new(elf.entry); // Start at ELF entry point

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

    pub fn switch_to_user_mode(&self, process: &Process, physical_memory_offset: VirtAddr) {
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

        // Get user mode selectors from GDT - these should already have RPL=3
        let user_code_sel = u64::from(crate::gdt::GDT.1.user_code.0) | 3;
        let user_data_sel = u64::from(crate::gdt::GDT.1.user_data.0) | 3;

        serial_println!("User code selector: 0x{:x}", user_code_sel);
        serial_println!("User data selector: 0x{:x}", user_data_sel);

        // Check if NX bit is enabled in EFER
        let mut efer: u64;
        unsafe {
            asm!("mov {}, cr4", out(reg) efer);
        }
        serial_println!("CR4 register: 0x{:x}", efer);

        // Check EFER register
        unsafe {
            asm!("
                mov ecx, 0xc0000080
                rdmsr
                mov {}, rax
            ", out(reg) efer);
        }
        serial_println!("EFER register: 0x{:x}", efer);
        if efer & (1 << 11) != 0 {
            serial_println!("NX bit is ENABLED in EFER - pages are non-executable by default");
        } else {
            serial_println!("NX bit is DISABLED in EFER");
        }
        serial_println!("Page table frame: {:?}", page_table_frame.start_address());

        // Let's try to examine the program code before switching
        serial_println!("Examining program memory before switch...");

        // Look at the actual program code that was loaded
        let program_frame_addr = physical_memory_offset + 0x1f000; // Updated from debug output
        let program_ptr = program_frame_addr.as_ptr::<u8>();
        unsafe {
            serial_println!(
                "Program code first 32 bytes: {:02x?}",
                core::slice::from_raw_parts(program_ptr, 32)
            );
        }

        // Let's also verify the process page table mapping
        serial_println!("Verifying process page table mappings...");

        // Let's temporarily switch to the process page table and see if we can read the program
        let current_cr3: u64;
        unsafe {
            asm!("mov {}, cr3", out(reg) current_cr3);
            serial_println!("Current CR3: 0x{:x}", current_cr3);
            serial_println!(
                "Switching to process CR3: 0x{:x}",
                page_table_frame.start_address().as_u64()
            );

            // Switch to process page table temporarily
            asm!("mov cr3, {}", in(reg) page_table_frame.start_address().as_u64());

            // Try to read from the user program address - this might page fault, so let's be careful
            serial_println!("Attempting to read from user address space...");
            // We'll just switch back immediately since this might cause issues

            // Switch back to kernel page table
            asm!("mov cr3, {}", in(reg) current_cr3);
            serial_println!("Switched back to kernel page table");
        }

        // Try a much more conservative approach - don't change page table yet
        serial_println!("Now attempting user mode switch WITH page table change...");

        unsafe {
            asm!(
                // Switch to the process's page table
                "mov cr3, {page_table}",

                // Zero out general-purpose registers before entering user mode
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

                // Set user data segments before iretq
                "mov ax, {user_data_sel:x}",
                "mov ds, ax",
                "mov es, ax",

                // Set up the stack for iretq
                // Push selectors and pointers to the stack.
                // We use an intermediate register to ensure the full 64-bit value is pushed.
                "mov rax, {user_data_sel}",
                "push rax", // SS
                "push {user_stack}",    // RSP
                "push 0x200",           // RFLAGS (interrupts enabled)
                "mov rax, {user_code_sel}",
                "push rax", // CS
                "push {user_ip}",       // RIP

                // Jump to user mode
                "iretq",
                page_table = in(reg) page_table_frame.start_address().as_u64(),
                user_data_sel = in(reg) user_data_sel,
                user_stack = in(reg) process.stack_pointer.as_u64(),
                user_code_sel = in(reg) user_code_sel,
                user_ip = in(reg) process.instruction_pointer.as_u64(),
                options(noreturn)
            );
        }
    }

    pub fn get_process(&self, pid: u32) -> Option<&Process> {
        self.processes.iter().find(|p| p.pid == pid)
    }
}
