use crate::{
    framebuffer::{
        FRAMEBUFFER,
        font_constants::{CHAR_RASTER_HEIGHT, CHAR_RASTER_WIDTH},
    },
    serial, serial_println,
};
use core::fmt;
use core::fmt::Write;
use core::ptr;
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::instructions::interrupts;

const MAX_CHARS_X: usize = 1280 / CHAR_RASTER_WIDTH;
const MAX_CHARS_Y: usize = 720 / CHAR_RASTER_HEIGHT.val();

pub struct ConsoleWriter {
    x: usize,
    y: usize,
    // Using a raw pointer to avoid stack overflow in case of large screen resolutions.
    // This buffer will be allocated on the heap.
    chars: *mut [[char; MAX_CHARS_X]; MAX_CHARS_Y],
}

impl ConsoleWriter {
    fn new() -> ConsoleWriter {
        // Allocate the character buffer on the heap to prevent stack overflow.
        let layout = core::alloc::Layout::new::<[[char; MAX_CHARS_X]; MAX_CHARS_Y]>();
        let buffer = unsafe {
            let ptr = alloc::alloc::alloc_zeroed(layout) as *mut [[char; MAX_CHARS_X]; MAX_CHARS_Y];
            if ptr.is_null() {
                // In a real kernel, you might want to panic or handle this more gracefully.
                // For now, we'll assume allocation succeeds.
                panic!("Failed to allocate console buffer");
            }
            // Initialize with spaces
            for y in 0..MAX_CHARS_Y {
                for x in 0..MAX_CHARS_X {
                    ptr::write(&mut (*ptr)[y][x], ' ');
                }
            }
            ptr
        };

        ConsoleWriter {
            x: 0,
            y: 0,
            chars: buffer,
        }
    }

    fn chars_mut(&mut self) -> &mut [[char; MAX_CHARS_X]; MAX_CHARS_Y] {
        unsafe { &mut *self.chars }
    }

    fn chars(&self) -> &[[char; MAX_CHARS_X]; MAX_CHARS_Y] {
        unsafe { &*self.chars }
    }

    fn write_char(&mut self, c: char) {
        match c {
            '\n' => self.newline(),
            '\r' => self.carriage_return(),
            _ => {
                if self.x >= MAX_CHARS_X {
                    self.newline();
                }
                if self.y >= MAX_CHARS_Y {
                    self.scroll();
                }
                let x = self.x;
                let y = self.y;
                self.chars_mut()[y][x] = c;
                self.x += 1;

                serial_println!("Writing char '{}' at Y:{}", c, y);
            }
        }
    }

    fn newline(&mut self) {
        self.x = 0;
        self.y += 1;
        if self.y >= MAX_CHARS_Y {
            serial_println!("Reached the end of the console buffer, scrolling...");

            self.scroll();
        }
    }

    fn carriage_return(&mut self) {
        self.x = 0;
    }

    fn scroll(&mut self) {
        serial_println!("Scrolling the console buffer...");

        // Scroll the buffer up by one line
        for y in 1..MAX_CHARS_Y {
            self.chars_mut()[y - 1] = self.chars()[y];
        }
        // Clear the last line
        for x in 0..MAX_CHARS_X {
            self.chars_mut()[MAX_CHARS_Y - 1][x] = ' ';
        }

        // Reset the cursor position
        self.x = 0;
        self.y = MAX_CHARS_Y - 1;

        serial_println!(
            "Console buffer scrolled, resetting cursor position to (0, {})",
            self.y
        );

        self.flush();
    }

    pub fn write_text(&mut self, text: &str) {
        if let Some(fb_mutex) = FRAMEBUFFER.get() {
            let mut fb = fb_mutex.lock();

            for c in text.chars() {
                self.write_char(c);

                if c != '\n' && c != '\r' && c != ' ' {
                    // Draw the last character to the framebuffer
                    fb.write_char(
                        self.x * CHAR_RASTER_WIDTH,
                        self.y * CHAR_RASTER_HEIGHT.val(),
                        c,
                    );
                }
            }
        }
    }

    /// Flushes the character buffer to the framebuffer.
    pub fn flush(&self) {
        if let Some(fb_mutex) = FRAMEBUFFER.get() {
            let mut fb = fb_mutex.lock();
            fb.clear();
            let chars = self.chars();
            for (y, row) in chars.iter().enumerate() {
                for (x, &c) in row.iter().enumerate() {
                    if c != ' ' {
                        // Small optimization
                        fb.write_char(x * CHAR_RASTER_WIDTH, y * CHAR_RASTER_HEIGHT.val(), c);

                        serial_println!(
                            "Writing char '{}' at ({}, {})",
                            c,
                            x * CHAR_RASTER_WIDTH,
                            y * CHAR_RASTER_HEIGHT.val()
                        );
                    }
                }
            }
        }
    }
}

// The ConsoleWriter now contains a raw pointer, so it's not Send/Sync by default.
// We manage access via a Mutex and disable interrupts, making this safe.
unsafe impl Send for ConsoleWriter {}
unsafe impl Sync for ConsoleWriter {}

impl Drop for ConsoleWriter {
    fn drop(&mut self) {
        if !self.chars.is_null() {
            let layout = core::alloc::Layout::new::<[[char; MAX_CHARS_X]; MAX_CHARS_Y]>();
            unsafe {
                alloc::alloc::dealloc(self.chars as *mut u8, layout);
            }
        }
    }
}

impl fmt::Write for ConsoleWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_text(s);
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::console::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    // This function should not be called from an interrupt handler if it can lock.
    // Disabling interrupts prevents timer interrupts and other things from
    // interfering while we are holding the lock and writing to the console.
    interrupts::without_interrupts(|| {
        let mut console = CONSOLE.lock();
        // Write all the formatted arguments to the internal buffer
        console.write_fmt(args).unwrap();
    });
}

// This needs to be defined after ConsoleWriter and its impls.
// It also requires an allocator to be set up.
lazy_static! {
    pub static ref CONSOLE: Mutex<ConsoleWriter> = Mutex::new(ConsoleWriter::new());
}
