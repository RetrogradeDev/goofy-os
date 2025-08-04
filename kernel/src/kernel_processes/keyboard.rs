use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts};

use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;

use crate::{print, println, serial_println};

static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();

/// Called by the keyboard interrupt handler
///
/// Must not block or allocate.
pub fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        if let Err(_) = queue.push(scancode) {
            println!("WARNING: scancode queue full; dropping keyboard input");
        }
    } else {
        println!("WARNING: scancode queue uninitialized");
    }
}

pub fn init_scancode_queue() {
    serial_println!("Initializing scancode queue...");

    SCANCODE_QUEUE
        .try_init_once(|| ArrayQueue::new(100))
        .expect("ScancodeStream::new should only be called once");

    serial_println!("Scancode queue initialized successfully!");
}

pub fn print_keypresses() -> ! {
    let mut keyboard = Keyboard::new(ScancodeSet1::new(), layouts::Azerty, HandleControl::Ignore);

    let queue = SCANCODE_QUEUE
        .try_get()
        .expect("Scancode queue not initialized");

    serial_println!("[KEYBOARD] Starting to print keypresses...");
    println!("[KEYBOARD] Press keys to see their output. Press ESC to exit.");

    loop {
        if let Some(scancode) = queue.pop() {
            if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
                if let Some(key) = keyboard.process_keyevent(key_event) {
                    match key {
                        DecodedKey::Unicode(character) => print!("{}", character),
                        DecodedKey::RawKey(key) => print!("{:?}", key),
                    }
                }
            }
        }
    }
}
