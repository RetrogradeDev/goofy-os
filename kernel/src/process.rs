use core::arch::asm;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::Mutex;
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
    Waiting,
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

#[derive(Clone, Copy)]
pub struct Process {
    pub pid: u32,
    pub state: ProcessState,
    pub address_space: ProcessAddressSpace,
    pub stack_pointer: VirtAddr,
    pub instruction_pointer: VirtAddr,
    // Saved register state
    pub registers: RegisterState,
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

#[repr(C)]
#[derive(Clone, Copy)]
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
    current_pid: u32,
    next_pid: u32,
    kernel_cr3: u64,
    scheduler_state: SchedulerState,
    process_wakers: BTreeMap<u32, Waker>, // Store wakers for each process
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SchedulerState {
    Idle,
    RunningProcess(u32),
    ProcessExited(u32),
}

pub struct ProcessFuture {
    pid: u32,
    completed: bool,
}

impl ProcessFuture {
    pub fn new(pid: u32) -> Self {
        Self {
            pid,
            completed: false,
        }
    }
}

impl Future for ProcessFuture {
    type Output = u8; // Exit code

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.completed {
            return Poll::Ready(0);
        }

        let mut pm = PROCESS_MANAGER.lock();

        // Check if process exists and is not terminated
        if let Some(process) = pm.get_process(self.pid) {
            match process.state {
                ProcessState::Terminated => {
                    serial_println!("Process {} is terminated, completing future", self.pid);
                    self.completed = true;
                    Poll::Ready(0) // Process completed
                }
                ProcessState::Ready => {
                    // Start the process if it's ready and no other process is running
                    if matches!(pm.scheduler_state, SchedulerState::Idle) {
                        serial_println!("Starting process {} in async context", self.pid);
                        pm.start_process_async(self.pid);

                        // Store the waker so it can be called when the process exits
                        pm.set_process_waker(self.pid, cx.waker().clone());

                        // Mark that we're about to start user mode execution
                        pm.scheduler_state = SchedulerState::RunningProcess(self.pid);

                        // Return pending immediately - the actual user mode switch will happen
                        // when the timer interrupt calls the scheduler
                        serial_println!(
                            "Process {} scheduled to start, returning pending",
                            self.pid
                        );
                        Poll::Pending
                    } else {
                        cx.waker().wake_by_ref(); // Try again later
                        Poll::Pending
                    }
                }
                ProcessState::Running => {
                    // Process is running, just wait for it to terminate
                    // Store the waker so it can be called when the process exits
                    pm.set_process_waker(self.pid, cx.waker().clone());

                    serial_println!("Process {} is running, waiting for termination", self.pid);
                    Poll::Pending
                }
                ProcessState::Waiting => {
                    // Process is waiting, check back later
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            }
        } else {
            // Process doesn't exist or was cleaned up
            serial_println!(
                "Process {} doesn't exist, completing future with error",
                self.pid
            );
            self.completed = true;
            Poll::Ready(1) // Error exit code
        }
    }
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
            scheduler_state: SchedulerState::Idle,
            process_wakers: BTreeMap::new(),
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
        serial_println!("Parsing ELF binary...");
        let elf = goblin::elf::Elf::parse(binary).expect("Failed to parse ELF");
        serial_println!("ELF entry point: 0x{:x}", elf.entry);
        serial_println!("ELF has {} program headers", elf.program_headers.len());

        // Add yield point after ELF parsing
        crate::task::yield_now_blocking();

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

            serial_println!("=== Processing segment {} ===", i);
            serial_println!("Segment virtual address: 0x{:x}", ph.p_vaddr);
            serial_println!("Segment file size: {} bytes", ph.p_filesz);
            serial_println!("Segment memory size: {} bytes", ph.p_memsz);

            // Add cooperative yield point during ELF loading
            crate::task::yield_now_blocking();

            // Extract the segment data from the binary
            let segment_data = if ph.p_filesz > 0 {
                &binary[ph.p_offset as usize..(ph.p_offset + ph.p_filesz) as usize]
            } else {
                &[]
            };
            serial_println!("Extracted {} bytes of segment data", segment_data.len());

            // Calculate how many pages we need for this segment
            let segment_virtual_addr = VirtAddr::new(ph.p_vaddr & !0xfff); // Page-align the start address
            let segment_end_addr = ph.p_vaddr + ph.p_memsz;
            let aligned_size = (segment_end_addr + 4095) & !0xfff - (ph.p_vaddr & !0xfff); // Calculate aligned size
            let pages_needed = aligned_size / 4096;
            let mem_start = ph.p_vaddr; // Start address in memory for this segment

            serial_println!(
                "Segment {} needs {} pages ({} bytes)",
                i,
                pages_needed,
                aligned_size
            );
            serial_println!(
                "Original segment virtual address: 0x{:x}, aligned: {:?}",
                ph.p_vaddr,
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
        self.current_pid = pid;
        self.processes.push(process);
        self.next_pid += 1;
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

    /// Start a process asynchronously without blocking
    pub fn start_process_async(&mut self, pid: u32) {
        if let Some(process) = self.processes.iter_mut().find(|p| p.pid == pid) {
            process.state = ProcessState::Running;
            self.current_pid = pid;
            self.scheduler_state = SchedulerState::RunningProcess(pid);

            serial_println!("Process {} state changed to Running", pid);
            serial_println!("Process {} will be switched to user mode", pid);
        }
    }

    /// Get process info for switching to user mode
    pub fn get_process_for_execution(
        &self,
        pid: u32,
    ) -> Option<(VirtAddr, VirtAddr, x86_64::PhysAddr)> {
        if let Some(process) = self.get_process(pid) {
            Some((
                process.instruction_pointer,
                process.stack_pointer,
                process.address_space.page_table_frame.start_address(),
            ))
        } else {
            None
        }
    }

    /// Add a new process to the ready queue
    pub fn add_ready_process(&mut self, process: Process) {
        self.processes.push(process);
        serial_println!("Added new ready process with PID {}", process.pid);
    }

    /// Check if the scheduler needs new processes
    pub fn needs_processes(&self) -> bool {
        !self.has_running_processes() && matches!(self.scheduler_state, SchedulerState::Idle)
    }

    /// Kills the process with the given PID and cleans up resources
    pub fn kill_process(&mut self, pid: u32) -> Result<(), ProcessError> {
        if let Some(index) = self.processes.iter().position(|p| p.pid == pid) {
            serial_println!("Killing process with PID {}", pid);

            // Clean up any resources associated with the process
            let process = &mut self.processes[index];
            process.cleanup_resources();

            serial_println!("Process with PID {} is in state {:?}", pid, process.state);

            // Finally remove the process from the list
            self.processes.remove(index);

            serial_println!("Process with PID {} killed successfully", pid);

            Ok(())
        } else {
            serial_println!("Process with PID {} not found", pid);
            Err(ProcessError::InvalidStateTransition)
        }
    }

    pub fn get_process(&self, pid: u32) -> Option<&Process> {
        self.processes.iter().find(|p| p.pid == pid)
    }

    pub fn mark_current_process_for_exit(&mut self, exit_code: u8) {
        serial_println!("Marking current process for exit with code {}", exit_code);

        // Find the current process and mark it as terminated
        if let Some(process) = self
            .processes
            .iter_mut()
            .find(|p| p.pid == self.current_pid)
        {
            process.state = ProcessState::Terminated;
            serial_println!("Process {} marked as Terminated", self.current_pid);

            // Wake the ProcessFuture so it can detect the termination
            self.wake_process_waker(self.current_pid);
        } else {
            serial_println!(
                "Warning: Could not find current process {} to mark for exit",
                self.current_pid
            );
        }

        // DON'T update scheduler state yet - let the ProcessFuture handle the cleanup
        // after it safely returns from user mode
        serial_println!("Process {} marked for delayed cleanup", self.current_pid);
    }

    pub fn exit_current_process(&mut self, exit_code: u8) {
        serial_println!("Exiting current process with exit code {}", exit_code);

        // Switch back to kernel page table BEFORE any cleanup
        unsafe {
            asm!("mov cr3, {}", in(reg) self.kernel_cr3);
            serial_println!(
                "Switched back to kernel page table (CR3: 0x{:x})",
                self.kernel_cr3
            );
        }

        // Update scheduler state
        self.scheduler_state = SchedulerState::ProcessExited(self.current_pid);

        self.kill_process(self.current_pid)
            .unwrap_or_else(|e| serial_println!("Failed to exit process: {:?}", e));

        serial_println!("Current process exited");

        self.current_pid = 0; // Reset current PID after exit
        self.scheduler_state = SchedulerState::Idle;
    }

    /// Store a waker for a process so it can be notified when the process exits
    pub fn set_process_waker(&mut self, pid: u32, waker: Waker) {
        serial_println!("Setting waker for process {}", pid);
        self.process_wakers.insert(pid, waker);
    }

    /// Wake the waker for a process (called when process exits)
    pub fn wake_process_waker(&mut self, pid: u32) {
        if let Some(waker) = self.process_wakers.remove(&pid) {
            serial_println!("Waking ProcessFuture for process {}", pid);
            waker.wake();
        } else {
            serial_println!("No waker found for process {}", pid);
        }
    }

    /// Check if there's a process that needs to be started and return its execution info
    pub fn check_for_process_to_start(
        &mut self,
    ) -> Option<(u32, VirtAddr, VirtAddr, x86_64::PhysAddr)> {
        if let SchedulerState::RunningProcess(pid) = self.scheduler_state {
            if let Some((ip, sp, page_table_addr)) = self.get_process_for_execution(pid) {
                serial_println!("Process {} ready to start from timer interrupt", pid);
                return Some((pid, ip, sp, page_table_addr));
            }
        }
        None
    }
}

lazy_static! {
    pub static ref PROCESS_MANAGER: Mutex<ProcessManager> = Mutex::new(ProcessManager::new());
}

/// Create and run a process asynchronously
pub async fn run_process_async(
    binary: &[u8],
    frame_allocator: &mut BootInfoFrameAllocator,
    physical_memory_offset: VirtAddr,
) -> u8 {
    // Create the process
    let pid = {
        let mut pm = PROCESS_MANAGER.lock();
        match pm.create_process(binary, frame_allocator, physical_memory_offset) {
            Ok(pid) => {
                serial_println!("Created process with PID: {} (async)", pid);
                pid
            }
            Err(e) => {
                serial_println!("Failed to create process: {:?}", e);
                return 1; // Error exit code
            }
        }
    };

    // Start the process and signal it's ready to run
    {
        let mut pm = PROCESS_MANAGER.lock();
        pm.start_process_async(pid);
    }

    // Use ProcessFuture to wait for the process to complete
    let process_future = ProcessFuture::new(pid);

    // Wait for the process to complete
    process_future.await
}

/// Background scheduler that keeps the process manager alive
pub async fn process_scheduler_background() {
    serial_println!("Starting background process scheduler...");

    let mut tick_counter = 0;
    loop {
        tick_counter += 1;

        // Check for processes that need cleanup
        {
            let mut pm = PROCESS_MANAGER.lock();

            // Check scheduler state and log it (less frequently)
            if tick_counter % 50 == 0 {
                serial_println!(
                    "Background scheduler tick #{} - state: {:?}",
                    tick_counter,
                    pm.scheduler_state
                );
            }

            // Clean up processes marked for exit
            match pm.scheduler_state {
                SchedulerState::ProcessExited(pid) => {
                    serial_println!("Background scheduler cleaning up exited process {}", pid);
                    pm.exit_current_process(0); // Complete the cleanup
                    serial_println!("Process {} cleanup completed", pid);
                }
                _ => {
                    // Also check for terminated processes that haven't been marked for cleanup yet
                    // This handles the case where the ProcessFuture gets stuck and can't trigger cleanup
                    let terminated_processes: Vec<u32> = pm
                        .processes
                        .iter()
                        .filter(|p| p.state == ProcessState::Terminated)
                        .map(|p| p.pid)
                        .collect();

                    if !terminated_processes.is_empty() {
                        serial_println!(
                            "Found {} terminated processes that need cleanup",
                            terminated_processes.len()
                        );
                        for pid in terminated_processes {
                            serial_println!(
                                "Background scheduler force-cleaning up terminated process {}",
                                pid
                            );
                            pm.scheduler_state = SchedulerState::ProcessExited(pid);
                            break; // Handle one at a time
                        }
                    }
                }
            }
        }

        // Don't switch to user mode directly from here - this would kill the background task
        // Instead, just yield and let the syscall handler or other mechanisms handle process switching

        // Yield to allow other tasks to run
        crate::task::yield_now().await;

        // Small delay to prevent busy waiting but keep the scheduler alive
        for _ in 0..10000 {
            core::hint::spin_loop();
        }
    }
}

/// Switch to user mode without requiring a Process reference
pub fn switch_to_user_mode_direct(
    instruction_pointer: VirtAddr,
    stack_pointer: VirtAddr,
    page_table_frame_addr: x86_64::PhysAddr,
) {
    serial_println!("Switching to user mode (direct)");
    serial_println!("IP: {:?}, SP: {:?}", instruction_pointer, stack_pointer);

    // Get user mode selectors from GDT
    let user_code_sel = u64::from(crate::gdt::GDT.1.user_code.0) | 3;
    let user_data_sel = u64::from(crate::gdt::GDT.1.user_data.0) | 3;

    unsafe {
        // Switch to the process's page table
        asm!("mov cr3, {}", in(reg) page_table_frame_addr.as_u64());

        asm!(
            "
            // Set up user mode segments
            mov ax, {user_data_sel_16:x}
            mov ds, ax
            mov es, ax
            mov fs, ax
            mov gs, ax

            // Push values for IRET (in reverse order)
            push {user_data_sel}        // SS
            push {user_stack_ptr}       // RSP
            push 0x202                  // RFLAGS (interrupts enabled)
            push {user_code_sel}        // CS
            push {user_ip}              // RIP

            // Switch to user mode
            iretq
            ",
            user_data_sel_16 = in(reg) (user_data_sel as u16),
            user_data_sel = in(reg) user_data_sel,
            user_stack_ptr = in(reg) stack_pointer.as_u64(),
            user_code_sel = in(reg) user_code_sel,
            user_ip = in(reg) instruction_pointer.as_u64(),
            options(noreturn)
        );
    }
}
