use crate::framebuffer::SCREEN_SIZE;
use crate::print;

use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;
use ps2_mouse::MouseState;

pub static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
pub static STATE_QUEUE: OnceCell<ArrayQueue<MouseState>> = OnceCell::uninit();
pub static CLICK_QUEUE: OnceCell<ArrayQueue<(i16, i16)>> = OnceCell::uninit();

pub fn add_scancode(scancode: u8) {
    if let Some(queue) = SCANCODE_QUEUE.get() {
        if queue.push(scancode).is_err() {
            print!("Scancode queue is full, dropping scancode: {}", scancode);
        }
    } else {
        print!(
            "Scancode queue not initialized, cannot add scancode: {}",
            scancode
        );
    }
}

pub fn add_mouse_state(state: MouseState) {
    if let Some(queue) = STATE_QUEUE.get() {
        if queue.push(state).is_err() {
            print!("Mouse state queue is full, dropping state: {:?}", state);
        }
    } else {
        print!(
            "Mouse state queue not initialized, cannot add state: {:?}",
            state
        );
    }
}

pub fn init_queues() {
    SCANCODE_QUEUE
        .try_init_once(|| ArrayQueue::new(100))
        .expect("Scancode queue should only be initialized once");
    STATE_QUEUE
        .try_init_once(|| ArrayQueue::new(100))
        .expect("Mouse state queue should only be initialized once");
    CLICK_QUEUE
        .try_init_once(|| ArrayQueue::new(20))
        .expect("Click queue should only be initialized once");
}

pub struct CurrentMouseState {
    pub x: i16,
    pub y: i16,
    pub left_button_down: bool,
    pub right_button_down: bool,

    pub has_moved: bool,
    _screen_size: (u16, u16),
}

impl CurrentMouseState {
    pub fn new() -> Self {
        let screen_size = *SCREEN_SIZE.get().unwrap();
        CurrentMouseState {
            x: (screen_size.0 / 2) as i16,
            y: (screen_size.1 / 2) as i16,
            left_button_down: false,
            right_button_down: false,
            has_moved: true, // Ensure the cursor is drawn initially
            _screen_size: screen_size,
        }
    }

    pub fn update(&mut self, state: MouseState) {
        let prev_x = self.x;
        let prev_y = self.y;
        let prev_left_down = self.left_button_down;

        self.x += state.get_x();
        self.y -= state.get_y();

        // Make sure the mouse cursor stays within the screen boundaries
        self.x = self.x.clamp(0, self._screen_size.0 as i16 - 1);
        self.y = self.y.clamp(0, self._screen_size.1 as i16 - 1);

        self.left_button_down = state.left_button_down();
        self.right_button_down = state.right_button_down();

        self.has_moved = self.x != prev_x || self.y != prev_y; // TODO: fix this

        // Detect click: mouse down, no moving, mouse up
        if prev_left_down && !self.left_button_down && !self.has_moved {
            if let Some(queue) = CLICK_QUEUE.get() {
                if queue.push((self.x, self.y)).is_err() {
                    print!(
                        "Click queue is full, dropping click at: ({}, {})",
                        self.x, self.y
                    );
                }
            } else {
                print!(
                    "Click queue not initialized, cannot add click at: ({}, {})",
                    self.x, self.y
                );
            }
        }
    }
}
