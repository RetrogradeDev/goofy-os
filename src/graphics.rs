use lazy_static::lazy_static;
use vga::writers::{Graphics320x200x256, GraphicsWriter, PrimitiveDrawing};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    LightMagenta = 13,
    Yellow = 14,
    White = 15,
    Orange = 208,
}

lazy_static! {
    pub static ref MODE: Graphics320x200x256 = {
        let mode = Graphics320x200x256::new();
        mode.set_mode();
        mode
    };
}

pub fn clear_screen(color: Color) {
    MODE.clear_screen(color as u8);
}

pub fn set_pixel(x: usize, y: usize, color: Color) {
    MODE.set_pixel(x, y, color as u8);
}

pub fn draw_line(start: (isize, isize), end: (isize, isize), color: Color) {
    MODE.draw_line(start, end, color as u8);
}

pub fn draw_rect(top_left: (usize, usize), bottom_right: (usize, usize), color: Color) {
    MODE.draw_rect(top_left, bottom_right, color as u8);
}

pub fn draw_rect_outline(top_left: (isize, isize), bottom_right: (isize, isize), color: Color) {
    MODE.draw_line(top_left, (bottom_right.0, top_left.1), color as u8);
    MODE.draw_line((bottom_right.0, top_left.1), bottom_right, color as u8);
    MODE.draw_line(top_left, (top_left.0, bottom_right.1), color as u8);
    MODE.draw_line((top_left.0, bottom_right.1), bottom_right, color as u8);
}

pub fn draw_character(x: usize, y: usize, character: char, color: Color) {
    MODE.draw_character(x, y, character, color as u8);
}
