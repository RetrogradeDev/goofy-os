use crate::{
    desktop::input::{CLICK_QUEUE, CurrentMouseState, SCANCODE_QUEUE, STATE_QUEUE, init_queues},
    framebuffer::{self, SCREEN_SIZE},
    print, serial_println,
    surface::{Shape, Surface},
    time::get_utc_time,
};
use alloc::{format, string::ToString};
use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts};

use x86_64::instructions::interrupts::without_interrupts;

pub fn run_desktop() -> ! {
    serial_println!("Running desktop...");
    init_queues();

    let mut mouse_state = CurrentMouseState::new();

    let click_queue = CLICK_QUEUE.get().expect("Click queue not initialized");

    let scancode_queue = SCANCODE_QUEUE
        .try_get()
        .expect("Scancode queue not initialized");

    let mouse_state_queue = STATE_QUEUE
        .try_get()
        .expect("Mouse state queue not initialized");

    let screen_size = *SCREEN_SIZE.get().unwrap();
    let mut desktop = Surface::new(screen_size.0 as usize, screen_size.1 as usize);

    let start_button_region = (0, screen_size.1 as usize - 30, 80, 30);

    // Taskbar
    desktop.add_shape(Shape::Rectangle {
        x: 0,
        y: screen_size.1 as usize - 30,
        width: screen_size.0 as usize,
        height: 30,
        color: framebuffer::Color::new(255, 0, 0),
        filled: true,
        hide: false,
    });

    // Start button
    desktop.add_shape(Shape::Rectangle {
        x: start_button_region.0,
        y: start_button_region.1,
        width: start_button_region.2,
        height: start_button_region.3,
        color: framebuffer::Color::new(255, 0, 255),
        filled: true,
        hide: false,
    });

    // Start menu placeholder
    let start_menu_idx = desktop.add_shape(Shape::Rectangle {
        x: 0,
        y: screen_size.1 as usize - 330,
        width: 200,
        height: 300,
        color: framebuffer::Color::new(0, 255, 0),
        filled: true,
        hide: true,
    });

    // Time and date background
    desktop.add_shape(Shape::Rectangle {
        x: screen_size.0 as usize - 102,
        y: screen_size.1 as usize - 28,
        width: 100,
        height: 26,
        color: framebuffer::Color::new(0, 0, 0),
        filled: true,
        hide: false,
    });

    // Time
    let time_shape_idx = desktop.add_shape(Shape::Text {
        x: screen_size.0 as usize - 100,
        y: screen_size.1 as usize - 28,
        content: "22:42".to_string(),
        color: framebuffer::Color::new(255, 255, 255),
        fill_bg: false,
        hide: false,
    });

    // Date
    let date_shape_idx = desktop.add_shape(Shape::Text {
        x: screen_size.0 as usize - 100,
        y: screen_size.1 as usize - 16,
        content: "8/15/2025".to_string(),
        color: framebuffer::Color::new(255, 255, 255),
        fill_bg: false,
        hide: false,
    });

    serial_println!("Screen size: {}x{}", screen_size.0, screen_size.1);

    let mut keyboard = Keyboard::new(ScancodeSet1::new(), layouts::Azerty, HandleControl::Ignore);

    let time_update_ticks = 60 * 15; // FPS is somewhere between 60 and 50 (hard to test)
    let mut ticks = 0u64;

    loop {
        for _ in 0..10000 {
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

        if ticks % time_update_ticks == 0 {
            let raw_time = get_utc_time();

            // Update time
            if let Some(shape) = desktop.shapes.get_mut(time_shape_idx) {
                if let Shape::Text { content, .. } = shape {
                    let time_str = format!("{:02}:{:02}", raw_time.hours, raw_time.minutes);

                    *content = time_str;
                }
            }

            // Update date
            if let Some(shape) = desktop.shapes.get_mut(date_shape_idx) {
                if let Shape::Text { content, .. } = shape {
                    let date_str = format!("{}/{}/{}", raw_time.day, raw_time.month, raw_time.year);

                    *content = date_str;
                }
            }

            desktop.is_dirty = true;
        }

        while let Some((x, y)) = click_queue.pop() {
            let x = x as usize;
            let y = y as usize;

            // Check if click is within the start button region
            if x >= start_button_region.0
                && x < start_button_region.0 + start_button_region.2
                && y >= start_button_region.1
                && y < start_button_region.1 + start_button_region.3
            {
                // Toggle start menu visibility
                if let Some(shape) = desktop.shapes.get_mut(start_menu_idx) {
                    if let Shape::Rectangle { hide, .. } = shape {
                        *hide = !*hide;
                    }
                }

                desktop.is_dirty = true;
            }
        }

        // Draw desktop
        without_interrupts(|| {
            if let Some(fb) = framebuffer::FRAMEBUFFER.get() {
                let mut fb_lock = fb.lock();

                let did_render = desktop.render(&mut fb_lock); // TODO: Remove this when we use regions

                if mouse_state.has_moved || did_render {
                    fb_lock.draw_mouse_cursor(mouse_state.x as usize, mouse_state.y as usize);
                    mouse_state.has_moved = false;
                }
            } else {
                serial_println!("Framebuffer not initialized");
            }
        });

        ticks += 1;
    }
}
