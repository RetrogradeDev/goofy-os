use bootloader_api::{
    BootInfo,
    info::{FrameBufferInfo, PixelFormat},
};

use conquer_once::spin::OnceCell;
use core::ptr;
use spinning_top::Spinlock;

use font_constants::BACKUP_CHAR;
use noto_sans_mono_bitmap::{
    FontWeight, RasterHeight, RasterizedChar, get_raster, get_raster_width,
};

pub static FRAMEBUFFER: OnceCell<Spinlock<FrameBufferWriter>> = OnceCell::uninit();

/// Constants for the usage of the [`noto_sans_mono_bitmap`] crate.
pub mod font_constants {
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
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

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

/// Allows logging text to a pixel-based framebuffer.
pub struct FrameBufferWriter {
    framebuffer: &'static mut [u8],
    info: FrameBufferInfo,
}

impl FrameBufferWriter {
    /// Creates a new logger that uses the given framebuffer.
    pub fn new(framebuffer: &'static mut [u8], info: FrameBufferInfo) -> Self {
        let mut logger = Self { framebuffer, info };
        logger.clear();
        logger
    }

    /// Erases all everything on the screen
    pub fn clear(&mut self) {
        self.framebuffer.fill(0);
    }

    pub fn width(&self) -> usize {
        self.info.width
    }

    pub fn height(&self) -> usize {
        self.info.height
    }

    pub fn size(&self) -> (usize, usize) {
        (self.info.width, self.info.height)
    }
    pub fn write_char(&mut self, x: usize, y: usize, c: char) {
        let new_xpos = x + font_constants::CHAR_RASTER_WIDTH;
        let new_ypos = y + font_constants::CHAR_RASTER_HEIGHT.val();
        self.write_rendered_char(new_xpos, new_ypos, get_char_raster(c));
    }

    fn write_rendered_char(&mut self, x: usize, y: usize, rendered_char: RasterizedChar) -> usize {
        for (y_char, row) in rendered_char.raster().iter().enumerate() {
            for (x_char, byte) in row.iter().enumerate() {
                self.write_pixel(
                    x + x_char,
                    y + y_char,
                    Color::new(*byte, *byte, *byte / 2), // Yellow-ish color
                );
            }
        }
        rendered_char.width()
    }

    pub fn write_pixel(&mut self, x: usize, y: usize, color: Color) {
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
}

pub fn init(boot_info: &'static mut BootInfo) {
    FRAMEBUFFER.init_once(|| {
        let frame = boot_info.framebuffer.as_mut();
        let info = match frame {
            Some(ref v) => v.info(),
            None => panic!("BOOTLOADER NOT CONFIGURED TO SUPPORT FRAMEBUFFER"),
        };
        let buffer = match frame {
            Some(v) => v.buffer_mut(),
            None => panic!("BOOTLOADER NOT CONFIGURED TO SUPPORT FRAMEBUFFER"),
        };
        spinning_top::Spinlock::new(FrameBufferWriter::new(buffer, info))
    });
}
