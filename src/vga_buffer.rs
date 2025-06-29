use core::fmt;

use crate::graphics::{Color, clear_screen, draw_character};
use lazy_static::lazy_static;
use spin::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    char: char,
    color: Color,
}

const SCREEN_HEIGHT: usize = 16;
const SCREEN_WIDTH: usize = 40;

pub struct Writer {
    column_position: usize,
    color: Color,
    chars: [[ScreenChar; SCREEN_WIDTH]; SCREEN_HEIGHT],
}

impl Writer {
    pub fn write_char(&mut self, character: char) {
        match character {
            '\n' => self.new_line(),
            _ => {
                if self.column_position >= SCREEN_WIDTH {
                    self.new_line();
                }

                let row = SCREEN_HEIGHT - 1;
                let col = self.column_position;

                self.chars[row][col] = ScreenChar {
                    char: character,
                    color: self.color,
                };
                draw_character(col * 8 + 8, row * 12 + 12, character, self.color);
                self.column_position += 1;
            }
        }
    }

    fn new_line(&mut self) {
        for row in 1..SCREEN_HEIGHT {
            for col in 0..SCREEN_WIDTH {
                let character = self.chars[row][col];
                self.chars[row - 1][col] = character;
            }
        }
        self.clear_row(SCREEN_HEIGHT - 1);
        self.column_position = 0;

        // Rerender the whole screen
        clear_screen(Color::Black);
        for row in 0..SCREEN_HEIGHT {
            for col in 0..SCREEN_WIDTH {
                let character = self.chars[row][col];
                if character.char == ' ' {
                    continue; // Skip blank characters
                }
                draw_character(col * 8 + 8, row * 12 + 12, character.char, character.color);
            }
        }
    }

    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            char: ' ',
            color: self.color,
        };
        for col in 0..SCREEN_WIDTH {
            self.chars[row][col] = blank;
        }
    }

    pub fn write_string(&mut self, s: &str) {
        for char in s.chars() {
            match char {
                // printable ASCII byte or newline
                ' '..='~' | '\n' => self.write_char(char),
                // not part of printable ASCII range
                _ => self.write_char('?'),
            }
        }
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        column_position: 0,
        color: Color::Blue,
        chars: [[ScreenChar {
            char: ' ',
            color: Color::Blue
        }; SCREEN_WIDTH]; SCREEN_HEIGHT],
    });
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        writer
            .write_fmt(args)
            .expect("Failed to write to VGA buffer");
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_println_simple() {
        println!("test_println_simple output");
    }

    #[test_case]
    fn test_println_many() {
        for _ in 0..100 {
            println!("test_println_many output");
        }
    }

    #[test_case]
    fn test_println_output() {
        use core::fmt::Write;
        use x86_64::instructions::interrupts;

        let s = "Some test string that fits on 1 line";
        interrupts::without_interrupts(|| {
            let mut writer = WRITER.lock();
            writeln!(writer, "\n{}", s).expect("writeln failed");
            for (i, c) in s.chars().enumerate() {
                let screen_char = writer.chars[SCREEN_HEIGHT - 2][i];
                assert_eq!(screen_char.char, c,);
            }
        });
    }
}
