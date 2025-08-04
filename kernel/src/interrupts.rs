use crate::{hlt_loop, print, println, serial_println};
use core::arch::asm;
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use ps2_mouse::{Mouse, MouseState};
use spin::{self, lazy::Lazy};
use spinning_top::Spinlock;
use x86_64::{
    instructions::{
        interrupts::{disable, enable, without_interrupts},
        port::PortReadOnly,
    },
    structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode},
};

pub static MOUSE: Lazy<Spinlock<Mouse>> = Lazy::new(|| Spinlock::new(Mouse::new()));

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;
pub const SYSCALL_INTERRUPT: u8 = 0x80;
pub const KEYBOARD_INTERRUPT: u8 = PIC_1_OFFSET + 1;
pub const MOUSE_INTERRUPT: u8 = PIC_1_OFFSET + 12;

const PROCESS_EXITED: u64 = u64::MAX; // Special value to indicate process exit

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard = KEYBOARD_INTERRUPT,
    Mouse = MOUSE_INTERRUPT,
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
        idt[InterruptIndex::Mouse.as_u8()].set_handler_fn(mouse_interrupt_handler);

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

pub fn init_mouse() {
    MOUSE.lock().init().unwrap();
    MOUSE.lock().set_on_complete(on_complete);
}

// This will be fired when a packet is finished being processed.
fn on_complete(mouse_state: MouseState) {
    crate::task::mouse::add_mouse_state(mouse_state);
}

// An example interrupt based on https://os.phil-opp.com/hardware-interrupts/. The ps2 mouse is configured to fire
// interrupts at PIC offset 12.
extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut port = PortReadOnly::new(0x60);
    let packet = unsafe { port.read() };

    // I know this is a bad practice but we are sort of forced to do this here
    // I spent 3h trying to do it otherwise but none of the solutions worked.
    without_interrupts(|| {
        MOUSE.lock().process_packet(packet);
    });

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Mouse.as_u8());
    }
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

    // Switch to the next task
    crate::process::schedule();
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

pub fn syscall_handler_asm() {
    unsafe {
        asm!(
            // Save registers (don't save RCX and R11 - syscall uses these for return)
                    "push rax",
                    "push rbx",
                    "push rdx",
                    "push rsi",
                    "push rdi",
                    "push rbp",
                    "push r8",
                    "push r9",
                    "push r10",

                    // Set up arguments for Rust function
                    // rax = syscall number (already in rax)
                    // rdi = arg1 (already in rdi)
                    // rsi = arg2 (already in rsi)
                    // rdx = arg3 (already in rdx)
                    // So we need to move them to the right positions:
                    "mov r8, rdx",   // Save rdx (arg3)
                    "mov r9, rsi",   // Save rsi (arg2)
                    "mov r10, rdi",  // Save rdi (arg1)
                    "mov rdi, rax",  // syscall number -> first arg
                    "mov rsi, r10",  // arg1 -> second arg
                    "mov rdx, r9",   // arg2 -> third arg
                    "mov rcx, r8",   // arg3 -> fourth arg

                    "call {}",

                    // Restore registers
                    "pop r10",
                    "pop r9",
                    "pop r8",
                    "pop rbp",
                    "pop rdi",
                    "pop rsi",
                    "pop rdx",
                    "pop rbx",
                    // Don't pop rax - it contains the return value

                    // RCX and R11 are already set by syscall instruction
                    // RCX = return RIP, R11 = RFLAGS
                    // Return to user mode
                    "sysretq",

            sym syscall_handler_rust_debug,
            options(noreturn)
        );
    }
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

        disable();

        crate::process::exit_current_process(rdi as u8);

        serial_println!("Process marked for exit, returning to scheduler...");

        enable(); // Just to be sure

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
