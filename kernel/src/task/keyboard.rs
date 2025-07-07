use core::{
    pin::Pin,
    task::{Context, Poll},
};
use futures_util::stream::Stream;
use futures_util::stream::StreamExt;
use futures_util::task::AtomicWaker;

use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts};

use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;

use crate::print;

static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
static WAKER: AtomicWaker = AtomicWaker::new();

/// Called by the keyboard interrupt handler
///
/// Must not block or allocate.
pub(crate) fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        if let Err(_) = queue.push(scancode) {
            crate::serial_println!("WARNING: scancode queue full; dropping keyboard input");
        } else {
            crate::serial_println!("Keyboard: Added scancode 0x{:02x} to queue", scancode);
            crate::serial_println!("Keyboard: About to wake keyboard task...");
            WAKER.wake();
            crate::serial_println!("Keyboard: Waker.wake() called");
        }
    } else {
        crate::serial_println!("WARNING: scancode queue uninitialized");
    }
}

pub struct ScancodeStream {
    _private: (),
}

impl ScancodeStream {
    pub fn new() -> Self {
        SCANCODE_QUEUE
            .try_init_once(|| ArrayQueue::new(100))
            .expect("ScancodeStream::new should only be called once");
        ScancodeStream { _private: () }
    }
}

impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<u8>> {
        let queue = SCANCODE_QUEUE
            .try_get()
            .expect("scancode queue not initialized");

        // fast path
        if let Some(scancode) = queue.pop() {
            crate::serial_println!(
                "Keyboard stream: Got scancode 0x{:02x} from queue",
                scancode
            );
            return Poll::Ready(Some(scancode));
        }

        crate::serial_println!("Keyboard stream: No scancode available, registering waker");
        WAKER.register(&cx.waker());
        match queue.pop() {
            Some(scancode) => {
                crate::serial_println!(
                    "Keyboard stream: Got scancode 0x{:02x} after registering waker",
                    scancode
                );
                WAKER.take();
                Poll::Ready(Some(scancode))
            }
            None => {
                crate::serial_println!("Keyboard stream: Still no scancode, returning Pending");
                Poll::Pending
            }
        }
    }
}

pub async fn print_keypresses() {
    crate::serial_println!("Keyboard task starting...");
    let mut scancodes = ScancodeStream::new();
    let mut keyboard = Keyboard::new(ScancodeSet1::new(), layouts::Azerty, HandleControl::Ignore);

    crate::serial_println!("Keyboard task ready to process scancodes");

    let mut poll_counter = 0;
    while let Some(scancode) = scancodes.next().await {
        poll_counter += 1;
        crate::serial_println!(
            "Keyboard: Processing scancode 0x{:02x} (poll #{})",
            scancode,
            poll_counter
        );

        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            crate::serial_println!("Keyboard: Key event: {:?}", key_event);
            if let Some(key) = keyboard.process_keyevent(key_event) {
                crate::serial_println!("Keyboard: Decoded key: {:?}", key);
                match key {
                    DecodedKey::Unicode(character) => {
                        print!("{}", character);
                        crate::serial_println!("Keyboard: Printed character '{}'", character);
                    }
                    DecodedKey::RawKey(key) => {
                        print!("{:?}", key);
                        crate::serial_println!("Keyboard: Printed raw key {:?}", key);
                    }
                }
            }
        }
    }

    crate::serial_println!("Keyboard task ended (this should not happen)");
}
