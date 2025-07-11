use crate::{hlt_loop, print, println, serial_println};
use core::arch::naked_asm;
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spin;
use x86_64::{
    instructions::interrupts::enable,
    structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode},
};

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;
pub const SYSCALL_INTERRUPT: u8 = 0x80;

const PROCESS_EXITED: u64 = u64::MAX; // Special value to indicate process exit

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

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

extern "x86-interrupt" fn timer_handler(_stack_frame: InterruptStackFrame) {
    print!(".");
    serial_println!("TIMER");

    // Notify the Programmable Interrupt Controller (PIC) that the interrupt has been handled
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }

    // Implement preemptive scheduling
    // Check if we should schedule a different process
    let mut pm = crate::process::PROCESS_MANAGER.lock();
    if pm.has_running_processes() {
        let current_pid = pm.get_current_pid();
        let next_pid = pm.get_next_ready_process();

        // If we have a next process and it's different from current, schedule it
        if let Some(next) = next_pid {
            if next != current_pid {
                drop(pm);
                // Call scheduler to switch to the next process
                crate::process::schedule();
            }
        } else if current_pid != 0 {
            // No ready processes, but we're not in kernel mode
            // Switch back to kernel idle
            drop(pm);
            crate::process::schedule();
        }
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    crate::task::keyboard::add_scancode(scancode);

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
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

        sym syscall_handler_rust_debug
    );
}

// TODO Debug version to figure out correct register values
extern "C" fn syscall_handler_rust_debug(rax: u64, rdi: u64, rsi: u64, rdx: u64) -> u64 {
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

    // Check if process exited
    if result == PROCESS_EXITED {
        serial_println!("Marking process as terminated...");

        crate::process::kill_current_process(rdi as u8);

        // Spin loop
        // for _ in 0..1_000_000_000 {
        //     unsafe { asm!("nop") };
        // }

        serial_println!("Process marked for exit, returning to scheduler...");

        enable(); // Just to be sure

        //hlt();

        crate::process::schedule();
    }

    serial_println!("About to return from syscall...");

    result
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

    print!(
        "sys_write called: fd={}, buf_ptr=0x{:x}, count={}",
        fd, buf_ptr, count
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
    serial_println!("Process exiting...");

    // Instead of immediately cleaning up, just mark the process for termination
    // The scheduler will handle the actual cleanup on the next timer tick
    serial_println!("Process marked for termination with code: {}", exit_code);

    // Return special value to indicate process exit
    PROCESS_EXITED
}

#[cfg(test)]
mod tests {
    #[test_case]
    fn test_breakpoint_exception() {
        // invoke a breakpoint exception
        x86_64::instructions::interrupts::int3();
    }
}
