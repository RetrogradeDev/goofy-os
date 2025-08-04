use core::{
    pin::Pin,
    task::{Context, Poll},
};
use futures_util::stream::Stream;
use futures_util::stream::StreamExt;
use futures_util::task::AtomicWaker;

use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;
use lazy_static::lazy_static;
use ps2_mouse::MouseState;
use spin::mutex::Mutex;
use x86_64::instructions::interrupts::without_interrupts;

use crate::{
    framebuffer::{Color, FRAMEBUFFER, SCREEN_SIZE},
    serial_println,
};

pub struct CurrentMouseState {
    x: i16,
    y: i16,

    _screen_size: (u16, u16),
}

impl CurrentMouseState {
    pub fn new() -> Self {
        let screen_size = *SCREEN_SIZE.get().unwrap();
        CurrentMouseState {
            x: (screen_size.0 / 2) as i16,
            y: (screen_size.1 / 2) as i16,
            _screen_size: screen_size,
        }
    }

    pub fn update(&mut self, state: MouseState) {
        self.x += state.get_x();
        self.y -= state.get_y();

        // Make sure the mouse cursor stays within the screen boundaries
        self.x = self.x.clamp(0, self._screen_size.0 as i16 - 1);
        self.y = self.y.clamp(0, self._screen_size.1 as i16 - 1);
    }
}

lazy_static! {
    pub static ref CURRENT_MOUSE_STATE: Mutex<CurrentMouseState> =
        Mutex::new(CurrentMouseState::new());
}
static STATE_QUEUE: OnceCell<ArrayQueue<MouseState>> = OnceCell::uninit();
static WAKER: AtomicWaker = AtomicWaker::new();

/// Called by the mouse interrupt handler
///
/// Must not block or allocate.
pub(crate) fn add_mouse_state(state: MouseState) {
    if let Ok(queue) = STATE_QUEUE.try_get() {
        if let Err(_) = queue.push(state) {
            panic!("WARNING: state queue full; dropping mouse input");
        } else {
            WAKER.wake();
        }
    } else {
        serial_println!("WARNING: state queue uninitialized");
    }
}

pub struct StateStream {
    _private: (),
}

impl StateStream {
    pub fn new() -> Self {
        STATE_QUEUE
            .try_init_once(|| ArrayQueue::new(100))
            .expect("StateStream::new should only be called once");
        StateStream { _private: () }
    }
}

impl Stream for StateStream {
    type Item = MouseState;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<MouseState>> {
        let queue = STATE_QUEUE.try_get().expect("state queue not initialized");

        // fast path
        if let Some(state) = queue.pop() {
            return Poll::Ready(Some(state));
        }

        WAKER.register(&cx.waker());
        match queue.pop() {
            Some(state) => {
                WAKER.take();
                Poll::Ready(Some(state))
            }
            None => Poll::Pending,
        }
    }
}

pub async fn print_states() {
    let mut states = StateStream::new();

    while let Some(state) = states.next().await {
        without_interrupts(|| {
            let mut current_state = CURRENT_MOUSE_STATE.lock();
            current_state.update(state);

            // print!("X: {}, Y: {}", current_state.x, current_state.y);
            if let Some(fb) = FRAMEBUFFER.get() {
                // serial_println!("Got one");

                let mut fb = fb.try_lock().unwrap();
                // serial_println!("fb");
                fb.write_pixel(
                    current_state.x as usize,
                    current_state.y as usize,
                    Color::new(255, 0, 0),
                );
                serial_println!("Done");
            } else {
                serial_println!("No framebuffer");
            }

            serial_println!("Done");
        });
    }
}
