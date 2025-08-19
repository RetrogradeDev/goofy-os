use alloc::vec::Vec;
use bootloader_api::info::{FrameBuffer, FrameBufferInfo, PixelFormat};
use conquer_once::spin::OnceCell;
use core::{fmt, ptr};
use font_constants::BACKUP_CHAR;
use noto_sans_mono_bitmap::{
    FontWeight, RasterHeight, RasterizedChar, get_raster, get_raster_width,
};
use spinning_top::Spinlock;

use crate::serial_println;

pub static FRAMEBUFFER: OnceCell<Spinlock<FrameBufferWriter>> = OnceCell::uninit();
pub static SCREEN_SIZE: OnceCell<(u16, u16)> = OnceCell::uninit();

/// Prints to framebuffer
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::framebuffer::_print(format_args!($($arg)*))
    };
}

/// Prints to framebuffer, appending a newline.
#[macro_export]
macro_rules! println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::print!(
        concat!($fmt, "\n"), $($arg)*));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use fmt::Write;
    use x86_64::instructions::interrupts;
    interrupts::without_interrupts(|| {
        if let Some(fb) = FRAMEBUFFER.get() {
            if let Some(mut guard) = fb.try_lock() {
                guard.write_fmt(args).unwrap();
            } else {
                serial_println!("[FB DEADLOCK] - {}", args.as_str().unwrap_or("<unknown>"));
            }
        }
    });
}

/// Additional vertical space between lines
const LINE_SPACING: usize = 2;
/// Additional horizontal space between characters.
const LETTER_SPACING: usize = 0;

/// Padding from the border. Prevent that font is too close to border.
const BORDER_PADDING: usize = 1;

const fn calculate_cursor_bg_data_size(rows: &[(isize, isize)]) -> usize {
    let mut sum = 0;
    let mut i = 0;

    while i < rows.len() {
        // For some weird reason rust doesn't allow iter, map or for
        let (start_x, end_x) = rows[i];
        sum += (end_x - start_x + 1) as usize;
        i += 1;
    }
    sum
}

const CURSOR_ROWS: [(isize, isize); 17] = [
    (0, 0), // Tip of the triangle
    (0, 1),
    (0, 3),
    (0, 4),
    (1, 6),
    (1, 7),
    (1, 9),
    (1, 10),
    (1, 11),
    (2, 13),
    (2, 14),
    (2, 13),
    (2, 11),
    (2, 9),
    (2, 7),
    (3, 5),
    (3, 3),
];

const CURSOR_BG_DATA_SIZE: usize = calculate_cursor_bg_data_size(&CURSOR_ROWS);
const CURSOR_ROW_OFFSET: usize = 0; // Offset for the cursor rows from the top of the cursor
const CURSOR_COLOR: Color = Color::BLUE;

const TEXT_COLOR: Color = Color::new(255, 255, 150);

/// Constants for the usage of the [`noto_sans_mono_bitmap`] crate.
mod font_constants {
    use super::*;

    /// Height of each char raster. The font size is ~0.84% of this. Thus, this is the line height that
    /// enables multiple characters to be side-by-side and appear optically in one line in a natural way.
    pub const CHAR_RASTER_HEIGHT: RasterHeight = RasterHeight::Size16;

    /// The width of each single symbol of the mono space font.
    pub const CHAR_RASTER_WIDTH: usize = get_raster_width(FontWeight::Regular, CHAR_RASTER_HEIGHT);

    /// Backup character if a desired symbol is not available by the font.
    /// The '�' character requires the feature "unicode-specials".
    pub const BACKUP_CHAR: char = '�';

    pub const FONT_WEIGHT: FontWeight = FontWeight::Regular;
}

/// Returns the raster of the given char or the raster of [`font_constants::BACKUP_CHAR`].
fn get_char_raster(c: char) -> RasterizedChar {
    fn get(c: char) -> Option<RasterizedChar> {
        get_raster(
            c,
            font_constants::FONT_WEIGHT,
            font_constants::CHAR_RASTER_HEIGHT,
        )
    }
    get(c).unwrap_or_else(|| get(BACKUP_CHAR).expect("Should get raster of backup char."))
}

#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const BLACK: Color = Color { r: 0, g: 0, b: 0 };
    pub const GRAY: Color = Color {
        r: 150,
        g: 150,
        b: 150,
    };
    pub const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
    };
    pub const RED: Color = Color { r: 255, g: 0, b: 0 };
    pub const GREEN: Color = Color { r: 0, g: 255, b: 0 };
    pub const BLUE: Color = Color { r: 0, g: 0, b: 255 };

    pub fn to_u8(&self) -> u8 {
        (self.r + self.g + self.b) / 3
    }

    pub fn to_rgb(&self) -> [u8; 3] {
        [self.r, self.g, self.b]
    }

    pub fn to_bgr(&self) -> [u8; 3] {
        [self.b, self.g, self.r]
    }
}

struct CursorBackground {
    saved_pixels: [u8; CURSOR_BG_DATA_SIZE * 3], // TODO: Don't hardcode this
    previous_pos: Option<(usize, usize)>,
}

impl CursorBackground {
    fn new(bytes_per_pixel: usize) -> Self {
        if bytes_per_pixel != 3 {
            panic!("Cursor background only supports 3 bytes per pixel (RGB).");
        }

        Self {
            saved_pixels: [0; CURSOR_BG_DATA_SIZE * 3],
            previous_pos: None,
        }
    }
}

/// Allows logging text to a pixel-based framebuffer.
pub struct FrameBufferWriter {
    framebuffer: &'static mut [u8],
    info: FrameBufferInfo,
    x_pos: usize,
    y_pos: usize,
    cursor_background: CursorBackground,
}

impl FrameBufferWriter {
    /// Creates a new logger that uses the given framebuffer.
    pub fn new(framebuffer: &'static mut [u8], info: FrameBufferInfo) -> Self {
        let mut logger = Self {
            framebuffer,
            info,
            x_pos: 0,
            y_pos: 0,
            cursor_background: CursorBackground::new(info.bytes_per_pixel),
        };
        logger.clear();
        logger
    }

    fn newline(&mut self) {
        self.y_pos += font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING;
        self.carriage_return()
    }

    fn carriage_return(&mut self) {
        self.x_pos = BORDER_PADDING;
    }

    pub fn fill(&mut self, brightness: u8) {
        self.framebuffer.fill(brightness);
    }

    /// Erases all text on the screen. Resets `self.x_pos` and `self.y_pos`.
    pub fn clear(&mut self) {
        self.x_pos = BORDER_PADDING;
        self.y_pos = BORDER_PADDING;
        self.framebuffer.fill(0);
    }

    fn width(&self) -> usize {
        self.info.width
    }

    fn height(&self) -> usize {
        self.info.height
    }

    pub fn size(&self) -> (usize, usize) {
        (self.info.width, self.info.height)
    }

    /// Writes a single char to the framebuffer. Takes care of special control characters, such as
    /// newlines and carriage returns.
    fn write_char(&mut self, c: char) {
        match c {
            '\n' => self.newline(),
            '\r' => self.carriage_return(),
            c => {
                let new_xpos = self.x_pos + font_constants::CHAR_RASTER_WIDTH;
                if new_xpos >= self.width() {
                    self.newline();
                }
                let new_ypos =
                    self.y_pos + font_constants::CHAR_RASTER_HEIGHT.val() + BORDER_PADDING;
                if new_ypos >= self.height() {
                    self.clear();
                }
                self.write_rendered_char(get_char_raster(c));
            }
        }
    }

    /// Prints a rendered char into the framebuffer.
    /// Updates `self.x_pos`.
    fn write_rendered_char(&mut self, rendered_char: RasterizedChar) {
        self.write_rendered_char_at_pos(
            self.x_pos,
            self.y_pos,
            &rendered_char,
            TEXT_COLOR,
            Color::BLACK,
        );
        self.x_pos += rendered_char.width() + LETTER_SPACING;
    }

    fn write_rendered_char_at_pos(
        &mut self,
        pos_x: usize,
        pos_y: usize,
        rendered_char: &RasterizedChar,
        color: Color,
        bg_color: Color,
    ) {
        for (y, row) in rendered_char.raster().iter().enumerate() {
            for (x, byte) in row.iter().enumerate() {
                if byte == &0 {
                    continue;
                }

                let byte = *byte as f32 / 255.0;
                let byte_reverse = 1.0 - byte;

                let color = Color::new(
                    ((color.r as f32 * byte) + (bg_color.r as f32 * byte_reverse)) as u8,
                    ((color.g as f32 * byte) + (bg_color.g as f32 * byte_reverse)) as u8,
                    ((color.b as f32 * byte) + (bg_color.b as f32 * byte_reverse)) as u8,
                );

                self.write_pixel(pos_x + x, pos_y + y, color);
            }
        }
    }

    pub fn write_pixel(&mut self, x: usize, y: usize, color: Color) {
        if x >= self.width() || y >= self.height() {
            return; // Out of bounds
        }

        let pixel_offset = y * self.info.stride + x;
        let color = match self.info.pixel_format {
            PixelFormat::Rgb => [color.r, color.g, color.b, 0],
            PixelFormat::Bgr => [color.b, color.g, color.r, 0],
            PixelFormat::U8 => [if color.to_u8() > 200 { 0xf } else { 0 }, 0, 0, 0],
            other => {
                // set a supported (but invalid) pixel format before panicking to avoid a double
                // panic; it might not be readable though
                self.info.pixel_format = PixelFormat::Rgb;
                panic!("pixel format {:?} not supported in logger", other)
            }
        };

        let bytes_per_pixel = self.info.bytes_per_pixel;
        let byte_offset = pixel_offset * bytes_per_pixel;
        self.framebuffer[byte_offset..(byte_offset + bytes_per_pixel)]
            .copy_from_slice(&color[..bytes_per_pixel]);
        let _ = unsafe { ptr::read_volatile(&self.framebuffer[byte_offset]) };
    }

    pub fn read_pixel(&self, x: usize, y: usize) -> Color {
        if x >= self.width() || y >= self.height() {
            return Color::BLACK; // Out of bounds, return black
        }

        let pixel_offset = y * self.info.stride + x;
        let bytes_per_pixel = self.info.bytes_per_pixel;
        let byte_offset = pixel_offset * bytes_per_pixel;

        let bytes = &self.framebuffer[byte_offset..(byte_offset + bytes_per_pixel)];

        match self.info.pixel_format {
            PixelFormat::Rgb => Color::new(bytes[0], bytes[1], bytes[2]),
            PixelFormat::Bgr => Color::new(bytes[2], bytes[1], bytes[0]),
            PixelFormat::U8 => {
                let intensity = if bytes[0] > 0 { 255 } else { 0 };
                Color::new(intensity, intensity, intensity)
            }
            other => {
                panic!("pixel format {:?} not supported for reading", other)
            }
        }
    }

    pub fn write_raw_pixel_row(&mut self, x: usize, y: usize, data: &[u8]) {
        if y >= self.height() || x >= self.width() {
            return;
        }

        let max_width = (self.width() - x).min(data.len() / self.info.bytes_per_pixel);
        let bytes_per_pixel = self.info.bytes_per_pixel;
        let start_offset = (y * self.info.stride + x) * bytes_per_pixel;

        self.framebuffer[start_offset..start_offset + max_width * bytes_per_pixel]
            .copy_from_slice(&data[..max_width * bytes_per_pixel]);
    }

    /// Read a horizontal line of pixels at once
    pub fn read_raw_pixel_row(&self, x: usize, y: usize, count: usize) -> &[u8] {
        if y >= self.height() || x >= self.width() {
            return &[];
        }

        let max_width = (self.width() - x).min(count);
        let bytes_per_pixel = self.info.bytes_per_pixel;
        let start_offset = (y * self.info.stride + x) * bytes_per_pixel;

        let data = self
            .framebuffer
            .get(start_offset..start_offset + max_width * bytes_per_pixel)
            .unwrap_or(&[]);

        data
    }

    pub fn draw_mouse_cursor(&mut self, x: usize, y: usize) {
        if let Some(prev_pos) = self.cursor_background.previous_pos {
            self.restore_cursor_background(prev_pos);
        }

        self.save_cursor_background(x, y);
        self.draw_cursor(x, y);

        self.cursor_background.previous_pos = Some((x, y));
    }

    /// Get the bounds of the mouse cursor at the given position
    pub fn get_cursor_bounds(x: usize, y: usize) -> (usize, usize, usize, usize) {
        let start_y = y.saturating_sub(CURSOR_ROW_OFFSET);
        let mut min_x = x;
        let mut max_x = x;
        let mut max_y = start_y;

        for (i, (start_x, end_x)) in CURSOR_ROWS.iter().enumerate() {
            let cursor_y = start_y + i;
            let cursor_start_x = (x as isize + start_x).max(0) as usize;
            let cursor_end_x = (x as isize + end_x).max(0) as usize;

            min_x = min_x.min(cursor_start_x);
            max_x = max_x.max(cursor_end_x);
            max_y = max_y.max(cursor_y);
        }

        (min_x, start_y, max_x - min_x + 1, max_y - start_y + 1)
    }

    /// Get the previous cursor position
    pub fn get_previous_cursor_pos(&self) -> Option<(usize, usize)> {
        self.cursor_background.previous_pos
    }

    fn draw_cursor(&mut self, x: usize, y: usize) {
        let start_y: usize = y.saturating_sub(CURSOR_ROW_OFFSET);

        for (i, (start_x, end_x)) in CURSOR_ROWS.iter().enumerate() {
            let cursor_y = start_y as isize + i as isize;
            let start_x = x as isize + start_x;
            let end_x = x as isize + end_x;

            if cursor_y < 0 || cursor_y >= self.height() as isize {
                continue; // Skip out-of-bounds rows
            }

            self.write_pixel_row(
                start_x as usize,
                end_x as usize,
                cursor_y as usize,
                &CURSOR_COLOR,
            );
        }
    }

    pub fn save_cursor_background(&mut self, x: usize, y: usize) {
        let start_y: usize = y.saturating_sub(CURSOR_ROW_OFFSET);

        let mut idx: usize = 0; // Index for the saved pixels
        for (i, (start_x, end_x)) in CURSOR_ROWS.iter().enumerate() {
            let cursor_y = start_y as isize + i as isize;

            let count = (end_x - start_x + 1)
                .min(self.width() as isize - x as isize - start_x)
                .max(0) as usize;

            let start_x = x as isize + start_x;

            let bytes_per_pixel = self.info.bytes_per_pixel;

            if cursor_y < 0 || cursor_y >= self.height() as isize {
                continue; // Skip out-of-bounds rows
            }

            let row = if start_x < 0 || start_x > self.width() as isize {
                // Just black
                alloc::vec![0; count * bytes_per_pixel]
            } else {
                self.read_raw_pixel_row(start_x as usize, cursor_y as usize, count)
                    .to_vec()
            };

            self.cursor_background.saved_pixels[idx..idx + count * bytes_per_pixel]
                .copy_from_slice(&row);

            idx += count * bytes_per_pixel;
        }
    }

    fn restore_cursor_background(&mut self, prev_pos: (usize, usize)) {
        let start_y: usize = prev_pos.1.saturating_sub(CURSOR_ROW_OFFSET);

        let mut idx: usize = 0; // Index for the saved pixels
        for (i, (start_x, end_x)) in CURSOR_ROWS.iter().enumerate() {
            let cursor_y = start_y as isize + i as isize;

            let count = (end_x - start_x + 1)
                .min(self.width() as isize - start_x - prev_pos.0 as isize)
                .max(0) as usize;

            let start_x = prev_pos.0 as isize + start_x;

            let bytes_per_pixel = self.info.bytes_per_pixel;

            if cursor_y < 0 || cursor_y >= self.height() as isize {
                continue; // Skip out-of-bounds rows
            }
            if start_x < 0 || start_x + count as isize > self.width() as isize {
                continue;
            }

            let row =
                &self.cursor_background.saved_pixels[idx..idx + count * bytes_per_pixel].to_vec();

            self.write_raw_pixel_row(start_x as usize, cursor_y as usize, &row);
            idx += count * bytes_per_pixel;
        }
    }

    pub fn draw_line(&mut self, start: (usize, usize), end: (usize, usize), color: Color) {
        let dx = end.0 as isize - start.0 as isize;
        let dy = end.1 as isize - start.1 as isize;
        let steps = dx.abs().max(dy.abs()) as usize;

        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let x = (start.0 as f32 + t * dx as f32) as usize;
            let y = (start.1 as f32 + t * dy as f32) as usize;
            self.write_pixel(x, y, color);
        }
    }

    /// Writes a horizontal line of pixels at once.
    fn write_pixel_row(&mut self, x1: usize, x2: usize, y: usize, data: &Color) {
        let x2 = x2.min(self.width() - 1); // Ensure x2 is within bounds
        if y >= self.height() || x1 >= self.width() {
            return;
        }

        let max_width = (x2 - x1 + 1).min(self.width() - x1);
        let bytes_per_pixel = self.info.bytes_per_pixel;
        let start_offset = (y * self.info.stride + x1) * bytes_per_pixel;

        // Convert the color to the appropriate pixel format
        let color: [u8; 3] = match self.info.pixel_format {
            PixelFormat::Rgb => [data.r, data.g, data.b],
            PixelFormat::Bgr => [data.b, data.g, data.r],
            PixelFormat::U8 => [if data.to_u8() > 200 { 0xf } else { 0 }, 0, 0],
            _ => {
                panic!(
                    "pixel format {:?} not supported for writing",
                    self.info.pixel_format
                );
            }
        };

        // Convert the color slice to the correct length
        let data: Vec<u8> = color
            .iter()
            .cloned()
            .cycle()
            .take(max_width * bytes_per_pixel)
            .collect();

        self.framebuffer[start_offset..start_offset + max_width * bytes_per_pixel]
            .copy_from_slice(&data);
    }

    pub fn draw_rect(
        &mut self,
        top_left: (usize, usize),
        bottom_right: (usize, usize),
        color: Color,
    ) {
        for y in top_left.1..=bottom_right.1 {
            self.write_pixel_row(top_left.0, bottom_right.0, y, &color);
        }
    }

    pub fn draw_rect_outline(
        &mut self,
        top_left: (usize, usize),
        bottom_right: (usize, usize),
        color: Color,
    ) {
        self.write_pixel_row(top_left.0, bottom_right.0, top_left.1, &color);
        self.write_pixel_row(top_left.0, bottom_right.0, bottom_right.1, &color);

        for y in top_left.1..=bottom_right.1 {
            self.write_pixel(top_left.0, y, color);
            self.write_pixel(bottom_right.0, y, color);
        }
    }

    pub fn draw_raw_text(&mut self, text: &str, x: usize, y: usize, color: Color, bg_color: Color) {
        let mut x_offset = x;
        for c in text.chars() {
            let rendered_char = get_char_raster(c);

            self.write_rendered_char_at_pos(x_offset, y, &rendered_char, color, bg_color);
            x_offset += LETTER_SPACING + rendered_char.width(); // Move to the next character position
        }
    }
}

unsafe impl Send for FrameBufferWriter {}
unsafe impl Sync for FrameBufferWriter {}

impl fmt::Write for FrameBufferWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            self.write_char(c);
        }
        Ok(())
    }
}

pub fn init(frame: &'static mut FrameBuffer) {
    SCREEN_SIZE.init_once(|| {
        let info = frame.info();
        (info.width as u16, info.height as u16)
    });

    FRAMEBUFFER.init_once(|| {
        let info = frame.info();
        let buffer = frame.buffer_mut();
        spinning_top::Spinlock::new(FrameBufferWriter::new(buffer, info))
    });
}

pub fn set_buffer(buffer: &[u8]) {
    if let Some(fb) = FRAMEBUFFER.get() {
        fb.lock().framebuffer.copy_from_slice(buffer);
    } else {
        panic!("FrameBuffer not initialized");
    }
}
