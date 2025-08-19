use crate::{
    desktop::{
        input::{CLICK_QUEUE, CurrentMouseState, SCANCODE_QUEUE, STATE_QUEUE, init_queues},
        window_manager::{WindowManager, launch_calculator},
    },
    framebuffer::{self, Color, FrameBufferWriter, SCREEN_SIZE},
    print, serial_println,
    surface::{Rect, Shape, Surface},
    time::get_utc_time,
};
use alloc::{format, string::ToString, vec::Vec};
use noto_sans_mono_bitmap::{FontWeight, RasterHeight};
use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts};

use x86_64::instructions::interrupts::without_interrupts;

const TASKBAR_HEIGHT: usize = 50;
const TASKBAR_COLOR: Color = Color::new(175, 175, 175);

pub fn run_desktop() -> ! {
    serial_println!("Running desktop...");
    init_queues();

    let mut mouse_state = CurrentMouseState::new();
    let mut window_manager = WindowManager::new();

    let click_queue = CLICK_QUEUE.get().expect("Click queue not initialized");
    let scancode_queue = SCANCODE_QUEUE
        .try_get()
        .expect("Scancode queue not initialized");
    let mouse_state_queue = STATE_QUEUE
        .try_get()
        .expect("Mouse state queue not initialized");

    let screen_size = *SCREEN_SIZE.get().unwrap();
    let mut desktop = Surface::new(
        screen_size.0 as usize,
        screen_size.1 as usize,
        Color::new(50, 111, 168),
    );
    desktop.just_fill_bg = true;

    let start_button_region = (
        0,
        screen_size.1 as usize - TASKBAR_HEIGHT,
        160,
        TASKBAR_HEIGHT,
    );

    // Taskbar
    // Rerender performance trick:
    const TASKBAR_CHUNK_AMOUNT: usize = 8;
    for i in 0..TASKBAR_CHUNK_AMOUNT {
        desktop.add_shape(Shape::Rectangle {
            x: i * (screen_size.0 as usize / TASKBAR_CHUNK_AMOUNT),
            y: screen_size.1 as usize - TASKBAR_HEIGHT,
            width: screen_size.0 as usize / TASKBAR_CHUNK_AMOUNT,
            height: TASKBAR_HEIGHT,
            color: TASKBAR_COLOR,
            filled: true,
            hide: false,
        });
        desktop.add_shape(Shape::Rectangle {
            x: i * (screen_size.0 as usize / TASKBAR_CHUNK_AMOUNT),
            y: screen_size.1 as usize - TASKBAR_HEIGHT - 1,
            width: screen_size.0 as usize / TASKBAR_CHUNK_AMOUNT,
            height: 1,
            color: Color::BLACK,
            filled: true,
            hide: false,
        });
    }

    // Start button
    desktop.add_shape(Shape::Rectangle {
        x: start_button_region.2,
        y: start_button_region.1,
        width: 1,
        height: start_button_region.3,
        color: Color::BLACK,
        filled: true,
        hide: false,
    });

    desktop.add_shape(Shape::Text {
        x: start_button_region.0 + 50,
        y: start_button_region.1 + 15,
        content: "Start".to_string(),
        color: Color::BLACK,
        background_color: TASKBAR_COLOR,
        font_size: RasterHeight::Size24,
        font_weight: FontWeight::Light,
        hide: false,
    });

    let mut start_menu_entries: Vec<(usize, usize, usize, usize, usize, usize, &str)> = Vec::new(); // (idx, label idx, x, y, width, height, label)
    let mut start_menu_open = false;

    // Start menu placeholder
    start_menu_entries.push((
        desktop.add_shape(Shape::Rectangle {
            x: 0,
            y: screen_size.1 as usize - 300 - TASKBAR_HEIGHT - 2,
            width: 201,
            height: 302,
            color: Color::BLACK,
            filled: false,
            hide: true,
        }),
        desktop.add_shape(Shape::Rectangle {
            x: 0,
            y: screen_size.1 as usize - 300 - TASKBAR_HEIGHT - 1,
            width: 200,
            height: 300,
            color: TASKBAR_COLOR,
            filled: true,
            hide: true,
        }),
        0,
        screen_size.1 as usize - 290 - TASKBAR_HEIGHT,
        200,
        300,
        "",
    ));

    // Calculator start button
    start_menu_entries.push((
        desktop.add_shape(Shape::Rectangle {
            x: 10,
            y: screen_size.1 as usize - 340,
            width: 180,
            height: 30,
            color: Color::WHITE,
            filled: true,
            hide: true,
        }),
        desktop.add_shape(Shape::Text {
            x: 20,
            y: screen_size.1 as usize - 330,
            content: "Calculator".to_string(),
            color: Color::BLACK,
            background_color: Color::WHITE,
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Regular,
            hide: true,
        }),
        10,
        screen_size.1 as usize - 340,
        180,
        30,
        "Calculator",
    ));

    // Time and date background
    desktop.add_shape(Shape::Rectangle {
        x: screen_size.0 as usize - 95,
        y: screen_size.1 as usize - TASKBAR_HEIGHT + 16,
        width: 1,
        height: TASKBAR_HEIGHT - 32,
        color: Color::BLACK,
        filled: true,
        hide: false,
    });

    // Time
    let time_shape_idx = desktop.add_shape(Shape::Text {
        x: screen_size.0 as usize - 80,
        y: screen_size.1 as usize - TASKBAR_HEIGHT + 12,
        content: "22:42".to_string(),
        color: Color::BLACK,
        background_color: TASKBAR_COLOR,
        font_size: RasterHeight::Size16,
        font_weight: FontWeight::Bold,
        hide: false,
    });

    // Date
    let date_shape_idx = desktop.add_shape(Shape::Text {
        x: screen_size.0 as usize - 80,
        y: screen_size.1 as usize - TASKBAR_HEIGHT + 8 + 16,
        content: "8/15/2025".to_string(),
        color: Color::BLACK,
        background_color: TASKBAR_COLOR,
        font_size: RasterHeight::Size16,
        font_weight: FontWeight::Bold,
        hide: false,
    });

    serial_println!("Screen size: {}x{}", screen_size.0, screen_size.1);

    let mut keyboard = Keyboard::new(ScancodeSet1::new(), layouts::Azerty, HandleControl::Ignore);

    let time_update_ticks = 60 * 5; // FPS is somewhere between 60 and 50 (hard to test)
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
            let time_str = format!("{:02}:{:02}", raw_time.hours, raw_time.minutes);
            desktop.update_text_content(time_shape_idx, time_str);

            // Update date
            let date_str = format!("{}/{}/{}", raw_time.day, raw_time.month, raw_time.year);
            desktop.update_text_content(date_shape_idx, date_str);
        }

        while let Some((x, y)) = click_queue.pop() {
            let (mut handled, redraw_region) = window_manager.handle_mouse_click(x, y);
            if let Some((x, y, width, height)) = redraw_region {
                desktop.force_dirty_region(x, y, width, height);
            }

            if handled {
                continue;
            }

            let x = x as usize;
            let y = y as usize;

            if start_menu_open {
                for (_, _, item_x, item_y, width, height, label) in &start_menu_entries {
                    if *item_x <= x && x < *item_x + *width && *item_y <= y && y < *item_y + *height
                    {
                        if *label == "Calculator" {
                            launch_calculator(&mut window_manager);

                            start_menu_open = false;
                            for (idx, label_idx, _, _, _, _, _) in &start_menu_entries {
                                desktop.hide_shape(*idx);
                                desktop.hide_shape(*label_idx);
                            }

                            handled = true;
                            break;
                        }
                    }
                }
            }

            if handled {
                continue;
            }

            // Check if click is within the start button region
            if x >= start_button_region.0
                && x < start_button_region.0 + start_button_region.2
                && y >= start_button_region.1
                && y < start_button_region.1 + start_button_region.3
            {
                start_menu_open = !start_menu_open;

                // Update start menu entries visibility
                for (idx, label_idx, _, _, _, _, _) in &start_menu_entries {
                    if start_menu_open {
                        desktop.show_shape(*idx);
                        desktop.show_shape(*label_idx);
                    } else {
                        desktop.hide_shape(*idx);
                        desktop.hide_shape(*label_idx);
                    }
                }
            }
        }

        if mouse_state.left_button_down && !mouse_state.prev_left_button_down {
            without_interrupts(|| {
                if let Some(fb) = framebuffer::FRAMEBUFFER.get() {
                    let fb_lock = fb.lock();
                    window_manager.handle_mouse_down(mouse_state.x, mouse_state.y, &fb_lock);
                }
            });
        }

        if !mouse_state.left_button_down && mouse_state.prev_left_button_down {
            without_interrupts(|| {
                if let Some(fb) = framebuffer::FRAMEBUFFER.get() {
                    let mut fb_lock = fb.lock();
                    let dirty_regions = window_manager.handle_mouse_release(&mut fb_lock);

                    // Mark all dirty regions from window drag completion
                    for (x, y, width, height) in dirty_regions {
                        desktop.force_dirty_region(x, y, width, height);
                    }
                }
            });
        }

        if mouse_state.has_moved && mouse_state.left_button_down {
            without_interrupts(|| {
                if let Some(fb) = framebuffer::FRAMEBUFFER.get() {
                    let mut fb_lock = fb.lock();
                    window_manager.handle_mouse_move(mouse_state.x, mouse_state.y, &mut fb_lock);
                    // Note: No dirty regions needed during drag since we're using direct framebuffer manipulation
                }
            });
        }

        // Draw desktop
        without_interrupts(|| {
            if let Some(fb) = framebuffer::FRAMEBUFFER.get() {
                let mut fb_lock = fb.lock();

                // Get dirty regions BEFORE rendering (since render() clears them)
                let dirty_regions: Vec<Rect> = desktop.get_dirty_regions().to_vec();

                // Render desktop
                let desktop_rendered = desktop.render(&mut fb_lock, 0, 0, false);

                // Only render windows if they intersect with dirty regions
                let windows_rendered = window_manager.render(&mut fb_lock, &dirty_regions);

                // Handle mouse cursor rendering with region optimization
                let should_redraw_cursor = if mouse_state.has_moved {
                    // Mouse moved, always redraw
                    true
                } else if desktop_rendered || windows_rendered {
                    // Check if any dirty regions intersect with current or previous cursor position
                    let current_cursor_bounds = FrameBufferWriter::get_cursor_bounds(
                        mouse_state.x as usize,
                        mouse_state.y as usize,
                    );
                    let current_cursor_rect = Rect::new(
                        current_cursor_bounds.0,
                        current_cursor_bounds.1,
                        current_cursor_bounds.2,
                        current_cursor_bounds.3,
                    );

                    // Check if dirty regions intersect with current cursor
                    let cursor_intersects = dirty_regions
                        .iter()
                        .any(|region| region.intersects(&current_cursor_rect));

                    if cursor_intersects {
                        fb_lock
                            .save_cursor_background(mouse_state.x as usize, mouse_state.y as usize);
                    }

                    // Also check previous cursor position if it exists
                    let prev_cursor_intersects =
                        if let Some((prev_x, prev_y)) = fb_lock.get_previous_cursor_pos() {
                            let prev_cursor_bounds =
                                FrameBufferWriter::get_cursor_bounds(prev_x, prev_y);
                            let prev_cursor_rect = Rect::new(
                                prev_cursor_bounds.0,
                                prev_cursor_bounds.1,
                                prev_cursor_bounds.2,
                                prev_cursor_bounds.3,
                            );
                            dirty_regions
                                .iter()
                                .any(|region| region.intersects(&prev_cursor_rect))
                        } else {
                            false
                        };

                    cursor_intersects || prev_cursor_intersects
                } else {
                    false
                };

                if should_redraw_cursor {
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
