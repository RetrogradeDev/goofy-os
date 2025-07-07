use crate::{hlt_loop, print, println, process::PROCESS_MANAGER, serial_println};
use core::arch::naked_asm;
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spin;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use x86_64::{PhysAddr, registers::control::Cr3};

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;
pub const SYSCALL_INTERRUPT: u8 = 0x80;
pub const PROCESS_EXITED: u64 = u64::MAX;

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

// Store the kernel page table address for interrupt handlers
static mut KERNEL_PAGE_TABLE: Option<PhysAddr> = None;

pub fn set_kernel_page_table(addr: PhysAddr) {
    unsafe {
        KERNEL_PAGE_TABLE = Some(addr);
    }
}

fn ensure_kernel_page_table() -> Option<PhysAddr> {
    unsafe {
        let current_cr3 = Cr3::read().0.start_address();
        if let Some(kernel_cr3) = KERNEL_PAGE_TABLE {
            if current_cr3 != kernel_cr3 {
                // Switch to kernel page table for interrupt handling
                core::arch::asm!("mov cr3, {}", in(reg) kernel_cr3.as_u64());
                return Some(current_cr3);
            }
        }
        None
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }
}

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);

        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(crate::gdt::DOUBLE_FAULT_IST_INDEX);
        }

        idt[InterruptIndex::Timer.as_u8()].set_handler_fn(timer_handler);
        idt[InterruptIndex::Keyboard.as_u8()].set_handler_fn(keyboard_interrupt_handler);

        // Set up syscall handler with DPL 3 to allow user mode access
        // Use a custom gate instead of the x86-interrupt attribute
        unsafe {
            let syscall_entry = syscall_handler_asm as *const () as u64;
            idt[SYSCALL_INTERRUPT]
                .set_handler_addr(x86_64::VirtAddr::new(syscall_entry))
                .set_privilege_level(x86_64::PrivilegeLevel::Ring3);
        }

        idt.general_protection_fault.set_handler_fn(general_protection_fault_handler);
        // idt.security_exception
        //     .set_handler_fn(general_protection_fault_handler);

        idt
    };
}

pub fn init_idt() {
    IDT.load();
}

extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    println!("EXCEPTION: GENERAL PROTECTION FAULT\n{:#?}", stack_frame);
    serial_println!(
        "General Protection Fault occurred. Error code: {}",
        error_code
    );
    serial_println!("{:#?}", stack_frame);

    hlt_loop();
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    println!("EXCEPTION: PAGE FAULT",);
    println!("Accessed Address: {:?}", Cr2::read());
    println!("Error Code: {:?}", error_code);
    println!("{:#?}", stack_frame);

    serial_println!("Page Fault occurred at address: {:?}", Cr2::read());
    serial_println!("Error Code: {:?}", error_code);
    serial_println!("{:#?}", stack_frame);

    hlt_loop();
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    serial_println!("Double fault occurred, halting the system.");
    serial_println!("Stack frame: {:#?}", stack_frame);

    println!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);

    hlt_loop();
}

extern "x86-interrupt" fn timer_handler(stack_frame: InterruptStackFrame) {
    // Ensure we're running with the kernel page table
    let original_cr3 = ensure_kernel_page_table();

    // Debug: Print timer info including where it was called from
    serial_println!(
        "Timer interrupt fired from CS:RIP = 0x{:x}:0x{:x}",
        stack_frame.code_segment.0,
        stack_frame.instruction_pointer.as_u64()
    );

    // Print a dot to show timer is working
    print!(".");

    // Check if we need to start any processes
    check_and_start_processes();

    // Tick the async executor to keep tasks running
    crate::task::executor::tick_executor();

    // Notify the Programmable Interrupt Controller (PIC) that the interrupt has been handled
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }

    // Restore original page table if we switched
    if let Some(original) = original_cr3 {
        unsafe {
            core::arch::asm!("mov cr3, {}", in(reg) original.as_u64());
        }
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    // Ensure we're running with the kernel page table
    let original_cr3 = ensure_kernel_page_table();

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };

    serial_println!("Keyboard interrupt: scancode 0x{:02x}", scancode);

    crate::task::keyboard::add_scancode(scancode);

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }

    // Restore original page table if we switched
    if let Some(original) = original_cr3 {
        unsafe {
            core::arch::asm!("mov cr3, {}", in(reg) original.as_u64());
        }
    }
}

// Debug version to figure out correct register values
// TODO
#[unsafe(naked)]
unsafe extern "C" fn syscall_handler_asm() {
    naked_asm!(
        // Save registers
        "push rax",
        "push rbx",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push rbp",

        // Pass register values as arguments to debug function
        // Move original register values (now on stack) to argument registers
        "mov rdi, [rsp + 48]",  // original rax (syscall number)
        "mov rsi, [rsp + 8]",   // original rdi (arg1)
        "mov rdx, [rsp + 16]",  // original rsi (arg2)
        "mov rcx, [rsp + 24]",  // original rdx (arg3)

        "call {}",

        // Check if this was a process exit (return value = PROCESS_EXITED)
        "cmp rax, {}",
        "je 2f",

        // Normal syscall return path
        // Store return value in original rax position
        "mov [rsp + 48], rax",

        // Restore registers
        "pop rbp",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rbx",
        "pop rax",

        // Pop the user RIP, increment it, and push it back.
        "pop r11",
        "add r11, 2",
        "push r11",

        "iretq",

        // Process exit path - don't return to user mode
        "2:",
        // Clean up the stack by removing saved registers
        "add rsp, 56", // 7 registers * 8 bytes
        // Also need to clean up the interrupt frame (rip, cs, rflags, rsp, ss)
        "add rsp, 40", // 5 values * 8 bytes

        // Jump to kernel continuation function instead of returning to user
        "jmp {}",

        sym syscall_handler_rust_debug,
        const PROCESS_EXITED,
        sym kernel_idle_after_process_exit
    );
}

// TODO Debug version to figure out correct register values
extern "C" fn syscall_handler_rust_debug(rax: u64, rdi: u64, rsi: u64, rdx: u64) -> u64 {
    // Ensure we're running with the kernel page table and save the original
    let original_cr3 = ensure_kernel_page_table();

    // Re-enable interrupts for syscall handling
    x86_64::instructions::interrupts::enable();

    serial_println!("Syscall handler (Rust) called");
    serial_println!(
        "Syscall: rax={}, rdi={}, rsi=0x{:x}, rdx={}",
        rax,
        rdi,
        rsi,
        rdx
    );

    let result = handle_syscall(rax, rdi, rsi, rdx);
    serial_println!("Syscall completed, result: {}", result);

    // For sys_exit, we cannot safely return to user mode, so we signal this
    if rax == 60 {
        serial_println!("Process exit syscall - returning special exit code");
        return PROCESS_EXITED; // This will be handled by the assembly wrapper
    } else {
        // Normal syscall - restore the original page table
        if let Some(original) = original_cr3 {
            unsafe {
                core::arch::asm!("mov cr3, {}", in(reg) original.as_u64());
                serial_println!("Restored original page table before returning to user mode");
            }
        }
        serial_println!("About to return from syscall...");
    }

    result
}

/// Called when a process exits to return control to the kernel without returning to user mode
extern "C" fn kernel_idle_after_process_exit() -> ! {
    serial_println!("Process exited - transitioning back to async executor");

    // Make sure we're on the kernel page table
    if let Some(kernel_cr3) = unsafe { KERNEL_PAGE_TABLE } {
        unsafe {
            core::arch::asm!("mov cr3, {}", in(reg) kernel_cr3.as_u64());
        }
    }

    // The process has exited and mark_current_process_for_exit was called,
    // which should have woken the ProcessFuture.

    // However, we can't just return to the async executor from here because
    // we're in a different execution context (we jumped here from syscall).

    // The best we can do is enable interrupts and halt. The timer interrupts
    // will continue to fire and poll the async tasks, including the ProcessFuture
    // which should now detect that the process is terminated.

    serial_println!(
        "Entering interrupt-driven execution - async tasks will continue via timer interrupts"
    );

    loop {
        // Enable interrupts to ensure timer and keyboard interrupts continue
        x86_64::instructions::interrupts::enable();

        // Halt and wait for next interrupt
        // Timer interrupts will continue to fire and keep the async executor active
        x86_64::instructions::hlt();

        // Brief yield point - if any async tasks are ready, this allows them to run
        // This is called from timer interrupt context
        crate::task::executor::tick_executor();
    }
}

fn handle_syscall(number: u64, arg1: u64, arg2: u64, arg3: u64) -> u64 {
    match number {
        1 => sys_write(arg1, arg2, arg3),
        60 => sys_exit(arg1),
        _ => {
            serial_println!("Unknown syscall: {}", number);
            u64::MAX // Error
        }
    }
}

fn sys_write(fd: u64, buf_ptr: u64, count: u64) -> u64 {
    serial_println!(
        "sys_write called: fd={}, buf_ptr=0x{:x}, count={}",
        fd,
        buf_ptr,
        count
    );

    if fd == 1 {
        // stdout
        // For now, just print that we got a write syscall
        serial_println!("Write to stdout: {} bytes", count);
        count // Return number of bytes "written"
    } else {
        serial_println!("Write to unsupported fd: {}", fd);
        0
    }
}

fn sys_exit(exit_code: u64) -> u64 {
    serial_println!("sys_exit called with code: {}", exit_code);

    // Mark the process as exited but don't clean up memory yet
    // The ProcessFuture will handle proper cleanup after detecting the exit
    {
        let mut pm = PROCESS_MANAGER.lock();
        pm.mark_current_process_for_exit(exit_code as u8);
    }

    serial_println!("Process marked for exit with code: {}", exit_code);

    // Return success - but the process is now marked as terminated
    // The ProcessFuture will detect this and complete properly
    0
}

/// Check if any processes need to be started and start them
fn check_and_start_processes() {
    use crate::process::PROCESS_MANAGER;

    let mut pm = PROCESS_MANAGER.lock();

    // Check if there's a process that needs to be started
    if let Some((pid, ip, sp, page_table_addr)) = pm.check_for_process_to_start() {
        serial_println!("Timer interrupt starting user mode for process {}", pid);

        // Drop the lock before switching to user mode
        drop(pm);

        // Switch to user mode - this will not return if the process exits normally
        crate::process::switch_to_user_mode_direct(ip, sp, page_table_addr);

        // If we get here, there was some kind of error or interrupt
        serial_println!("Returned from user mode unexpectedly");
    }
}

#[cfg(test)]
mod tests {
    #[test_case]
    fn test_breakpoint_exception() {
        // invoke a breakpoint exception
        x86_64::instructions::interrupts::int3();
    }
}
