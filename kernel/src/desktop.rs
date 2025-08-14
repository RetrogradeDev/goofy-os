use crate::{
    framebuffer::{self, SCREEN_SIZE},
    print, serial_println,
    surface::{Shape, Surface},
};
use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts};

use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;
use ps2_mouse::MouseState;
use x86_64::instructions::interrupts::without_interrupts;

static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
static STATE_QUEUE: OnceCell<ArrayQueue<MouseState>> = OnceCell::uninit();

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
}

pub struct CurrentMouseState {
    pub x: i16,
    pub y: i16,

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

pub fn run_desktop() -> ! {
    serial_println!("Running desktop...");
    init_queues();

    let mut mouse_state = CurrentMouseState::new();
    let scancode_queue = SCANCODE_QUEUE
        .try_get()
        .expect("Scancode queue not initialized");

    let mouse_state_queue = STATE_QUEUE
        .try_get()
        .expect("Mouse state queue not initialized");

    let screen_size = *SCREEN_SIZE.get().unwrap();
    let mut taskbar_surface = Surface::new(screen_size.0 as usize, screen_size.1 as usize);
    taskbar_surface.add_shape(Shape::Rectangle {
        x: 0,
        y: screen_size.1 as usize - 30,
        width: screen_size.0 as usize,
        height: 30,
        color: framebuffer::Color::new(255, 0, 0),
        filled: true,
    });

    let mut keyboard = Keyboard::new(ScancodeSet1::new(), layouts::Azerty, HandleControl::Ignore);

    loop {
        for _ in 0..5000 {
            // Poll for scancodes
            if let Some(scancode) = scancode_queue.pop() {
                if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
                    if let Some(key) = keyboard.process_keyevent(key_event) {
                        match key {
                            DecodedKey::Unicode(character) => print!("{}", character),
                            DecodedKey::RawKey(key) => print!("{:?}", key),
                        }
                    }
                }
            }

            if let Some(state) = mouse_state_queue.pop() {
                mouse_state.update(state);
            }
        }

        // Draw desktop
        without_interrupts(|| {
            if let Some(fb) = framebuffer::FRAMEBUFFER.get() {
                let mut fb_lock = fb.lock();
                // fb_lock.fill(); // Clear to black

                fb_lock.draw_mouse_cursor(mouse_state.x as usize, mouse_state.y as usize);

                taskbar_surface.render(&mut fb_lock);
            } else {
                serial_println!("Framebuffer not initialized");
            }
        });
    }
}
